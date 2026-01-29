use std::fs::File;
use std::io::Read;
use std::io::ErrorKind::*;

use crate::config::config;
use crate::handler::responses;
use crate::http::response::{HttpResponse, ResponseHeader};
use crate::http::status::HttpStatus;

pub fn serve(path: &str) -> HttpResponse {
    let mut response = HttpResponse::new();

    let safe_path = sanitize_path(path);
    let full_path = format!("{}{}", config().static_files_root, safe_path);
    eprintln!("Serving static file: {}", full_path);

    let mut file = match File::open(&full_path) {
        Ok(f) => f,
        Err(err) => match err.kind() {
            NotFound => return responses::not_found(),
            PermissionDenied => return responses::forbidden(),
            _ => return responses::internal_server_error(), 
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
    path // do nothing for now
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
