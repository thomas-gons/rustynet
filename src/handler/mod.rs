mod responses;
mod router;
mod static_files;

use crate::http::request::HttpRequest;
use crate::http::response::HttpResponse;
use crate::http::status::HttpStatus;

pub fn handle_request(req: &HttpRequest) -> HttpResponse {
    router::route(req)
}

pub fn handle_error(err: HttpStatus) -> HttpResponse {
    responses::any_error(err)
}
