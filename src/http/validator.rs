//! HTTP request validator
//! Once all headers have been parsed from an incoming request,
//! this module provides functionality to validate its semantics
//! according to HTTP rules and server configuration.
//! The semantics errors are directly mapped to appropriate HTTP status codes
//! 
//! This includes validating:
//! - HTTP version support
//! - HTTP method constraints (e.g., body presence for POST/PUT)
//! - Content-Length header correctness
//! - Maximum allowed body size

use crate::config::config;
use crate::http::HttpMethod;
use crate::http::HttpVersion;
use crate::http::request::HttpRequest;
use crate::http::status::HttpStatus;

pub enum ValidatorError {
    Error,
    HttpVersionNotSupported,
    PayloadTooLarge,
    MalformedHeaderField,
    MissingContentLength,
    BodyNotAllowed,
    MandatoryBody,
}

impl ValidatorError {

    /// Maps a `ValidatorError` to the corresponding `HttpStatus` code.
    pub fn into_http_status(self) -> HttpStatus {
        match self {
            ValidatorError::Error => HttpStatus::BadRequest,
            ValidatorError::HttpVersionNotSupported => HttpStatus::HttpVersionNotSupported,
            ValidatorError::PayloadTooLarge => HttpStatus::PayloadTooLarge,
            ValidatorError::MalformedHeaderField => HttpStatus::BadRequest,
            ValidatorError::MandatoryBody => HttpStatus::BadRequest,
            ValidatorError::BodyNotAllowed => HttpStatus::BadRequest,
            ValidatorError::MissingContentLength => HttpStatus::LengthRequired,
        }
    }
}

pub struct Validator;

impl Validator {

    /// Validates that the HTTP version is supported by the server configuration. (see [`http_version`](crate::config::ServerConfig::http_version))
    fn validate_http_version(v: (u8, u8)) -> Result<(), ValidatorError> {
        match HttpVersion::is_valid(v) {
            Ok(http_v) => {
                if http_v <= config().http_version {
                    Ok(())
                } else {
                    Err(ValidatorError::HttpVersionNotSupported)
                }
            }
            Err(_) => Err(ValidatorError::Error),
        }
    }

    /// Applies method-specific validation rules.
    /// GET/HEAD must not have a body whereas POST/PUT must have one.
    /// Other methods are not constrained.
    fn validate_http_method(
        content_length: Option<usize>,
        method: &HttpMethod,
    ) -> Result<(), ValidatorError> {
        match method {
            HttpMethod::Get | HttpMethod::Head => match content_length {
                Some(n) if n > 0 => Err(ValidatorError::BodyNotAllowed),
                _ => Ok(()),
            },

            HttpMethod::Post | HttpMethod::Put => match content_length {
                None => Err(ValidatorError::MissingContentLength),
                Some(0) => Err(ValidatorError::MandatoryBody),
                Some(_) => Ok(()),
            },
            _ => Ok(()),
        }
    }

    /// public interface to enforce all validations on an `HttpRequest`
    pub fn validate_request(req: &HttpRequest) -> Result<(), ValidatorError> {
        Self::validate_http_version(req.http_version)?;

        let content_length = req
            .headers
            .get("Content-Length")
            .map(|v| v.parse::<usize>())
            .transpose()
            .map_err(|_| ValidatorError::MalformedHeaderField)?;

        Self::validate_http_method(content_length, &req.method)?;

        if content_length.is_some() && content_length > Some(config().max_body_size) {
            return Err(ValidatorError::PayloadTooLarge);
        }

        Ok(())
    }
}
