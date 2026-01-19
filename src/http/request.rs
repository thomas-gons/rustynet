use crate::http::HttpMethod;
use crate::http::headers::HttpHeaders;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestHeader {
    Host,
    ContentLength,
    ContentType,
}

pub struct HttpRequest {
    pub method: HttpMethod,
    pub path: String,
    pub http_version: [u8; 2],

    // headers
    pub headers: HttpHeaders,
    pub body: Vec<u8>,
}

impl HttpRequest {
    pub fn new() -> Self {
        Self {
            method: HttpMethod::Unknown,
            path: String::new(),
            http_version: [0; 2],
            headers: HttpHeaders::new(),
            body: Vec::new(),
        }
    }

    pub fn set_header(&mut self, h: RequestHeader, value: &str) {
        let name = match h {
            RequestHeader::ContentLength => "Content-Length",
            RequestHeader::ContentType => "Content-Type",
            RequestHeader::Host => "Host",
        };

        self.headers.set_raw(name, value);
    }
}
