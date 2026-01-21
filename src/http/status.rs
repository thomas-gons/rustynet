#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpStatus {
    Ok = 200,

    BadRequest = 400,
    NotFound = 404,
    MethodNotAllowed = 405,
    LengthRequired = 411,
    PayloadTooLarge = 413,
    UriTooLong = 414,

    InternalServerError = 500,
    HttpVersionNotSupported = 505,
}
