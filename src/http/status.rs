#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpStatus {
    OK = 200,

    BAD_REQUEST = 400,
    NOT_FOUND = 404,
    METHOD_NOT_ALLOWED = 405,
    PAYLOAD_TOO_LARGE = 413,
    URI_TOO_LONG = 414,

    INTERNAL_SERVER_ERROR = 500,
    HTTP_VERSION_NOT_SUPPORTED = 505,
}