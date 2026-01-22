use flate2::Compression;
use flate2::write::{DeflateEncoder, GzEncoder};
use std::io::Write;

use crate::http::request::HttpRequest;
use crate::http::response::{HttpResponse, ResponseHeader};

// Algorithm used for body compression as listed in MDN
#[allow(dead_code)]
pub enum CompressionAlgorithm {
    Gzip,
    Compress,
    Deflate,
    Br,
    Zstd,
    Dcb,
    Dcz,
    Identity,
}

impl CompressionAlgorithm {
    pub fn as_str(&self) -> &str {
        match self {
            CompressionAlgorithm::Gzip => "gzip",
            CompressionAlgorithm::Compress => "compress",
            CompressionAlgorithm::Deflate => "deflate",
            CompressionAlgorithm::Br => "br",
            CompressionAlgorithm::Zstd => "zstd",
            CompressionAlgorithm::Dcb => "dcb",
            CompressionAlgorithm::Dcz => "dcz",
            CompressionAlgorithm::Identity => "identity",
        }
    }
}

#[allow(dead_code)]
pub enum CompressionError {
    Io(std::io::Error),
    UnsupportedAlgorithm,
}

pub fn apply(req: &HttpRequest, res: &mut HttpResponse) {
    if req.headers.get("Accept-Encoding").is_none() {
        return;
    }
    match compress_body(res, CompressionAlgorithm::Gzip) {
        Ok(_) => (),
        Err(CompressionError::Io(err)) => eprintln!("Compression IO error: {}", err),
        Err(CompressionError::UnsupportedAlgorithm) => {
            eprintln!("Unsupported compression algorithm")
        }
    }
}

fn compress_body(
    res: &mut HttpResponse,
    algo: CompressionAlgorithm,
) -> Result<(), CompressionError> {
    match algo {
        CompressionAlgorithm::Gzip => {
            let mut e = GzEncoder::new(Vec::new(), Compression::default());
            e.write_all(&res.body).map_err(CompressionError::Io)?;
            res.body = e.finish().map_err(CompressionError::Io)?;
        }
        CompressionAlgorithm::Deflate => {
            let mut e = DeflateEncoder::new(Vec::new(), Compression::default());
            e.write_all(&res.body).map_err(CompressionError::Io)?;
            res.body = e.finish().map_err(CompressionError::Io)?;
        }
        _ => return Err(CompressionError::UnsupportedAlgorithm),
    }

    res.set_header(ResponseHeader::ContentEncoding, algo.as_str());
    res.set_header(ResponseHeader::ContentLength, &res.body.len().to_string());
    Ok(())
}
