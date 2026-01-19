use crate::config::config;
use crate::handler;
use crate::http::parser::*;
use crate::http::request::HttpRequest;
use crate::http::response::HttpResponse;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

pub struct Server {
    listener: TcpListener,
}

enum ReadError {
    Io(std::io::Error),
    ConnectionClosed,
    Parser(RequestParserOutcome),
}

impl Server {
    pub fn init() -> std::io::Result<Self> {
        let listener = TcpListener::bind((config().address, config().port))?;
        Ok(Self { listener })
    }

    pub fn run(&self) -> std::io::Result<()> {
        for stream in self.listener.incoming() {
            let mut stream = stream?;
            self.handle_client(&mut stream)?;
        }
        Ok(())
    }

    fn read_request(&self, stream: &mut TcpStream) -> Result<HttpRequest, ReadError> {
        let mut parser = RequestParser::new();
        let mut req = HttpRequest::new();
        let mut buffer = vec![0; config().buffer_size];

        let mut parser_res = RequestParserOutcome::Incomplete;
        while parser_res != RequestParserOutcome::Done {
            let r = stream.read(&mut buffer);

            let n = match r {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(ReadError::Io(e)),
            };

            // Connection closed
            if n == 0 {
                return Err(ReadError::ConnectionClosed);
            }

            parser_res = parser.feed(&buffer[..n], &mut req);
            match parser_res {
                RequestParserOutcome::Ok => continue,
                RequestParserOutcome::Incomplete => continue,
                _ => {
                    if parser_res != RequestParserOutcome::Done {
                        return Err(ReadError::Parser(parser_res));
                    }
                }
            }
        }

        Ok(req)
    }

    fn handle_client(&self, stream: &mut TcpStream) -> std::io::Result<()> {
        let response = match self.read_request(stream) {
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

        self.write_response(stream, &response)
    }

    fn write_response(
        &self,
        stream: &mut TcpStream,
        response: &HttpResponse,
    ) -> std::io::Result<()> {
        let headers = response.build_headers();
        stream.write_all(headers.as_bytes())?;
        stream.write_all(&response.body)?;
        Ok(())
    }
}
