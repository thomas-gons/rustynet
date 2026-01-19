use crate::config::config;
use crate::http::request::*;
use crate::http::status::HttpStatus;
use crate::http::*;

const PARSER_BUF_CAP: usize = 4096;

#[derive(PartialEq, Debug)]
pub enum RequestParserOutcome {
    Ok,
    Incomplete,
    Done,

    // To keep parser logic separate from HTTP status codes,
    // direct http error codes are not used here but mapped later.

    // 400 Bad Request
    Error,

    // 413 Payload Too Large
    PayloadTooLarge,

    // 414 URI Too Long
    TooLongUri,

    // 505 HTTP Version Not Supported
    HttpVersionNotSupported,
}

impl RequestParserOutcome {
    pub fn into_http_status(self) -> HttpStatus {
        match self {
            RequestParserOutcome::Error => HttpStatus::BadRequest,
            RequestParserOutcome::PayloadTooLarge => HttpStatus::PayloadTooLarge,
            RequestParserOutcome::TooLongUri => HttpStatus::UriTooLong,
            RequestParserOutcome::HttpVersionNotSupported => HttpStatus::HttpVersionNotSupported,
            _ => HttpStatus::Ok,
        }
    }
}

#[derive(PartialEq, PartialOrd)]
enum RequestParserState {
    RequestLine,
    Headers,
    Body,
    Done,
}

pub struct RequestParser {
    buf: [u8; PARSER_BUF_CAP],
    buf_len: usize,
    state: RequestParserState,
}

impl RequestParser {
    pub fn new() -> Self {
        Self {
            buf: [0; PARSER_BUF_CAP],
            buf_len: 0,
            state: RequestParserState::RequestLine,
        }
    }

    fn parse_request_line(&mut self, req: &mut HttpRequest) -> RequestParserOutcome {
        // Look for end of request line \r\n
        let mut request_line_end: usize = usize::MAX;
        for i in 0..self.buf_len - 1 {
            if self.buf[i] == b'\r' && self.buf[i + 1] == b'\n' {
                request_line_end = i;
                break;
            }
        }

        if request_line_end == usize::MAX {
            return RequestParserOutcome::Incomplete;
        }

        // Request line: METHOD PATH HTTP/VERSION
        let request_line = &self.buf[..request_line_end];
        let parts: Vec<&[u8]> = request_line.split(|&b| b == b' ').collect();
        if parts.len() != 3 {
            return RequestParserOutcome::Error;
        }

        if parts[0].len() > HTTP_METHOD_MAX_LEN {
            return RequestParserOutcome::Error;
        }

        let method = std::str::from_utf8(parts[0]).unwrap_or("").to_uppercase();
        let method_enum = match http_method_from_str(&method) {
            HttpMethod::Unknown => return RequestParserOutcome::Error,
            m => m,
        };

        let path = std::str::from_utf8(parts[1]).unwrap_or("");
        if path.len() > config().max_path_size {
            return RequestParserOutcome::TooLongUri;
        }

        let version = std::str::from_utf8(parts[2]).unwrap_or("");
        let result = version
            .strip_prefix("HTTP/")
            .and_then(|v| v.split_once('.'))
            .and_then(|(maj, min)| Some((maj.parse::<u8>().ok()?, min.parse::<u8>().ok()?)))
            .ok_or(RequestParserOutcome::Error);

        let http_version = match result {
            Ok((maj, min)) => {
                if !(maj == 1 && (min == 0 || min == 1)) {
                    return RequestParserOutcome::HttpVersionNotSupported;
                }
                (maj, min)
            }
            Err(e) => return e,
        };

        req.method = method_enum;
        req.path = path.to_string();
        req.http_version = [http_version.0, http_version.1];

        // Adjust request line end to point after \r\n
        request_line_end += 2;
        let remaining = self.buf_len - request_line_end;

        // Successfully parsed request line
        // Update parser state and remove parsed line from bufs
        self.state = RequestParserState::Headers;
        self.buf.copy_within(request_line_end..self.buf_len, 0);
        self.buf_len = remaining;

        RequestParserOutcome::Ok
    }

    fn parse_headers(&mut self, req: &mut HttpRequest) -> RequestParserOutcome {
        // Look for end of headers \r\n\r\n
        let mut headers_end: usize = usize::MAX;
        for i in 0..self.buf_len - 3 {
            if self.buf[i] == b'\r'
                && self.buf[i + 1] == b'\n'
                && self.buf[i + 2] == b'\r'
                && self.buf[i + 3] == b'\n'
            {
                headers_end = i;
                break;
            }
        }

        if headers_end == usize::MAX {
            return RequestParserOutcome::Incomplete;
        }

        if headers_end > config().max_header_size {
            return RequestParserOutcome::Error;
        }

        // Parse headers line by line
        let headers = &self.buf[..headers_end];
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
                None => return RequestParserOutcome::Error,
            };

            let name = std::str::from_utf8(name).unwrap_or("").trim();
            let value = std::str::from_utf8(value).unwrap_or("").trim();
            match name.to_lowercase().as_str() {
                "host" => req.set_header(RequestHeader::Host, value),
                "content-length" => {
                    if let Ok(content_len) = value.parse::<usize>() {
                        if content_len > config().max_body_size {
                            return RequestParserOutcome::PayloadTooLarge;
                        }

                        req.set_header(RequestHeader::ContentLength, value);
                    } else {
                        return RequestParserOutcome::Error;
                    }
                }
                "content-type" => req.set_header(RequestHeader::ContentType, value),
                _ => {}
            }
        }

        // Adjust headers end to point after \r\n\r\n
        headers_end += 4;
        let remaining = self.buf_len - headers_end;

        // Successfully parsed headers
        // Update parser state and remove parsed headers from bufs
        self.state = RequestParserState::Body;
        self.buf.copy_within(headers_end..self.buf_len, 0);
        self.buf_len = remaining;
        RequestParserOutcome::Ok
    }

    fn parse_body(&mut self, req: &mut HttpRequest) -> RequestParserOutcome {
        let content_length = req
            .headers
            .get("Content-Length")
            .unwrap()
            .parse::<usize>()
            .unwrap_or(0);
        let to_copy = std::cmp::min(self.buf_len, content_length - req.body.len());

        req.body.extend_from_slice(&self.buf[..to_copy]);
        self.buf.copy_within(to_copy..self.buf_len, 0);
        self.buf_len -= to_copy;

        if req.body.len() == content_length {
            return RequestParserOutcome::Ok;
        }

        RequestParserOutcome::Incomplete
    }

    pub fn feed(&mut self, buf: &[u8], req: &mut HttpRequest) -> RequestParserOutcome {
        // Basic overflow check for request line and headers
        if self.state < RequestParserState::Body && self.buf_len + buf.len() >= PARSER_BUF_CAP {
            return RequestParserOutcome::Error;
        }

        self.buf[self.buf_len..self.buf_len + buf.len()].copy_from_slice(buf);
        self.buf_len = buf.len();

        let mut res: RequestParserOutcome;

        // Iteratively parse request based on current state while data is available
        loop {
            match self.state {
                RequestParserState::RequestLine => {
                    res = self.parse_request_line(req);

                    if res != RequestParserOutcome::Ok {
                        return res;
                    }
                }
                RequestParserState::Headers => {
                    res = self.parse_headers(req);
                    if res != RequestParserOutcome::Ok {
                        return res;
                    }

                    if !check_headers(req) {
                        return RequestParserOutcome::Error;
                    }

                    if req.headers.get("Content-Length").is_none() {
                        self.state = RequestParserState::Done;
                        return RequestParserOutcome::Done;
                    }
                }
                RequestParserState::Body => return self.parse_body(req),
                RequestParserState::Done => return RequestParserOutcome::Done,
            }
        }
    }
}

fn check_headers(req: &HttpRequest) -> bool {
    req.headers.get("Host").is_some()
}
