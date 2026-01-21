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
}

impl Parser {
    pub fn new() -> Self {
        Self {
            buf: [0; PARSER_BUF_CAP],
            buf_len: 0,
            state: ParserState::RequestLine,
        }
    }

    pub fn is_buffer_empty(&self) -> bool {
        self.buf_len == 0
    }

    fn parse_request_line(&mut self, req: &mut HttpRequest) -> Result<ParserOk, ParserError> {
        // Look for end of request line \r\n
        let mut request_line_end: usize = usize::MAX;
        for i in 0..self.buf_len - 1 {
            if self.buf[i] == b'\r' && self.buf[i + 1] == b'\n' {
                request_line_end = i;
                break;
            }
        }

        if request_line_end == usize::MAX {
            return Ok(ParserOk::Incomplete);
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
            return Ok(ParserOk::Incomplete);
        }

        if headers_end > config().max_header_size {
            return Err(ParserError::Error);
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
                _ => {}
            }
        }

        // Adjust headers end to point after \r\n\r\n
        headers_end += 4;
        let remaining = self.buf_len - headers_end;

        // Successfully parsed headers
        // Update parser state and remove parsed headers from bufs
        self.state = ParserState::Body;
        self.buf.copy_within(headers_end..self.buf_len, 0);
        self.buf_len = remaining;
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

        req.body.extend_from_slice(&self.buf[..to_copy]);
        self.buf.copy_within(to_copy..self.buf_len, 0);
        self.buf_len -= to_copy;

        if req.body.len() == content_length {
            self.state = ParserState::Done;
            return Ok(ParserOk::Done);
        }

        Ok(ParserOk::Incomplete)
    }

    pub fn feed(&mut self, buf: &[u8], req: &mut HttpRequest) -> Result<ParserOk, ParserError> {
        if self.state < ParserState::Body && self.buf_len + buf.len() >= PARSER_BUF_CAP {
            return Err(ParserError::Error);
        }

        self.buf[self.buf_len..self.buf_len + buf.len()].copy_from_slice(buf);
        self.buf_len += buf.len();

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
