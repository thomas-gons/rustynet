use crate::handler::responses;
use crate::handler::static_files;
use crate::http::HttpMethod;
use crate::http::request::HttpRequest;
use crate::http::response::HttpResponse;
use crate::http::status::HttpStatus;

pub fn route(req: &HttpRequest) -> HttpResponse {
    match (&req.method, req.path.as_str()) {
        (HttpMethod::Get, "/") => responses::welcome(),

        (HttpMethod::Get, path) if path.starts_with("/static/") => static_files::serve(&req.path),

        (HttpMethod::Get, _) => responses::any_error(HttpStatus::NotFound),

        _ => responses::any_error(HttpStatus::MethodNotAllowed),
    }
}
