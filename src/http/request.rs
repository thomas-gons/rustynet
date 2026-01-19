use crate::http::headers::HttpHeaders;

pub const REQUEST_LINE_MAX_LEN: usize = 2048;
pub const HTTP_METHOD_MAX_LEN: usize = 16;
pub const PATH_MAX_LEN: usize = 1024;
pub const HEADERS_MAX_LEN: usize = 8192;
pub const BODY_MAX_LEN: usize = 1024 * 1024; // 1MB

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    OPTIONS,
    GET,
    HEAD,
    POST,
    PUT,
    DELETE,
    TRACE,
    CONNECT,
    PATCH,
    UNKNOWN
}

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
            method: HttpMethod::UNKNOWN,
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