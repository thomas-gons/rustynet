use crate::http::headers::HttpHeaders;
use crate::http::status::HttpStatus;

#[allow(dead_code)]
pub enum ResponseHeader {
    ContentLength,
    ContentType,
    Connection,
    Server,
}

pub struct HttpResponse {
    pub status: HttpStatus,
    pub headers: HttpHeaders,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn new() -> Self {
        Self {
            status: HttpStatus::Ok,
            headers: HttpHeaders::new(),
            body: Vec::new(),
        }
    }

    pub fn set_header(&mut self, h: ResponseHeader, value: &str) {
        let name = match h {
            ResponseHeader::ContentType => "Content-Type",
            ResponseHeader::ContentLength => "Content-Length",
            ResponseHeader::Connection => "Connection",
            ResponseHeader::Server => "Server",
        };

        self.headers.set_raw(name, value);
    }

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
            "HTTP/1.1 {} OK\r\n \
                 {}\
                 \r\n",
            self.status as usize,
            self.headers.stringify(),
        )
    }
}

fn error_code_stringify(code: HttpStatus) -> &'static str {
    match code {
        HttpStatus::BadRequest => "Bad Request",              // 400
        HttpStatus::NotFound => "Not Found",                  // 404
        HttpStatus::MethodNotAllowed => "Method Not Allowed", // 405
        HttpStatus::LengthRequired => "Content-Length field required", // 411
        HttpStatus::PayloadTooLarge => "Payload Too Large",   // 413
        HttpStatus::UriTooLong => "URI Too Long",             // 414

        HttpStatus::InternalServerError => "Internal Server Error", // 500
        HttpStatus::HttpVersionNotSupported => "HTTP Version Not Supported", // 505
        HttpStatus::Ok => "",
    }
}
