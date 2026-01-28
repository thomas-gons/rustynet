/*!
A finite-state machine parser for HTTP request serialization
([`HttpRequest`]).

The request is parsed incrementally in three stages: request line,
headers, and body. Parsing is performed as a pipeline, and each stage
may yield either a [`ParserOk`] status or a [`ParserError`].

This design allows the parser to operate directly on network packets.
An internal buffer is used to accumulate data until a complete request
field can be parsed, except for the request line, which must fit entirely
within the parsing buffer. By contrast, body data is appended directly
to the request body as it is received.


The [`ParserOk::Incomplete`] state is used to signal the server to
continue reading packets in order to complete the current field.

As in a compiler, the parser only checks for syntactic errors and
reports the corresponding error. Semantic validation is left to the
caller: the [`ParserOk::HeadersDone`] state allows the server to pause
parsing and validate the headers before body parsing begins.
*/

use crate::config::config;
use crate::http::request::*;
use crate::http::status::HttpStatus;
use crate::http::*;

/// Capacity of the internal parser buffer.
/// Its value should be the same as the server read [`buffer capacity`](crate::config::ServerConfig::buffer_size)
const PARSER_BUF_CAP: usize = 4096;

/// The finite states of the parser.
/// They are given sequentially as a pipeline
/// Each state is associated with a parsing method which may return a `ParserError`.
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

    /// helper to track the global headers size and apply the [`server limit`](crate::config::ServerConfig::max_header_size)
    headers_bytes_parsed: usize,
}

#[derive(PartialEq, Debug)]
pub enum ParserOk {
    /// Parsing completed successfully for the current stage.
    Ok,

    /// More data is required to complete the current field.
    Incomplete,

    /// Headers have been fully parsed and can be validated.
    HeadersDone,

    /// The full request has been parsed.
    Done,
}

// Syntaxic parsing errors
#[derive(PartialEq, Debug)]
pub enum ParserError {
    Error,

    /// Limit can be found in the server [`config`](crate::config::ServerConfig::max_uri_size)
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

    fn find_delimiter(&self, pattern: &[u8]) -> Option<usize> {
        self.buf[..self.buf_len]
            .windows(pattern.len())
            .position(|window| window == pattern)
    }

    fn parse_request_line(&mut self, req: &mut HttpRequest) -> Result<ParserOk, ParserError> {
        let end = self.find_delimiter(b"\r\n");

        let request_line_end = match end {
            Some(idx) => idx,
            None => {
                if self.buf_len > config().max_request_line_size {
                    return Err(ParserError::Error);
                }

                return Ok(ParserOk::Incomplete);
            }
        };

        if request_line_end > config().max_request_line_size {
            return Err(ParserError::Error);
        }

        // Request line: METHOD uri HTTP/VERSION
        let request_line = &self.buf[..request_line_end];
        let parts: Vec<&[u8]> = request_line.split(|&b| b == b' ').collect();
        if parts.len() != 3 {
            return Err(ParserError::Error);
        }

        let method = std::str::from_utf8(parts[0]).map_err(|_| ParserError::Error)?;

        let method_enum = match http_method_from_str(method) {
            HttpMethod::Unknown => return Err(ParserError::Error),
            m => m,
        };

        let uri = std::str::from_utf8(parts[1]).map_err(|_| ParserError::Error)?;
        if uri.len() > config().max_uri_size {
            return Err(ParserError::TooLongUri);
        }

        let version = std::str::from_utf8(parts[2]).map_err(|_| ParserError::Error)?;
        let http_version = version
            .strip_prefix("HTTP/")
            .and_then(|v| v.split_once('.'))
            .ok_or(ParserError::Error)?;

        let (maj, min) = http_version;
        let maj: u8 = maj.parse().map_err(|_| ParserError::Error)?;
        let min: u8 = min.parse().map_err(|_| ParserError::Error)?;

        req.method = method_enum;
        req.uri = uri.to_string();
        req.http_version = (maj, min);

        let consume = request_line_end + 2;
        let remaining = self.buf_len - consume;

        // Successfully parsed request line
        // Update parser state and remove parsed line from bufs
        self.state = ParserState::Headers;
        self.buf.copy_within(consume..self.buf_len, 0);
        self.buf_len = remaining;

        Ok(ParserOk::Ok)
    }

    fn get_header_name(name: &[u8]) -> Result<&str, ParserError> {
        let s = std::str::from_utf8(name).map_err(|_| ParserError::Error)?;
        if s.is_empty() {
            return Err(ParserError::Error);
        }

        // Only allow tchar characters (ASCII letters, digits, and these symbols)
        if !s.bytes().all(|b| {
            b.is_ascii_alphanumeric()
            || b"!#$%&'*+-.^_`|~".contains(&b)
        }) {
            return Err(ParserError::Error);
        }

        Ok(s)
    }

    fn get_header_value(value: &[u8]) -> Result<&str, ParserError> {
        let s = std::str::from_utf8(value).map_err(|_| ParserError::Error)?;

        // No control characters except HTAB (0x09)
        if s.bytes().any(|b| (b < 0x20 && b != 0x09) || b == 0x7F) {
            return Err(ParserError::Error);
        }

        Ok(s.trim())
    }

    fn parse_headers(&mut self, req: &mut HttpRequest) -> Result<ParserOk, ParserError> {
        let headers_end = self.find_delimiter(b"\r\n\r\n");
        let next_line_end = self.find_delimiter(b"\r\n");

        if next_line_end.is_none() {
            if self.buf_len == PARSER_BUF_CAP {
                return Err(ParserError::Error);
            }
            return Ok(ParserOk::Incomplete);
        }

        let bytes_to_consume = if let Some(end) = headers_end {
            end + 4
        } else {
            next_line_end.unwrap() + 2
        };

        self.headers_bytes_parsed += bytes_to_consume;
        if self.headers_bytes_parsed > config().max_header_size {
            return Err(ParserError::Error);
        }
        // Parse headers line by line
        let headers_chunk = &self.buf[..bytes_to_consume];
        let mut is_header_end = false;
        for raw_line in headers_chunk.split(|&b| b == b'\n') {
            if raw_line.is_empty() {
                continue;
            }

            let line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
            if line.is_empty() {
                is_header_end = true;
                break;
            }

            let mut parts = line.splitn(2, |&b| b == b':');
            let name = parts.next().unwrap();
            let value = parts.next().ok_or(ParserError::Error)?;

            let name = Self::get_header_name(name)?;
            let value = Self::get_header_value(value)?;

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

        let remaining = self.buf_len - bytes_to_consume;
        // Successfully parsed headers
        // Update parser state and remove parsed headers from bufs
        self.buf.copy_within(bytes_to_consume..self.buf_len, 0);
        self.buf_len = remaining;

        if headers_end.is_none() && !is_header_end {
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

    // Helper for the tests to work without server context.
    fn fill_buffer(&mut self, buf: &[u8]) -> Result<(), ParserError> {
        if self.buf_len + buf.len() > PARSER_BUF_CAP {
            return Err(ParserError::Error);
        }

        self.buf[self.buf_len..self.buf_len + buf.len()].copy_from_slice(buf);
        self.buf_len += buf.len();
        Ok(())
    }

    /// Incremental parsing of the HTTP request
    pub fn feed(&mut self, buf: &[u8], req: &mut HttpRequest) -> Result<ParserOk, ParserError> {
        self.fill_buffer(buf)?;

        loop {
            let outcome = match self.state {
                ParserState::RequestLine => self.parse_request_line(req)?,
                ParserState::Headers => match self.parse_headers(req)? {
                    ParserOk::Ok => ParserOk::HeadersDone, // send signal for headers validation
                    other_outcome => other_outcome,
                },
                ParserState::Body => self.parse_body(req)?,
                ParserState::Done => return Ok(ParserOk::Done),
            };

            if outcome == ParserOk::Incomplete || outcome == ParserOk::HeadersDone {
                return Ok(outcome);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;
    use crate::http::request::HttpRequest;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn setup() {
        INIT.call_once(|| {
            let cfg = ServerConfig {
                max_uri_size: 1024,
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
        parsing_method: F,
    ) -> Result<ParserOk, ParserError>
    where
        F: Fn(&mut Parser, &mut HttpRequest) -> Result<ParserOk, ParserError>,
    {
        let mut offset = 0;
        loop {
            if offset >= data.len() {
                break;
            }
            let chunk_size = std::cmp::min(4, data.len() - offset);
            let chunk = &data[offset..offset + chunk_size];
            offset += chunk_size;

            parser.fill_buffer(chunk).unwrap();
            let result = parsing_method(parser, req)?;
            if result != ParserOk::Incomplete {
                return Ok(result);
            }
        }
        Ok(ParserOk::Incomplete)
    }

    // --------------------------
    // Request Line Tests
    // --------------------------
    mod request_line {
        use super::*;

        #[test]
        fn valid_line() {
            run_test(|parser, req| {
                let line = b"GET /index.html HTTP/1.1\r\n";
                parser.fill_buffer(line).unwrap();
                let r = parser.parse_request_line(req).unwrap();
                assert_eq!(r, ParserOk::Ok);
                assert_eq!(req.method, HttpMethod::Get);
                assert_eq!(req.uri, "/index.html");
                assert_eq!(req.http_version, (1, 1));
            });
        }

        #[test]
        fn bad_method() {
            run_test(|parser, req| {
                let line = b"BADMETHOD /index.html HTTP/1.1\r\n";
                parser.fill_buffer(line).unwrap();
                let r = parser.parse_request_line(req);
                assert_eq!(r, Err(ParserError::Error));
            });
        }

        #[test]
        fn too_long_uri() {
            run_test(|parser, req| {
                let long_uri = "a".repeat(config().max_uri_size + 1);
                let line = format!("GET /{} HTTP/1.1\r\n", long_uri);
                let r =
                    parse_iteratively(parser, req, line.as_bytes(), |p, r| p.parse_request_line(r));
                assert_eq!(r, Err(ParserError::TooLongUri));
            });
        }

        #[test]
        fn bad_http_version() {
            run_test(|parser, req| {
                let line = b"GET /index.html HTTP/XYZ\r\n";
                parser.fill_buffer(line).unwrap();
                assert_eq!(parser.parse_request_line(req), Err(ParserError::Error));
            });
        }

        #[test]
        fn incomplete_line() {
            run_test(|parser, req| {
                let line = b"GET /incomplete";
                parser.fill_buffer(line).unwrap();
                let r = parser.parse_request_line(req).unwrap();
                assert_eq!(r, ParserOk::Incomplete);
            });
        }

        #[test]
        fn fragmented_line() {
            run_test(|parser, req| {
                let line = b"GET /frag HTTP/1.1\r\n";
                let r =
                    parse_iteratively(parser, req, line, |p, r| p.parse_request_line(r)).unwrap();
                assert_eq!(r, ParserOk::Ok);
                assert_eq!(req.uri, "/frag");
            });
        }
    }

    // --------------------------
    // Headers Tests
    // --------------------------
    mod headers {
        use super::*;

        #[test]
        fn valid_headers() {
            run_test(|parser, req| {
                let headers = b"Host: example.com\r\nContent-Length: 5\r\n\r\n";
                parser.fill_buffer(headers).unwrap();
                let r = parser.parse_headers(req).unwrap();
                assert_eq!(r, ParserOk::Ok);
                assert_eq!(req.headers.get("Host").unwrap(), "example.com");
                assert_eq!(req.headers.get("Content-Length").unwrap(), "5");
            });
        }

        #[test]
        fn header_too_long() {
            run_test(|parser, req| {
                let long_header = format!(
                    "X-Header: {}\r\n\r\n",
                    "a".repeat(config().max_header_size + 1)
                );
                let r = parse_iteratively(parser, req, long_header.as_bytes(), |p, r| {
                    p.parse_headers(r)
                });
                assert_eq!(r, Err(ParserError::Error));
            });
        }

        #[test]
        fn malformed_header() {
            run_test(|parser, req| {
                let header = b"BadHeaderWithoutColon\r\n\r\n";
                parser.fill_buffer(header).unwrap();
                assert_eq!(parser.parse_headers(req), Err(ParserError::Error));
            });
        }

        #[test]
        fn fragmented_header() {
            run_test(|parser, req| {
                let header = b"Host: ex";
                let r = parse_iteratively(parser, req, header, |p, r| p.parse_headers(r)).unwrap();
                assert_eq!(r, ParserOk::Incomplete);

                let header2 = b"ample.com\r\nContent-Length: 5\r\n\r\n";
                let r = parse_iteratively(parser, req, header2, |p, r| p.parse_headers(r)).unwrap();
                assert_eq!(r, ParserOk::Ok);
            });
        }
    }

    // --------------------------
    // Body Tests
    // --------------------------
    mod body {
        use super::*;

        #[test]
        fn valid_body() {
            run_test(|parser, req| {
                req.set_header(RequestHeader::ContentLength, "5");
                let body = b"Hello";
                parser.fill_buffer(body).unwrap();
                let r = parser.parse_body(req).unwrap();
                assert_eq!(r, ParserOk::Done);
                assert_eq!(req.body, b"Hello");
            });
        }

        #[test]
        fn no_content_lenggth() {
            run_test(|parser, req| {
                let body = b"Hello";
                parser.fill_buffer(body).unwrap();
                let r = parser.parse_body(req).unwrap();
                assert_eq!(r, ParserOk::Done);
                assert!(req.body.is_empty())
            });
        }

        #[test]
        fn incomplete_body() {
            run_test(|parser, req| {
                req.set_header(RequestHeader::ContentLength, "10");
                let body = b"Hello";
                parser.fill_buffer(body).unwrap();
                let r = parser.parse_body(req).unwrap();
                assert_eq!(r, ParserOk::Incomplete);
                assert_eq!(req.body, b"Hello");
            });
        }

        #[test]
        fn too_large_body() {
            run_test(|parser, req| {
                req.set_header(
                    RequestHeader::ContentLength,
                    &(config().max_body_size + 1).to_string(),
                );
                let body = vec![b'a'; config().max_body_size + 1];
                let r = parse_iteratively(parser, req, body.as_slice(), |p, r| p.parse_body(r));
                assert_eq!(r, Err(ParserError::Error));
            });
        }

        #[test]
        fn fragmented_body() {
            run_test(|parser, req| {
                req.set_header(RequestHeader::ContentLength, "5");
                let body = b"He";
                let r = parse_iteratively(parser, req, body, |p, r| p.parse_body(r)).unwrap();
                assert_eq!(r, ParserOk::Incomplete);

                let body2 = b"llo";
                let r2 = parse_iteratively(parser, req, body2, |p, r| p.parse_body(r)).unwrap();
                assert_eq!(r2, ParserOk::Done);
                assert_eq!(req.body, b"Hello");
            });
        }
    }

    // --------------------------
    // Integration / feed Tests
    // --------------------------
    mod feed {
        use super::*;

        #[test]
        fn complete_request_in_chunks() {
            run_test(|parser, req| {
                let request = b"GET /index.html HTTP/1.1\r\nHost: example.com\r\nContent-Length: 5\r\n\r\nHello";
                let mut offset = 0;
                while offset < request.len() {
                    let chunk_size = 4;
                    let end = std::cmp::min(offset + chunk_size, request.len());
                    let chunk = &request[offset..end];
                    offset = end;
                    let r = parser.feed(chunk, req).unwrap();
                    if offset < request.len() {
                        assert!(matches!(r, ParserOk::Incomplete | ParserOk::HeadersDone));
                    } else {
                        assert_eq!(r, ParserOk::Done);
                    }
                }

                assert_eq!(req.method, HttpMethod::Get);
                assert_eq!(req.uri, "/index.html");
                println!("{:?}", std::str::from_utf8(&req.body));
                assert_eq!(req.body, b"Hello");
            });
        }
    }
}
