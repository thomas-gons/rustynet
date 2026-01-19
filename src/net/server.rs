use std::net::{TcpListener, TcpStream, Ipv4Addr};
use std::io::{Read, Write};
use std::fs::File;

use crate::http::status::HttpStatus;
use crate::http::parser::*;
use crate::http::request::HttpRequest;
use crate::http::response::*;
use crate::net::config::ServerConfig;

pub struct Server {
    config: ServerConfig,
    listener: TcpListener
}

impl Server {
    pub fn init(config: ServerConfig) -> std::io::Result<Self> {
        let listener = TcpListener::bind((
            config.address,
            config.port
        ))?;
        Ok(Self { config, listener })
    }

    pub fn run(&self) -> std::io::Result<()> {
        for stream in self.listener.incoming() {
            let mut stream = stream?;
            self.handle_client(&mut stream)?;
        }
        Ok(())
    }

    fn handle_client(&self, stream: &mut TcpStream) -> std::io::Result<()> {
        let mut parser = RequestParser::new();
        let mut req = HttpRequest::new();
        let mut response = HttpResponse::new();
        let mut buffer = vec![0; self.config.buffer_size];

        let mut parser_res = RequestParserResult::INCOMPLETE;
        while parser_res != RequestParserResult::DONE {
            let r = stream.read(&mut buffer);

            let n = match r {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };

            // Connection closed
            if n == 0 {
                break;
            }

            parser_res = parser.feed(&buffer[..n], &mut req);
            match parser_res {
                RequestParserResult::OK => continue,
                RequestParserResult::INCOMPLETE => continue,
                _ => {
                    let status = map_error_to_http_status(&parser_res);
                    if status != HttpStatus::OK {
                        response.status = status;
                        break;
                    }
                }
            }
        }

        if response.status != HttpStatus::OK {
            let err = response.build_headers();
            stream.write_all(err.as_bytes())?;
            return Ok(())
        }

        if !req.path.eq("/") {
            let r = Self::send_file(stream, &req, &mut response);
            match r {
                Ok(_) => {},
                Err(_) => {
                    response.status = HttpStatus::NOT_FOUND;
                    let body = b"404 Not Found".to_vec();
                    response.set_header(ResponseHeader::ContentLength, &body.len().to_string());
                    response.set_header(ResponseHeader::ContentType, "text/plain");
                    response.set_header(ResponseHeader::Connection, "close");
                    response.set_header(ResponseHeader::Server, "RustyNet/0.1");
                    response.body = body;
                    stream.write_all(response.build_headers().as_bytes())?;
                    stream.write_all(&response.body)?;
                }
            }
        } else {
            let body = b"Hello from RustyNet!".to_vec();
            response.set_header(ResponseHeader::ContentLength, &body.len().to_string());
            response.set_header(ResponseHeader::ContentType, "text/plain");
            response.set_header(ResponseHeader::Connection, "close");
            response.set_header(ResponseHeader::Server, "RustyNet/0.1");
            response.body = body;
            stream.write_all(response.build_headers().as_bytes())?;
            stream.write_all(&response.body)?;
        }

        Ok(())
    }

    fn send_file(stream: &mut TcpStream, req: &HttpRequest, response: &mut HttpResponse) -> std::io::Result<()> {
        let r = File::open(&req.path);

        let mut file = match r {
            Ok(file) => file,
            Err(err) => return Err(err),
        };


        let file_ext = match req.path.rsplit('.').next() {
            Some(ext) => ext,
            None => "",
        };

        response.set_header(ResponseHeader::ContentLength, &file.metadata()?.len().to_string());
        response.set_header(ResponseHeader::ContentType, get_mime_type(file_ext));
        let headers = response.build_headers();

        stream.write_all(headers.as_bytes())?;
        std::io::copy(&mut file, stream)?;

        Ok(())
    }
}


fn get_mime_type(file_ext: &str) -> &str {
    match file_ext {
        "htm" | "html" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "json" => "application/json",
        "xml" => "application/xml",
        "txt" => "text/plain",
        "pdf" => "application/pdf",
        _ => "application/octet-stream", 
    }
}
