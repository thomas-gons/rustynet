use std::fs::File;
use std::io::Read;

use crate::http::response::{HttpResponse, ResponseHeader};
use crate::http::status::HttpStatus;

const PUBLIC_DIR: &str = "./public";

pub fn serve(path: &str) -> HttpResponse {
    let mut response = HttpResponse::new();

    let safe_path = sanitize_path(path);
    let full_path = format!("{}{}", PUBLIC_DIR, safe_path);

    let mut file = match File::open(&full_path) {
        Ok(f) => f,
        Err(_) => {
            response.status = HttpStatus::NotFound;
            return response;
        }
    };

    let mut body = Vec::new();
    if file.read_to_end(&mut body).is_err() {
        response.status = HttpStatus::InternalServerError;
        return response;
    }

    response.set_header(ResponseHeader::ContentLength, &body.len().to_string());
    response.set_header(ResponseHeader::ContentType, guess_mime(&full_path));

    response.body = body;
    response
}

fn sanitize_path(path: &str) -> &str {
    path.strip_prefix("/static").unwrap_or("")
}

fn guess_mime(path: &str) -> &str {
    match path.rsplit('.').next() {
        Some("htm") | Some("html") => "text/html",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("json") => "application/json",
        Some("xml") => "application/xml",
        Some("txt") => "text/plain",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}
