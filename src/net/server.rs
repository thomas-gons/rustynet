use crate::config::config;
use crate::handler;
use crate::http::parser::*;
use crate::http::request::HttpRequest;
use crate::http::response::HttpResponse;
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::task;

pub struct Server;

enum ReadError {
    Io(std::io::Error),
    ConnectionClosed,
    Parser(RequestParserOutcome),
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
        let mut parser = RequestParser::new();
        let mut req = HttpRequest::new();
        let mut buffer = vec![0; config().buffer_size];

        let mut parser_res = RequestParserOutcome::Incomplete;
        while parser_res != RequestParserOutcome::Done {
            let n = match stream.read(&mut buffer).await {
                Ok(0) => return Err(ReadError::ConnectionClosed),
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(ReadError::Io(e)),
            };


            parser_res = parser.feed(&buffer[..n], &mut req);
            match parser_res {
                RequestParserOutcome::Ok | RequestParserOutcome::Incomplete => continue,
                _ => return Err(ReadError::Parser(parser_res)),
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
            Err(ReadError::Parser(err)) => {
                let err = err.into_http_status();
                handler::handle_error(err)
            }
        };

        Self::write_response(&mut stream, &response).await
    }

    async fn write_response(stream: &mut TcpStream, response: &HttpResponse) -> std::io::Result<()> {
        let headers = response.build_headers();
        stream.write_all(headers.as_bytes()).await?;
        stream.write_all(&response.body).await?;
        Ok(())
    }
}
