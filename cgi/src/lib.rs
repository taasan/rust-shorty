use headers::{ContentType, HeaderMapExt};
use http::{HeaderMap, StatusCode};
use std::time::SystemTime;

pub mod cgi_env;
pub mod controller;
pub mod templates;

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

#[allow(clippy::missing_errors_doc)]
pub fn serialize_response<T>(
    response: http::Response<T>,
    out: &mut impl std::io::Write,
) -> std::io::Result<()>
where
    T: AsRef<[u8]>,
{
    write!(
        out,
        "Status: {} {}\r\n",
        response.status(),
        response.status().canonical_reason().unwrap_or("<none>")
    )?;

    let mut headers = HeaderMap::new();
    headers.typed_insert(headers::Date::from(SystemTime::now()));
    serialize_headers(&headers, out)?;
    serialize_headers(response.headers(), out)?;
    write!(out, "\r\n")?;

    out.write_all(response.body().as_ref())?;
    drop(response);
    Ok(())
}

#[must_use]
pub fn html_response(status_code: StatusCode, body: String) -> http::Response<String> {
    response(status_code, body, ContentType::html())
}

#[must_use]
pub fn text_response(status_code: StatusCode, body: String) -> http::Response<String> {
    response(status_code, body, ContentType::text_utf8())
}

#[must_use]
pub fn response(
    status_code: StatusCode,
    body: String,
    content_type: ContentType,
) -> http::Response<String> {
    let mut response = http::Response::new(body);
    *response.status_mut() = status_code;
    response.headers_mut().typed_insert(content_type);
    response
}
