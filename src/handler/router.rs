use crate::handler::responses;
use crate::handler::static_files;
use crate::http::HttpMethod;
use crate::http::request::HttpRequest;
use crate::http::response::HttpResponse;
use crate::http::status::HttpStatus;

pub fn route(req: &HttpRequest) -> HttpResponse {
    match (&req.method, req.uri.as_str()) {
        (HttpMethod::Get, "/") => responses::welcome(),

        (HttpMethod::Get, _) => static_files::serve(&req.uri),
        _ => responses::any_error(HttpStatus::MethodNotAllowed),
    }
}
