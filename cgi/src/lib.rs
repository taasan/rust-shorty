use headers::{ContentType, Header as _, HeaderMapExt};
use http::StatusCode;
use std::time::SystemTime;

use git_version::git_version;

#[cfg(test)]
#[macro_use]
extern crate html5ever;

pub mod cgi_env;
pub mod controller;
#[cfg(feature = "sentry")]
pub mod sentry;
mod templates;

pub const VERSION: &str = git_version!(prefix = "", cargo_prefix = "cargo:", fallback = "unknown");

#[inline]
fn serialize_headers(
    headers: &http::HeaderMap,
    out: &mut impl std::io::Write,
) -> std::io::Result<()> {
    headers.iter().try_for_each(|(k, v)| {
        write!(out, "{k}: ")?;
        out.write_all(v.as_bytes())?;
        out.write_all(b"\r\n")?;
        Ok(())
    })
}

#[derive(Debug)]
pub enum SerializeError {
    Io(std::io::Error),
    ContentTooLarge,
}

impl From<std::io::Error> for SerializeError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

fn create_etag(bytes: &[u8]) -> headers::ETag {
    use xxhash_rust::xxh3::xxh3_64;
    let etag = format!("\"{:x}\"", xxh3_64(bytes));
    etag.parse::<headers::ETag>()
        .expect("Failed to create ETag")
}

#[allow(clippy::missing_errors_doc)]
pub fn serialize_response<T>(
    response: http::Response<T>,
    out: &mut impl std::io::Write,
) -> Result<(), SerializeError>
where
    T: AsRef<[u8]>,
{
    let mut response = response;
    let status = response.status();
    let is_empty_response = status == StatusCode::NO_CONTENT || status == StatusCode::NOT_MODIFIED;
    if (status == StatusCode::OK || status == StatusCode::NOT_FOUND)
        && !response.headers().contains_key(headers::ETag::name())
    {
        let etag = create_etag(response.body().as_ref());
        response.headers_mut().typed_insert(etag);
    }
    write!(out, "Status: {}\r\n", response.status(),)?;
    response
        .headers_mut()
        .typed_insert(headers::Date::from(SystemTime::now()));
    if !is_empty_response {
        let length = response
            .body()
            .as_ref()
            .len()
            .try_into()
            .map_err(|_| SerializeError::ContentTooLarge)?;
        response
            .headers_mut()
            .typed_insert(headers::ContentLength(length));
    }
    serialize_headers(response.headers(), out)?;
    write!(out, "\r\n")?;
    if !is_empty_response {
        out.write_all(response.body().as_ref())?;
    }
    out.flush()?;

    Ok(())
}

#[must_use]
pub fn html_response(status_code: StatusCode, body: String) -> http::Response<String> {
    response(status_code, body, ContentType::html())
}

#[must_use]
pub fn text_response<T: AsRef<str>>(status_code: StatusCode, body: T) -> http::Response<String> {
    response(status_code, body, ContentType::text_utf8())
}

#[must_use]
pub fn response<T: AsRef<str>>(
    status_code: StatusCode,
    body: T,
    content_type: ContentType,
) -> http::Response<String> {
    let mut response = http::Response::new(body.as_ref().to_string());
    *response.status_mut() = status_code;
    response.headers_mut().typed_insert(content_type);
    response
}

#[cfg(test)]
mod test {
    use super::*;
    use http::Response;

    #[test]
    fn test_serialize_response() {
        let response = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/plain")
            .body("Hello, world!")
            .unwrap();
        let out: &mut Vec<_> = &mut Vec::new();
        serialize_response(response, out).unwrap();
        let out = String::from_utf8(out.to_owned()).unwrap();
        assert!(out.contains("Status: 200 OK\r\n"));
        assert!(out.contains("\r\ncontent-length: 13\r\n"));
        assert!(out.contains("\r\ncontent-type: text/plain\r\n"));
        assert!(out.contains("\r\ndate: "));
        assert!(out.contains("\r\netag: "));
        assert!(out.contains("\r\n\r\nHello, world!"));
    }

    #[test]
    fn test_serialize_response_etag_already_present() {
        let response = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/plain")
            .header(headers::ETag::name(), "\"d'oh\"")
            .body("Hello, world!")
            .unwrap();
        let out: &mut Vec<_> = &mut Vec::new();
        serialize_response(response, out).unwrap();
        let out = String::from_utf8(out.to_owned()).unwrap();
        assert!(out.contains("Status: 200 OK\r\n"));
        assert!(out.contains("\r\ncontent-length: 13\r\n"));
        assert!(out.contains("\r\ncontent-type: text/plain\r\n"));
        assert!(out.contains("\r\ndate: "));
        assert!(out.contains("\r\netag: \"d'oh\"\r\n"));
        assert!(out.contains("\r\n\r\nHello, world!"));
    }

    fn test_serialize_response_no_content(status: StatusCode) {
        let body = "Hello, world!";
        let response = Response::builder().status(status).body(body).unwrap();
        let out: &mut Vec<_> = &mut Vec::new();
        serialize_response(response, out).unwrap();
        let out = String::from_utf8(out.to_owned()).unwrap();
        assert!(!out.contains("content-length"));
        assert!(!out.contains(body));
        assert!(out.ends_with("GMT\r\n\r\n"));
    }

    #[test]
    fn test_serialize_response_no_content_204() {
        test_serialize_response_no_content(StatusCode::NO_CONTENT);
    }

    #[test]
    fn test_serialize_response_no_content_304() {
        test_serialize_response_no_content(StatusCode::NOT_MODIFIED);
    }

    #[test]
    fn test_create_etag() {
        create_etag(b"abc");
    }
}
