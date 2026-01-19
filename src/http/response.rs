use crate::http::status::HttpStatus;
use crate::http::headers::HttpHeaders;


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
            status: HttpStatus::OK,
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
        if self.status != HttpStatus::OK {
            let error = error_code_stringify(self.status);

            // HTTP <major>.<minor> <status> <reason>\r\n
            // \r\n
            return format!("HTTP/1.1 {} {}\r\n \
                            \r\n",
                           self.status as usize, error
            )
        }

        // HTTP <major>.<minor> <status>\r\n
        // <header_name>: <header_value>\r\n
        // ...
        // \r\n
        format!("HTTP/1.1 {} OK\r\n \
                 {}\
                 \r\n",
                self.status as usize,
                self.headers.stringify(),
        )
    }
}


fn error_code_stringify(code: HttpStatus) -> &'static str {
    match code {
        HttpStatus::BAD_REQUEST => "Bad Request",                               // 400
        HttpStatus::NOT_FOUND => "Not Found",                                   // 404
        HttpStatus::METHOD_NOT_ALLOWED => "Method Not Allowed",                 // 405
        HttpStatus::PAYLOAD_TOO_LARGE => "Payload Too Large",                   // 413
        HttpStatus::URI_TOO_LONG => "URI Too Long",                             // 414

        HttpStatus::INTERNAL_SERVER_ERROR => "Internal Server Error",           // 500
        HttpStatus::HTTP_VERSION_NOT_SUPPORTED => "HTTP Version Not Supported", // 505
        HttpStatus::OK => "",
    } 
}