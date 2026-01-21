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

enum ReadError {
    Io(std::io::Error),
    ConnectionClosed,
    Parser(ParserError),
    Validator(ValidatorError),
}

impl Server {
    pub async fn run(&self) -> std::io::Result<()> {
        let listener = TcpListener::bind((config().address, config().port)).await?;

        while let Ok((stream, _addr)) = listener.accept().await {
            task::spawn(Self::handle_client(stream));
        }

        Ok(())
    }

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
                parser
                    .feed(&buffer[..n], &mut req)
                    .map_err(ReadError::Parser)?;
            }

            // Feed with empty slice to continue parsing existing buffer
            let parser_res = parser.feed(&[], &mut req).map_err(ReadError::Parser)?;

            match parser_res {
                ParserOk::Incomplete | ParserOk::Ok => {
                    // need more data, continue the loop
                    continue;
                }
                ParserOk::HeadersDone => {
                    // headers are done, validate the request
                    Validator::validate_request(&req).map_err(ReadError::Validator)?;
                    continue; // continue to read body if any
                }
                ParserOk::Done => break, // request is fully parsed
            }
        }

        Ok(req)
    }

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

    async fn write_response(
        stream: &mut TcpStream,
        response: &HttpResponse,
    ) -> std::io::Result<()> {
        let headers = response.build_headers();
        stream.write_all(headers.as_bytes()).await?;
        stream.write_all(&response.body).await?;
        Ok(())
    }
}
