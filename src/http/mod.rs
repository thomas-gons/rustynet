pub mod headers;
pub mod parser;
pub mod request;
pub mod response;
pub mod status;

const HTTP_METHOD_MAX_LEN: usize = 16;

#[derive(PartialEq)]
pub enum HttpMethod {
    Options,
    Get,
    Head,
    Post,
    Put,
    Delete,
    Trace,
    Connect,
    Patch,
    Unknown,
}

pub fn http_method_from_str(method: &str) -> HttpMethod {
    match method {
        "OPTIONS" => HttpMethod::Options,
        "GET" => HttpMethod::Get,
        "HEAD" => HttpMethod::Head,
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "DELETE" => HttpMethod::Delete,
        "TRACE" => HttpMethod::Trace,
        "CONNECT" => HttpMethod::Connect,
        "PATCH" => HttpMethod::Patch,
        _ => HttpMethod::Unknown,
    }
}
