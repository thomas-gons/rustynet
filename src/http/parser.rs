use crate::http::{request::{self, *}, status::HttpStatus};

const PARSER_BUF_CAP: usize = 4096;


#[derive(PartialEq, Debug)]
pub enum RequestParserResult {
    OK,
    INCOMPLETE,
    DONE,
    
    // To keep parser logic separate from HTTP status codes,
    // direct http error codes are not used here but mapped later.

    // 400 Bad Request
    ERROR,

    // 413 Payload Too Large
    PAYLOAD_TOO_LARGE,

    // 414 URI Too Long
    TOO_LONG_URI,

    // 505 HTTP Version Not Supported
    HTTP_VERSION_NOT_SUPPORTED, 
}

#[derive(PartialEq, PartialOrd)]
enum RequestParserState {
    REQUEST_LINE,
    HEADERS,
    BODY,
    DONE,
}

pub struct RequestParser {
    buf: [u8; PARSER_BUF_CAP],
    buf_len: usize,
    state: RequestParserState
}


impl RequestParser {
    pub fn new() -> Self {
        Self {
            buf: [0; PARSER_BUF_CAP],
            buf_len: 0,
            state: RequestParserState::REQUEST_LINE,    
        }        
    }

    fn parse_request_line(&mut self, req: &mut HttpRequest) -> RequestParserResult {
        
        // Look for end of request line \r\n
        let mut request_line_end: usize = usize::MAX;
        for i in 0..self.buf_len-1 {
            if self.buf[i] == b'\r' &&
               self.buf[i+1] == b'\n'
            {
                request_line_end = i;
                break;
            }
        }

        if request_line_end == usize::MAX {
            return RequestParserResult::INCOMPLETE;
        }

        // Request line: METHOD PATH HTTP/VERSION
        let request_line = &self.buf[..request_line_end as usize];
        let parts: Vec<&[u8]> = request_line.split(|&b| b == b' ').collect();
        if parts.len() != 3 {
            return RequestParserResult::ERROR;
        }

        if parts[0].len() > request::HTTP_METHOD_MAX_LEN {
            return RequestParserResult::ERROR;
        }

        let method = std::str::from_utf8(parts[0]).unwrap_or("").to_uppercase();
        let method_enum = get_http_method(&method);
        if method_enum == HttpMethod::UNKNOWN {
            return RequestParserResult::ERROR;
        }

        let path = std::str::from_utf8(parts[1]).unwrap_or("");
        if path.len() > request::PATH_MAX_LEN {
            return RequestParserResult::TOO_LONG_URI;
        }

        let http_version: (u8, u8);
        let version = std::str::from_utf8(parts[2]).unwrap_or("");
        let result = version
            .strip_prefix("HTTP/")
            .and_then(|v| v.split_once('.'))
            .and_then(|(maj, min)| {
                Some((maj.parse::<u8>().ok()?, min.parse::<u8>().ok()?))
            })
            .ok_or(RequestParserResult::ERROR);
        
        match result {
            Ok((maj, min)) => {
                if !(maj == 1 && (min == 0 || min == 1)) {
                    return RequestParserResult::HTTP_VERSION_NOT_SUPPORTED;
                }
                http_version = (maj, min);
            },
            Err(e) => return e,
        }

        req.method = method_enum;
        req.path = path.to_string();
        req.http_version = [http_version.0, http_version.1];

        // Adjust request line end to point after \r\n
        request_line_end += 2;
        let remaining = self.buf_len - request_line_end as usize;
        
        // Successfully parsed request line
        // Update parser state and remove parsed line from bufs
        self.state = RequestParserState::HEADERS;
        self.buf.copy_within(request_line_end..self.buf_len, 0);
        self.buf_len = remaining;

        RequestParserResult::OK
    }

    fn parse_headers(&mut self, req: &mut HttpRequest) -> RequestParserResult {
        
        // Look for end of headers \r\n\r\n
        let mut headers_end: usize = usize::MAX;
        for i in 0..self.buf_len-3{
            if self.buf[i] == b'\r' &&
               self.buf[i+1] == b'\n' &&
               self.buf[i+2] == b'\r' &&
               self.buf[i+3] == b'\n'
            {
                headers_end = i;
                break;
            }
        }

        if headers_end == usize::MAX {
            return RequestParserResult::INCOMPLETE;
        }

        if headers_end > request::HEADERS_MAX_LEN {
            return RequestParserResult::ERROR;
        }

        // Parse headers line by line
        let headers = &self.buf[..headers_end as usize];
        let header_lines: Vec<&[u8]> = headers.split(|&b| b == b'\n').collect();
        for line in header_lines {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if line.is_empty() {
                continue;
            }
            let mut it = line.splitn(2, |&b| b == b':');
            let name = it.next().unwrap();
            let value = match it.next() {
                Some(v) => v,
                None => return RequestParserResult::ERROR,
            };

            let name = std::str::from_utf8(name).unwrap_or("").trim();
            let value = std::str::from_utf8(value).unwrap_or("").trim();
            match name.to_lowercase().as_str() {
                "host" => req.set_header(RequestHeader::Host, value),
                "content-length" => {
                    if let Ok(content_len) = value.parse::<usize>() {
                        if content_len > request::BODY_MAX_LEN {
                            return RequestParserResult::PAYLOAD_TOO_LARGE;
                        }

                        req.set_header(RequestHeader::ContentLength, value);
                    } else {
                        return RequestParserResult::ERROR;
                    }
                },
                "content-type" => req.set_header(RequestHeader::ContentType, value),
                _ => {},
            }
        }

        // Adjust headers end to point after \r\n\r\n
        headers_end += 4;
        let remaining = self.buf_len - headers_end as usize;

        // Successfully parsed headers
        // Update parser state and remove parsed headers from bufs
        self.state = RequestParserState::BODY;
        self.buf.copy_within(headers_end..self.buf_len, 0);
        self.buf_len = remaining;
        RequestParserResult::OK
    }

    fn parse_body(&mut self, req: &mut HttpRequest) -> RequestParserResult {
        let content_length = req.headers.get("Content-Length").unwrap().parse::<usize>().unwrap_or(0);
        let to_copy = std::cmp::min(self.buf_len, content_length - req.body.len());

        req.body.extend_from_slice(&self.buf[..to_copy]);
        self.buf.copy_within(to_copy..self.buf_len, 0);
        self.buf_len -= to_copy;

        if req.body.len() == content_length {
            return RequestParserResult::OK;
        }

        RequestParserResult::INCOMPLETE
    }

    pub fn feed(&mut self, buf: &[u8], req: &mut HttpRequest) -> RequestParserResult {

        // Basic overflow check for request line and headers
        if self.state < RequestParserState::BODY && self.buf_len + buf.len() >= PARSER_BUF_CAP {
            return RequestParserResult::ERROR;
        }

        self.buf[self.buf_len..self.buf_len + buf.len()].copy_from_slice(buf);
        self.buf_len = buf.len();

        let mut res: RequestParserResult;

        // Iteratively parse request based on current state while data is available
        loop {
            match self.state {
                RequestParserState::REQUEST_LINE => {
                    res = self.parse_request_line(req);

                    if res != RequestParserResult::OK {
                        return res;
                    }

                },
                RequestParserState::HEADERS => {
                    res = self.parse_headers(req);
                    if res != RequestParserResult::OK {
                        return res;
                    }

                    if !check_headers(req) {
                        return RequestParserResult::ERROR;
                    }

                    if !req.headers.get("Content-Length").is_some() {
                        self.state = RequestParserState::DONE;
                        return RequestParserResult::DONE;
                    }
                },
                RequestParserState::BODY => return self.parse_body(req),
                RequestParserState::DONE => return RequestParserResult::DONE,
            }
        }

    }
}


fn check_headers(req: &HttpRequest) -> bool {
    req.headers.get("Host").is_some()
}

fn get_http_method(method: &str) -> HttpMethod {
    match method {
        "OPTIONS" => HttpMethod::OPTIONS,
        "GET" => HttpMethod::GET,
        "HEAD" => HttpMethod::HEAD,
        "POST" => HttpMethod::POST,
        "PUT" => HttpMethod::PUT,
        "DELETE" => HttpMethod::DELETE,
        "TRACE" => HttpMethod::TRACE,
        "CONNECT" => HttpMethod::CONNECT,
        "PATCH" => HttpMethod::PATCH,
        _ => HttpMethod::UNKNOWN,
    }
}

pub fn map_error_to_http_status(err: &RequestParserResult) -> HttpStatus {
    match err {
        RequestParserResult::ERROR => HttpStatus::BAD_REQUEST,
        RequestParserResult::PAYLOAD_TOO_LARGE => HttpStatus::PAYLOAD_TOO_LARGE,
        RequestParserResult::TOO_LONG_URI => HttpStatus::URI_TOO_LONG,
        RequestParserResult::HTTP_VERSION_NOT_SUPPORTED => HttpStatus::HTTP_VERSION_NOT_SUPPORTED,
        _ => HttpStatus::OK,
    }
}