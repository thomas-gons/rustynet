use crate::config::config;
use crate::http::response::{HttpResponse, ResponseHeader};
use crate::http::status::HttpStatus;

pub fn welcome() -> HttpResponse {
    let mut res = HttpResponse::new();
    res.status = HttpStatus::Ok;
    let body = format!("<h1>Welcome to {}!</h1>", config().server_name)
        .as_bytes()
        .to_vec();

    res.set_header(ResponseHeader::ContentLength, &body.len().to_string());
    res.set_header(ResponseHeader::ContentType, "text/html");

    res.body = body;
    res
}

pub fn not_found() -> HttpResponse {
    let mut res = HttpResponse::new();
    res.status = HttpStatus::NotFound;
    let body = b"<h1>404 Not Found</h1>".to_vec();

    res.set_header(ResponseHeader::ContentLength, &body.len().to_string());
    res.set_header(ResponseHeader::ContentType, "text/html");

    res.body = body;
    res
}

pub fn any_error(err: HttpStatus) -> HttpResponse {
    let mut res = HttpResponse::new();
    res.status = err;
    res
}
