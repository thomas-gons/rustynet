//! Core HTTP server implementation.
//!
//! This module implements the low-level HTTP server runtime.
//! It is responsible only for networking concerns such as:
//! - accepting TCP connections,
//! - reading raw bytes from the network,
//! - writing raw bytes back to the client.
//!
//! Higher-level HTTP semantics—such as request parsing, validation,
//! and response generation—are intentionally delegated to other modules
//! in the `http` and `handler` namespaces.
//!
//! The server is fully asynchronous and leverages the `async-std` crate
//! to provide non-blocking I/O and concurrent client handling.
//!
//! ## Request handling flow
//!
//! The typical lifecycle of a client connection is as follows:
//!
//! 1. Accept a TCP connection
//! 2. Read raw data from the stream
//! 3. Incrementally parse the data into an [`HttpRequest`]
//!    (delegated to [`http::parser::Parser`](crate::http::parser::Parser))
//! 4. Validate the request
//!    (delegated to [`http::validator::Validator`](crate::http::validator::Validator))
//! 5. Generate an [`HttpResponse`]
//!    (delegated to [`handler::handle_request`](crate::handler::handle_request))
//! 6. Serialize and write the response back to the client
//!
//! Errors at any stage result in appropriate HTTP error responses
//! being generated and sent back to the client.

use crate::config::config;
use crate::handler;
use crate::http::parser::*;
use crate::http::request::HttpRequest;
use crate::http::response::HttpResponse;
use crate::http::validator::{Validator, ValidatorError};
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::task;

pub struct Server;

/// Errors that can occur while reading and parsing an HTTP request from the stream
/// used to interrupt the flow and return appropriate responses.
enum ReadError {
    Io(std::io::Error),
    ConnectionClosed,
    Parser(ParserError),
    Validator(ValidatorError),
}

impl Server {

    /// Starts the HTTP server by binding to the configured address and port.
    ///
    /// This method runs indefinitely, accepting incoming TCP connections and
    /// spawning a new asynchronous task for each client.
    pub async fn run(&self) -> std::io::Result<()> {
        let listener = TcpListener::bind((config().address, config().port)).await?;

        while let Ok((stream, _addr)) = listener.accept().await {
            task::spawn(Self::handle_client(stream));
        }

        Ok(())
    }

    /// Reads and incrementally parses an HTTP request from the TCP stream.
    ///
    /// The request is parsed as data becomes available. Once all headers are read,
    /// the request is validated. If a body is expected, it is read until completion.
    ///
    /// Returns a fully constructed [`HttpRequest`] or a [`ReadError`] in case of
    /// I/O, parsing, or validation failure.
    async fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, ReadError> {
        let mut parser = Parser::new();
        let mut req = HttpRequest::new();
        let mut buffer = vec![0; config().buffer_size];

        loop {
            // If parser buffer is empty, read more data from the stream
            if parser.is_buffer_empty() {
                let n = match stream.read(&mut buffer).await {
                    Ok(0) => return Err(ReadError::ConnectionClosed),
                    Ok(n) => n,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(ReadError::Io(e)),
                };
                
                // Feed newly read bytes into the parser.
                parser
                    .feed(&buffer[..n], &mut req)
                    .map_err(ReadError::Parser)?;
            }

            // Continue parsing using any remaining buffered data.
            // Feeding an empty slice allows the parser to progress without
            // requiring a new network read.
            let parser_res = parser
                .feed(&[], &mut req)
                .map_err(ReadError::Parser)?;

            match parser_res {
                ParserOk::Incomplete | ParserOk::Ok => {
                    // The parser needs more data to make progress.
                    continue;
                }
                ParserOk::HeadersDone => {
                    // All headers have been parsed.
                    // Validate the request early, before reading the body.
                    Validator::validate_request(&req).map_err(ReadError::Validator)?;
                    
                    // Continue the loop to read and parse the request body, if any.
                    continue;
                }
                ParserOk::Done => break, // request is fully parsed
            }
        }

        Ok(req)
    }


    /// Writes the given `HttpResponse` back to the TCP stream.
    /// Serializes the response headers and body appropriately.
    async fn write_response(
        stream: &mut TcpStream,
        response: &HttpResponse,
    ) -> std::io::Result<()> {
        let headers = response.build_headers();
        stream.write_all(headers.as_bytes()).await?;
        stream.write_all(&response.body).await?;
        Ok(())
    }
    
    /// Handles a single client connection.
    /// Reads the HTTP request, processes it via the handler, and writes back the response.
    async fn handle_client(mut stream: TcpStream) -> std::io::Result<()> {
        let response = match Self::read_request(&mut stream).await {
            Ok(r) => handler::handle_request(&r),
            Err(ReadError::Io(err)) => {
                eprintln!("I/O error while reading request: {:?}", err);
                return Ok(());
            }
            Err(ReadError::ConnectionClosed) => return Ok(()),
            Err(ReadError::Parser(err)) => handler::handle_error(err.into_http_status()),
            Err(ReadError::Validator(err)) => handler::handle_error(err.into_http_status()),
        };

        Self::write_response(&mut stream, &response).await
    }
}
