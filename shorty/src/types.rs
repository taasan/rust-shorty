use core::fmt;

use rusqlite::{
    types::{FromSql, FromSqlError, FromSqlResult, ToSqlOutput, ValueRef},
    ToSql,
};

#[derive(Debug, Clone, Copy)]
pub struct InvalidShortUrlName;

impl fmt::Display for InvalidShortUrlName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Invalid short URL name")
    }
}

impl core::error::Error for InvalidShortUrlName {}

impl From<FromSqlError> for InvalidShortUrlName {
    fn from(_: FromSqlError) -> Self {
        Self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InvalidUrl;

impl fmt::Display for InvalidUrl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Invalid short URL")
    }
}

impl core::error::Error for InvalidUrl {}

impl From<FromSqlError> for InvalidUrl {
    fn from(_: FromSqlError) -> Self {
        Self
    }
}

impl From<url::ParseError> for InvalidUrl {
    fn from(_: url::ParseError) -> Self {
        Self
    }
}

#[derive(Debug, Clone, Eq)]
pub struct ShortUrlName(String);

impl PartialEq for ShortUrlName {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }
}

impl fmt::Display for ShortUrlName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ShortUrlName {
    pub const MIN_LENGTH: usize = 2;
    pub const MAX_LENGTH: usize = 16;
}

impl TryFrom<&str> for ShortUrlName {
    type Error = InvalidShortUrlName;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if (Self::MIN_LENGTH..=Self::MAX_LENGTH).contains(&value.len())
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            Ok(Self(value.to_string()))
        } else {
            Err(InvalidShortUrlName)
        }
    }
}

impl AsRef<str> for ShortUrlName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl FromSql for ShortUrlName {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Self::try_from(value.as_str()?).map_or_else(|_| Err(FromSqlError::InvalidType), Ok)
    }
}

impl ToSql for ShortUrlName {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.0.as_str()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url(url::Url);

impl fmt::Display for Url {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn is_http_or_https(url: &url::Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

fn has_password(url: &url::Url) -> bool {
    url.password().is_some()
}

fn has_username(url: &url::Url) -> bool {
    !url.username().is_empty()
}

impl TryFrom<url::Url> for Url {
    type Error = InvalidUrl;

    fn try_from(url: url::Url) -> Result<Self, Self::Error> {
        if is_http_or_https(&url) && !has_password(&url) && !has_username(&url) {
            Ok(Self(url))
        } else {
            Err(InvalidUrl)
        }
    }
}

impl TryFrom<&str> for Url {
    type Error = InvalidUrl;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        url::Url::parse(s)?.try_into()
    }
}

impl FromSql for Url {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let url = value.as_str()?;
        url::Url::parse(url)
            .map_err(|_| FromSqlError::InvalidType)
            .and_then(|url| url.try_into().map_err(|_| FromSqlError::InvalidType))
    }
}

impl ToSql for Url {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.0.as_str()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortUrl {
    pub name: ShortUrlName,
    pub url: Url,
}

impl fmt::Display for ShortUrl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} -> {}", self.name, self.url)
    }
}

impl ShortUrl {
    #[must_use]
    pub const fn new(name: ShortUrlName, url: Url) -> Self {
        Self { name, url }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum InvalidShortUrl {
    InvalidName(InvalidShortUrlName),
    InvalidUrl(InvalidUrl),
}

impl fmt::Display for InvalidShortUrl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::InvalidName(e) => write!(f, "Invalid short URL name: {e}"),
            Self::InvalidUrl(e) => write!(f, "Invalid short URL: {e}"),
        }
    }
}

impl core::error::Error for InvalidShortUrl {}

impl From<InvalidShortUrlName> for InvalidShortUrl {
    fn from(value: InvalidShortUrlName) -> Self {
        Self::InvalidName(value)
    }
}

impl From<InvalidUrl> for InvalidShortUrl {
    fn from(value: InvalidUrl) -> Self {
        Self::InvalidUrl(value)
    }
}

impl<N: AsRef<str>, U: AsRef<str>> TryFrom<(N, U)> for ShortUrl {
    type Error = InvalidShortUrl;

    fn try_from((name, url): (N, U)) -> Result<Self, Self::Error> {
        Ok(Self::new(
            ShortUrlName::try_from(name.as_ref())?,
            Url::try_from(url.as_ref())?,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct Quotation {
    source: String,
    quote: String,
}

impl Quotation {
    #[must_use]
    pub const fn new(source: String, quote: String) -> Self {
        Self { source, quote }
    }

    #[must_use]
    pub const fn source(&self) -> &String {
        &self.source
    }

    #[must_use]
    pub const fn quote(&self) -> &String {
        &self.quote
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_short_url_name_try_from_too_short() {
        let result = ShortUrlName::try_from("a");
        assert!(result.is_err());
    }

    #[test]
    fn test_short_url_name_try_from_too_long() {
        let result = ShortUrlName::try_from("a".repeat(17).as_str());
        assert!(result.is_err());
    }

    #[test]
    fn test_short_url_name_try_from_invalid_chars() {
        let result = ShortUrlName::try_from("abc$");
        assert!(result.is_err());
    }

    #[test]
    fn test_short_url_name_try_from_valid() {
        let result = ShortUrlName::try_from("-abc_");
        assert!(result.is_ok());
    }

    #[test]
    fn test_url_try_from_valid() {
        let result = Url::try_from("http://localhost/");
        assert!(result.is_ok());
    }

    #[test]
    fn test_url_try_from_invalid_scheme() {
        let result = Url::try_from("ftp://localhost/");
        assert!(result.is_err());
    }

    #[test]
    fn test_url_with_username() {
        let result = Url::try_from("http://user@localhost/");
        assert!(result.is_err());
    }

    #[test]
    fn test_url_with_password() {
        let result = Url::try_from("http://:pass@localhost/");
        assert!(result.is_err());
    }
}