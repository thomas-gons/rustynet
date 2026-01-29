//! HTTP headers abstraction for [`HttpRequest`](crate::http::request::HttpRequest) and
//! [`HttpResponse`](crate::http::response::HttpResponse)
//!
//! This module provides a low-level abstraction for handling HTTP headers in
//! requests and responses. It supports setting, retrieving, and serializing headers.
//!
//! Headers are stored in an ordered map to preserve insertion order.
//! Both header names and values are stored as raw strings, without validation
//! or restrictions on which headers are allowed.
//!
//! This abstraction does not enforce any HTTP semantics or constraints.
//! Higher-level types such as [`HttpRequest`](crate::http::request::HttpRequest)
//! and [`HttpResponse`](crate::http::response::HttpResponse) are responsible for
//! applying their own rules by wrapping or constraining access to this structure.
//!
//! When required, header values can be validated by the
//! [`validator`](crate::http::validator) module.

use indexmap::IndexMap;

pub struct HttpHeaders {
    headers: IndexMap<String, String>,
}

impl HttpHeaders {
    pub fn new() -> Self {
        Self {
            headers: IndexMap::new(),
        }
    }

    pub fn set_raw(&mut self, name: &str, value: &str) {
        self.headers.insert(name.to_string(), value.to_string());
    }

    pub fn get(&self, name: &str) -> Option<&String> {
        self.headers.get(name)
    }

    pub fn stringify(&self) -> String {
        let mut result = String::new();
        for (name, value) in &self.headers {
            result.push_str(&format!("{}: {}\r\n", name, value));
        }
        result
    }
}
