use crate::config::config;
use crate::http::request::*;
use crate::http::status::HttpStatus;
use crate::http::*;

const PARSER_BUF_CAP: usize = 4096;

#[derive(PartialEq, Debug)]
pub enum ParserOk {
    Ok,
    Incomplete,
    HeadersDone,
    Done,
}

#[derive(PartialEq, Debug)]
pub enum ParserError {
    Error,
    TooLongUri,
}

impl ParserError {
    pub fn into_http_status(self) -> HttpStatus {
        match self {
            ParserError::Error => HttpStatus::BadRequest,
            ParserError::TooLongUri => HttpStatus::UriTooLong,
        }
    }
}

#[derive(PartialEq, PartialOrd)]
enum ParserState {
    RequestLine,
    Headers,
    Body,
    Done,
}

pub struct Parser {
    buf: [u8; PARSER_BUF_CAP],
    buf_len: usize,
    state: ParserState,
    headers_bytes_parsed: usize,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            buf: [0; PARSER_BUF_CAP],
            buf_len: 0,
            state: ParserState::RequestLine,
            headers_bytes_parsed: 0,
        }
    }

    pub fn is_buffer_empty(&self) -> bool {
        self.buf_len == 0
    }

    fn parse_request_line(&mut self, req: &mut HttpRequest) -> Result<ParserOk, ParserError> {
        // Look for end of request line \r\n
        let mut request_line_end: usize = usize::MAX;
        
        // Prevent underflow when not enough data
        if self.buf_len < 2 {
            return Ok(ParserOk::Incomplete);
        }

        for i in 0..self.buf_len - 1 {
            if self.buf[i] == b'\r' && self.buf[i + 1] == b'\n' {
                request_line_end = i;
                break;
            }
        }

        if request_line_end == usize::MAX {
            if self.buf_len > config().max_request_line_size {
                return Err(ParserError::Error);
            }

            return Ok(ParserOk::Incomplete);
        }

        if request_line_end > config().max_request_line_size {
            return Err(ParserError::Error);
        }

        // Request line: METHOD PATH HTTP/VERSION
        let request_line = &self.buf[..request_line_end];
        let parts: Vec<&[u8]> = request_line.split(|&b| b == b' ').collect();
        if parts.len() != 3 {
            return Err(ParserError::Error);
        }

        if parts[0].len() > HTTP_METHOD_MAX_LEN {
            return Err(ParserError::Error);
        }

        let method = std::str::from_utf8(parts[0]).unwrap_or("").to_uppercase();
        let method_enum = match http_method_from_str(&method) {
            HttpMethod::Unknown => return Err(ParserError::Error),
            m => m,
        };

        let path = std::str::from_utf8(parts[1]).unwrap_or("");
        if path.len() > config().max_path_size {
            return Err(ParserError::TooLongUri);
        }

        let version = std::str::from_utf8(parts[2]).unwrap_or("");
        let result = version
            .strip_prefix("HTTP/")
            .and_then(|v| v.split_once('.'))
            .and_then(|(maj, min)| Some((maj.parse::<u8>().ok()?, min.parse::<u8>().ok()?)))
            .ok_or(Err(ParserError::Error));

        let http_version = match result {
            Ok((maj, min)) => (maj, min),
            Err(e) => return e,
        };

        req.method = method_enum;
        req.path = path.to_string();
        req.http_version = http_version;

        // Adjust request line end to point after \r\n
        request_line_end += 2;
        let remaining = self.buf_len - request_line_end;

        // Successfully parsed request line
        // Update parser state and remove parsed line from bufs
        self.state = ParserState::Headers;
        self.buf.copy_within(request_line_end..self.buf_len, 0);
        self.buf_len = remaining;

        Ok(ParserOk::Ok)
    }

    fn parse_headers(&mut self, req: &mut HttpRequest) -> Result<ParserOk, ParserError> {
        // Look for end of headers \r\n\r\n
        let mut headers_end: usize = usize::MAX;
        let mut headers_chunk_end: usize = usize::MAX;

        // Prevent underflow when not enough data
        if self.buf_len < 4 {
            return Ok(ParserOk::Incomplete);
        }

        for i in 0..self.buf_len - 3 {
            if self.buf[i] == b'\r'
                && self.buf[i + 1] == b'\n'
                && self.buf[i + 2] == b'\r'
                && self.buf[i + 3] == b'\n'
            {
                headers_end = i;
                break;
            }

            if self.buf[i] == b'\r' && self.buf[i + 1] == b'\n' {
                headers_chunk_end = i;
            }
        }

        if headers_chunk_end == usize::MAX {
            if self.buf_len == PARSER_BUF_CAP {
                return Err(ParserError::Error);
            }
            return Ok(ParserOk::Incomplete);
        }

        if headers_end != usize::MAX {
            headers_chunk_end = headers_end;
        }

        self.headers_bytes_parsed += headers_chunk_end + if headers_end != usize::MAX { 4 } else { 2 };
        if self.headers_bytes_parsed > config().max_header_size {
            return Err(ParserError::Error);
        }

        // Parse headers line by line
        let headers = &self.buf[..headers_chunk_end];
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
                None => return Err(ParserError::Error),
            };

            let name = std::str::from_utf8(name).unwrap_or("").trim();
            let value = std::str::from_utf8(value).unwrap_or("").trim();
            match name.to_lowercase().as_str() {
                "host" => req.set_header(RequestHeader::Host, value),
                "content-length" => {
                    value.parse::<usize>().map_err(|_| ParserError::Error)?;

                    req.set_header(RequestHeader::ContentLength, value);
                }
                "content-type" => req.set_header(RequestHeader::ContentType, value),
                "accept-encoding" => req.headers.set_raw("Accept-Encoding", value),
                _ => {}
            }
        }

        headers_chunk_end += if headers_end != usize::MAX { 4 } else { 2 };
        let remaining = self.buf_len - headers_chunk_end;

        // Successfully parsed headers
        // Update parser state and remove parsed headers from bufs
        self.buf.copy_within(headers_chunk_end..self.buf_len, 0);
        self.buf_len = remaining;

        if headers_end == usize::MAX {
            return Ok(ParserOk::Incomplete);
        }        
        self.state = ParserState::Body;
        Ok(ParserOk::Ok)
    }

    fn parse_body(&mut self, req: &mut HttpRequest) -> Result<ParserOk, ParserError> {
        let content_length = match req.headers.get("Content-Length") {
            Some(v) => v.parse::<usize>().map_err(|_| ParserError::Error)?,
            None => {
                self.state = ParserState::Done;
                return Ok(ParserOk::Done);
            }
        };

        let remaining = content_length.saturating_sub(req.body.len());
        let to_copy = std::cmp::min(self.buf_len, remaining);
        if req.body.len() + to_copy > config().max_body_size {
            return Err(ParserError::Error);
        }

        req.body.extend_from_slice(&self.buf[..to_copy]);
        self.buf.copy_within(to_copy..self.buf_len, 0);
        self.buf_len -= to_copy;

        if req.body.len() == content_length {
            self.state = ParserState::Done;
            return Ok(ParserOk::Done);
        }

        Ok(ParserOk::Incomplete)
    }

    fn fill_buffer(&mut self, buf: &[u8]) -> Result<(), ParserError> {
        if self.buf_len + buf.len() > PARSER_BUF_CAP {
            return Err(ParserError::Error);
        }

        self.buf[self.buf_len..self.buf_len + buf.len()].copy_from_slice(buf);
        self.buf_len += buf.len();
        Ok(())
    }

    pub fn feed(&mut self, buf: &[u8], req: &mut HttpRequest) -> Result<ParserOk, ParserError> {
        self.fill_buffer(buf)?;

        loop {
            match self.state {
                ParserState::RequestLine => {
                    let r = self.parse_request_line(req)?;
                    if r == ParserOk::Incomplete {
                        return Ok(ParserOk::Incomplete);
                    }
                }
                ParserState::Headers => match self.parse_headers(req)? {
                    ParserOk::Ok => return Ok(ParserOk::HeadersDone),
                    other_outcome => return Ok(other_outcome),
                },
                ParserState::Body => return self.parse_body(req),
                ParserState::Done => return Ok(ParserOk::Done),
            }
        }
    }
}


#[cfg(test)] mod tests {
    use std::sync::Once;

    use super::*;
    use crate::config::*;
    use crate::http::request::HttpRequest;

    static INIT: Once = Once::new();

    fn setup() {
        INIT.call_once(|| {
            let cfg = ServerConfig {
                max_path_size: 1024,
                max_header_size: 8192,
                max_body_size: 1024 * 1024,
                ..Default::default()
            };
            set_config(cfg);
        });
    }

    fn run_test<F: FnOnce(&mut Parser, &mut HttpRequest)>(f: F) {
        setup();
        let mut parser = Parser::new();
        let mut req = HttpRequest::new();
        f(&mut parser, &mut req);
    }

    fn parse_iteratively<F>(
        parser: &mut Parser,
        req: &mut HttpRequest,
        data: &[u8],
        parsing_method: F
    ) -> Result<ParserOk, ParserError> 
    where
        F: Fn(&mut Parser, &mut HttpRequest) -> Result<ParserOk, ParserError>
    {
        let mut offset = 0;
        loop {
            let chunk_size = std::cmp::min(4, data.len() - offset);
            let chunk = &data[offset..offset + chunk_size];
            offset += chunk_size;

            parser.fill_buffer(chunk).unwrap();
            let result = parsing_method(parser, req)?;
            if result != ParserOk::Incomplete || offset >= data.len() {
                return Ok(result);
            }
        }
    }

    #[test]
    fn test_parse_request_line() {
        run_test(|parser, req| {
            let request_line = b"GET /index.html HTTP/1.1\r\n";
            parser.fill_buffer(request_line).unwrap();
            let result = parser.parse_request_line(req).unwrap();
            assert_eq!(result, ParserOk::Ok);
            assert_eq!(req.method, HttpMethod::Get);
            assert_eq!(req.path, "/index.html");
            assert_eq!(req.http_version, (1, 1));
        });
    }

    #[test]
    fn test_parse_rl_bad_method() {
        run_test(|parser, req| {
            let request_line = b"BADMETHOD /index.html HTTP/1.1\r\n";
            parser.fill_buffer(request_line).unwrap();
            let result = parser.parse_request_line(req);
            assert_eq!(result, Err(ParserError::Error));
        });
    }

    #[test]
    fn test_parse_rl_too_long_uri() {
        run_test(|parser, req| {
            let long_path = "a".repeat(config().max_path_size + 1);
            let request_line = format!("GET /{} HTTP/1.1\r\n", long_path);

            let result = parse_iteratively(
                parser,
                req,
                request_line.as_bytes(),
                |p, r| p.parse_request_line(r)
            );

            assert_eq!(result, Err(ParserError::TooLongUri));
        });
    }

    #[test]
    fn test_parse_rl_bad_http_version() {
        run_test(|parser, req| {
            let request_line = b"GET /index.html HTTP/XYZ\r\n";
            parser.fill_buffer(request_line).unwrap();
            let result = parser.parse_request_line(req);
            assert_eq!(result, Err(ParserError::Error));

            let request_line = b"GET /index.html HTT/1.2\r\n";
            parser.fill_buffer(request_line).unwrap();
            let result = parser.parse_request_line(req);
            assert_eq!(result, Err(ParserError::Error));

        });
    }

    #[test]
    fn test_parse_rl_too_long() {
        run_test(|parser, req| {
            let long_request_line = format!("GET /{} HTTP/1.1\r\n", "a".repeat(config().max_header_size + 1));
            let result = parse_iteratively(
                parser,
                req,
                long_request_line.as_bytes(),
                |p, r| p.parse_request_line(r)
            );
            assert_eq!(result, Err(ParserError::Error));
        });
    }

    #[test]
    fn test_parse_headers() {
        run_test(|parser, req| {
            let headers = b"Host: example.com\r\nContent-Length: 123\r\n\r\n";
            parser.fill_buffer(headers).unwrap();
            let result = parser.parse_headers(req).unwrap();
            assert_eq!(result, ParserOk::Ok);
            assert_eq!(req.headers.get("Host").unwrap(), "example.com");
            assert_eq!(req.headers.get("Content-Length").unwrap(), "123");
        });
    }

    #[test]
    fn test_parse_headers_too_long() {
        run_test(|parser, req| {
            let long_header = format!("X-Header: {}\r\n\r\n", "a".repeat(config().max_header_size + 1));
            let result = parse_iteratively(
                parser,
                req,
                long_header.as_bytes(),
                |p, r| p.parse_headers(r)
            );
            assert_eq!(result, Err(ParserError::Error));
        });
    }

    #[test]
    fn test_parse_body() {
        run_test(|parser, req| {
            req.set_header(RequestHeader::ContentLength, "5");
            let body = b"Hello";
            parser.fill_buffer(body).unwrap();
            let result = parser.parse_body(req).unwrap();
            assert_eq!(result, ParserOk::Done);
            assert_eq!(req.body, b"Hello");
        });
    }

    #[test]
    fn test_parse_body_incomplete() {
        run_test(|parser, req| {
            req.set_header(RequestHeader::ContentLength, "10");
            let body = b"Hello";
            parser.fill_buffer(body).unwrap();
            let result = parser.parse_body(req).unwrap();
            assert_eq!(result, ParserOk::Incomplete);
            assert_eq!(req.body, b"Hello");
        });
    }

    #[test]
    fn test_parse_body_too_large() {
        run_test(|parser, req| {
            req.set_header(RequestHeader::ContentLength, &(config().max_body_size + 1).to_string());
            let body = vec![b'a'; config().max_body_size + 1];
            let result = parse_iteratively(
                parser,
                req,
                body.as_slice(),
                |p, r| p.parse_body(r)
            );

            assert_eq!(result, Err(ParserError::Error));
        });
    }   
}