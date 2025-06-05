use core::fmt;

use rusqlite::{
    types::{FromSql, FromSqlError, FromSqlResult, ToSqlOutput, ValueRef},
    ToSql,
};

#[derive(Debug, Clone, Copy)]
pub struct InvalidShortUrlName;

impl fmt::Display for InvalidShortUrlName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

impl TryFrom<String> for ShortUrlName {
    type Error = InvalidShortUrlName;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
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

impl<'a> From<&'a Url> for &'a url::Url {
    fn from(value: &'a Url) -> Self {
        &value.0
    }
}

impl fmt::Display for Url {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

impl TryFrom<&str> for Url {
    type Error = InvalidUrl;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let url = url::Url::parse(s)?;
        if is_http_or_https(&url) && !has_password(&url) && !has_username(&url) {
            Ok(Self(url))
        } else {
            Err(InvalidUrl)
        }
    }
}

impl TryFrom<String> for Url {
    type Error = InvalidUrl;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

impl FromSql for Url {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let url = value.as_str()?;
        url.try_into().map_err(|_| FromSqlError::InvalidType)
    }
}

impl ToSql for Url {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.0.as_str()))
    }
}

/// Only values at or after unix epoch are valid
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnixTimestamp(pub u64);

impl UnixTimestamp {
    #[must_use]
    pub fn iso8601(self) -> Option<String> {
        let secs: i64 = self.0.try_into().ok()?;
        chrono::DateTime::from_timestamp(secs, 0)
            .map(|x| x.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
    }
}

impl core::fmt::Display for UnixTimestamp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

impl FromSql for UnixTimestamp {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let i64_value = value.as_i64_or_null()?.unwrap_or_default();
        Ok(Self(
            i64_value
                .try_into()
                .map_err(|_| FromSqlError::OutOfRange(i64_value))?,
        ))
    }
}

impl ToSql for UnixTimestamp {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.0.to_string()))
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortUrl {
    pub name: ShortUrlName,
    pub url: Url,
    pub last_modified: Option<UnixTimestamp>,
}

impl fmt::Display for ShortUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.name, self.url)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum InvalidShortUrl {
    InvalidName,
    InvalidUrl,
}

impl fmt::Display for InvalidShortUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName => write!(f, "Invalid short URL name"),
            Self::InvalidUrl => write!(f, "Invalid short URL url"),
        }
    }
}

impl core::error::Error for InvalidShortUrl {}

impl From<InvalidShortUrlName> for InvalidShortUrl {
    fn from(_: InvalidShortUrlName) -> Self {
        Self::InvalidName
    }
}

impl From<InvalidUrl> for InvalidShortUrl {
    fn from(_: InvalidUrl) -> Self {
        Self::InvalidUrl
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
