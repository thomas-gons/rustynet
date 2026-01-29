use crate::http::HttpMethod;
use crate::http::headers::HttpHeaders;

/// Common HTTP request headers
/// This enum defines the set of headers that can be explicitly set on an
/// [`HttpRequest`] through its safe wrapper API.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestHeader {
    Host,
    ContentLength,
    ContentType,
}

pub struct HttpRequest {
    pub method: HttpMethod,
    pub uri: String,
    pub http_version: (u8, u8),

    // headers
    pub headers: HttpHeaders,
    pub body: Vec<u8>,
}

impl HttpRequest {
    pub fn new() -> Self {
        Self {
            method: HttpMethod::Unknown,
            uri: String::new(),
            http_version: (0, 0),
            headers: HttpHeaders::new(),
            body: Vec::new(),
        }
    }

    /// Sets a request header constrained to the allowed [`RequestHeader`] variants.
    ///
    /// This method acts as a safe wrapper around [`HttpHeaders::set_raw`],
    /// ensuring that only headers explicitly supported by [`RequestHeader`]
    /// can be added through this API.
    ///
    /// No validation is performed on the header value itself.
    pub fn set_header(&mut self, h: RequestHeader, value: &str) {
        let name = match h {
            RequestHeader::ContentLength => "Content-Length",
            RequestHeader::ContentType => "Content-Type",
            RequestHeader::Host => "Host",
        };

        self.headers.set_raw(name, value);
    }
}
