pub mod headers;
pub mod parser;
pub mod request;
pub mod response;
pub mod status;
pub mod validator;

const HTTP_METHOD_MAX_LEN: usize = 16;

#[allow(dead_code)]
#[derive(PartialEq, PartialOrd, Debug, Clone)]
pub enum HttpVersion {
    V0_9,
    V1_0,
    V1_1,
    V2_0,
    V3_0,
}

impl HttpVersion {
    pub fn is_valid(v: (u8, u8)) -> Result<HttpVersion, ()> {
        match (v.0, v.1) {
            (0, 9) => Ok(HttpVersion::V0_9),
            (1, 0) => Ok(HttpVersion::V1_0),
            (1, 1) => Ok(HttpVersion::V1_1),
            (2, 0) => Ok(HttpVersion::V2_0),
            (3, 0) => Ok(HttpVersion::V3_0),
            _ => Err(()),
        }
    }
}

#[derive(PartialEq)]
pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Delete,
    Connect,
    Options,
    Trace,
    Unknown,
}

pub fn http_method_from_str(method: &str) -> HttpMethod {
    match method {
        "GET" => HttpMethod::Get,
        "HEAD" => HttpMethod::Head,
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "DELETE" => HttpMethod::Delete,
        "TRACE" => HttpMethod::Trace,
        "OPTIONS" => HttpMethod::Options,
        "CONNECT" => HttpMethod::Connect,
        _ => HttpMethod::Unknown,
    }
}
