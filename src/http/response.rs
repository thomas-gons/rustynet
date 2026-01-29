use crate::config::config;
use crate::http::headers::HttpHeaders;
use crate::http::status::HttpStatus;
use httpdate;


/// Common HTTP request headers
/// This enum defines the set of headers that can be explicitly set on an
/// [`HttpResponse`] through its safe wrapper API.
#[allow(dead_code)]
pub enum ResponseHeader {
    ContentLength,
    ContentType,
    ContentEncoding,
    Connection,
    Date,
    Server,
}

pub struct HttpResponse {
    pub status: HttpStatus,
    pub headers: HttpHeaders,
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Creates a new `HttpResponse` with default headers set.
    /// These include the `Server` header with the server name from the configuration
    /// and the `Date` header with the current system time.
    pub fn new() -> Self {
        let mut res = Self {
            status: HttpStatus::Ok,
            headers: HttpHeaders::new(),
            body: Vec::new(),
        };

        // Host system name
        res.set_header(ResponseHeader::Server, &config().server_name);
        res.set_header(
            ResponseHeader::Date,
            &httpdate::fmt_http_date(std::time::SystemTime::now()),
        );
        res
    }

    /// Sets a request header constrained to the allowed [`ResponseHeader`] variants.
    ///
    /// This method acts as a safe wrapper around [`HttpHeaders::set_raw`],
    /// ensuring that only headers explicitly supported by [`ResponseHeader`]
    /// can be added through this API.
    ///
    /// No validation is performed on the header value itself.
    pub fn set_header(&mut self, h: ResponseHeader, value: &str) {
        let name = match h {
            ResponseHeader::ContentType => "Content-Type",
            ResponseHeader::ContentLength => "Content-Length",
            ResponseHeader::ContentEncoding => "Content-Encoding",
            ResponseHeader::Connection => "Connection",
            ResponseHeader::Date => "Date",
            ResponseHeader::Server => "Server",
        };

        self.headers.set_raw(name, value);
    }

    /// Builds the HTTP response headers as a formatted string.
    /// If the response status is not `200 OK`, it generates a minimal
    /// response with just the status line.
    /// 
    /// Otherwise, it includes all headers set in the `HttpHeaders` structure.
    pub fn build_headers(&self) -> String {
        if self.status != HttpStatus::Ok {
            let error = error_code_stringify(self.status);

            // HTTP <major>.<minor> <status> <reason>\r\n
            // \r\n
            return format!(
                "HTTP/1.1 {} {}\r\n \
                            \r\n",
                self.status as usize, error
            );
        }

        // HTTP <major>.<minor> <status>\r\n
        // <header_name>: <header_value>\r\n
        // ...
        // \r\n
        format!(
            "HTTP/1.1 {} OK\r\n\
                 {}\
                 \r\n",
            self.status as usize,
            self.headers.stringify(),
        )
    }
}

/// Maps HTTP status codes to their standard reason phrases.
fn error_code_stringify(code: HttpStatus) -> &'static str {
    match code {
        HttpStatus::BadRequest => "Bad Request",                              // 400
        HttpStatus::Forbidden => "Forbidden",                                  // 403
        HttpStatus::NotFound => "Not Found",                                  // 404
        HttpStatus::MethodNotAllowed => "Method Not Allowed",                 // 405
        HttpStatus::LengthRequired => "Content-Length field required",        // 411
        HttpStatus::PayloadTooLarge => "Payload Too Large",                   // 413
        HttpStatus::UriTooLong => "URI Too Long",                             // 414

        HttpStatus::InternalServerError => "Internal Server Error",           // 500
        HttpStatus::HttpVersionNotSupported => "HTTP Version Not Supported",  // 505
        HttpStatus::Ok => "",
    }
}
