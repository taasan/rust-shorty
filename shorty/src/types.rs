use rusqlite::{
    types::{FromSql, FromSqlError, FromSqlResult, ToSqlOutput, ValueRef},
    ToSql,
};
use url::Url;

#[derive(Debug)]
pub struct InvalidShortUrlName;

impl core::fmt::Display for InvalidShortUrlName {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "Invalid short URL name")
    }
}

impl core::error::Error for InvalidShortUrlName {}

impl From<FromSqlError> for InvalidShortUrlName {
    fn from(_: FromSqlError) -> Self {
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

impl core::fmt::Display for ShortUrlName {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ShortUrlName {
    pub const MIN_LENGTH: usize = 2;
    pub const MAX_LENGTH: usize = 16;

    /// # Errors
    ///
    /// `InvalidShortUrlName` if the name is too long, too short or it
    /// contains characters other than alphanumeric ascii, `-` or `_`.
    pub fn new<T>(name: T) -> Result<Self, InvalidShortUrlName>
    where
        T: AsRef<str>,
    {
        let name = name.as_ref();

        if (Self::MIN_LENGTH..=Self::MAX_LENGTH).contains(&name.len())
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            Ok(Self(name.to_string()))
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
        Self::new(value.as_str()?).map_or_else(|_| Err(FromSqlError::InvalidType), Ok)
    }
}

impl ToSql for ShortUrlName {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.0.as_str()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortUrl {
    short_url: ShortUrlName,
    url: Url,
}

impl ShortUrl {
    #[must_use]
    pub const fn new(short_url: ShortUrlName, url: Url) -> Self {
        Self { short_url, url }
    }

    #[must_use]
    pub const fn short_url(&self) -> &ShortUrlName {
        &self.short_url
    }

    #[must_use]
    pub const fn url(&self) -> &Url {
        &self.url
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
    fn test_short_url_name_new_too_short() {
        let result = ShortUrlName::new("a");
        assert!(result.is_err());
    }

    #[test]
    fn test_short_url_name_new_too_long() {
        let result = ShortUrlName::new("a".repeat(17));
        assert!(result.is_err());
    }

    #[test]
    fn test_short_url_name_new_invalid_chars() {
        let result = ShortUrlName::new("abc$");
        assert!(result.is_err());
    }

    #[test]
    fn test_short_url_name_new_valid() {
        let result = ShortUrlName::new("-abc_");
        assert!(result.is_ok());
    }
}
