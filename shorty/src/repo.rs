use std::path::Path;

use crate::types::{InvalidShortUrlName, ShortUrl, ShortUrlName};
use rusqlite::{Connection, OpenFlags, OptionalExtension};

#[derive(Debug)]
pub struct Repo {
    conn: Connection,
}

#[derive(Debug)]
pub struct SqlError(rusqlite::Error);

impl core::fmt::Display for SqlError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "SqlError: {}", self.0)
    }
}

impl core::error::Error for SqlError {}

impl From<rusqlite::Error> for SqlError {
    fn from(e: rusqlite::Error) -> Self {
        Self(e)
    }
}

#[derive(Debug)]
pub enum RepoError {
    Sql(SqlError),
    InvalidUrl(url::ParseError),
    InvalidShortUrlName(InvalidShortUrlName),
}

impl core::fmt::Display for RepoError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::Sql(e) => write!(f, "RepoError: {e}"),
            Self::InvalidUrl(e) => write!(f, "RepoError: {e}"),
            Self::InvalidShortUrlName(e) => write!(f, "RepoError: {e}"),
        }
    }
}
impl core::error::Error for RepoError {}

impl From<rusqlite::Error> for RepoError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sql(e.into())
    }
}

impl From<url::ParseError> for RepoError {
    fn from(e: url::ParseError) -> Self {
        Self::InvalidUrl(e)
    }
}

type Result<T> = ::core::result::Result<T, RepoError>;

impl Repo {
    pub const fn new(conn: rusqlite::Connection) -> Self {
        Self { conn }
    }

    #[cfg(test)]
    #[allow(clippy::missing_errors_doc)]
    #[must_use]
    pub fn open_in_memory() -> Self {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r"
            CREATE TABLE IF NOT EXISTS urls (
                shortUrl TEXT PRIMARY KEY COLLATE NOCASE,
                url TEXT NOT NULL
                CHECK (LENGTH(shortUrl) >= 2)
                CHECK (LENGTH(shortUrl) <= 16)
                CHECK (url LIKE 'https://%' OR url LIKE 'http://%')
            )
            STRICT;

            CREATE TABLE IF NOT EXISTS quotations (
                collection TEXT NOT NULL COLLATE NOCASE,
                quote TEXT NOT NULL
            ) STRICT;

            CREATE UNIQUE INDEX IF NOT EXISTS collection_quote ON quotations(collection, quote);
            ",
        )
        .unwrap();

        Self::new(conn)
    }

    #[allow(clippy::missing_errors_doc)]
    pub fn open<P: AsRef<Path>>(path: P) -> rusqlite::Result<Self> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        Ok(Self::new(conn))
    }

    #[allow(clippy::missing_errors_doc)]
    pub fn get_url(&self, id: &ShortUrlName) -> Result<Option<ShortUrl>> {
        let query = "SELECT shortUrl, url FROM urls WHERE shortUrl = ? LIMIT 1";
        Ok(self
            .conn
            .query_row(query, rusqlite::params![id.as_ref()], |row| {
                Ok(ShortUrl::new(row.get(0)?, row.get(1)?))
            })
            .optional()?)
    }

    #[allow(clippy::missing_errors_doc)]
    pub fn insert_url(&self, short_url: &ShortUrl) -> Result<()> {
        let query = "INSERT OR REPLACE INTO urls (shortUrl, url) VALUES (?, ?)";
        self.conn.execute(
            query,
            rusqlite::params![short_url.short_url(), short_url.url()],
        )?;
        Ok(())
    }

    #[allow(clippy::missing_errors_doc)]
    pub fn get_random_quote(&self) -> Result<Option<String>> {
        let query = "SELECT quote FROM quotations ORDER BY RANDOM() LIMIT 1";
        Ok(self
            .conn
            .query_row(query, rusqlite::params![], |row| row.get(0))
            .optional()?)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use url::Url;

    fn repo() -> Repo {
        Repo::open_in_memory()
    }

    #[test]
    fn test_insert_and_get() {
        const NAME: &str = "test";
        let name = ShortUrlName::new(NAME).unwrap();
        let repo = repo();

        // No match returns None
        let result = repo.get_url(&name).unwrap();
        assert!(result.is_none());

        // Fetches newly inserted url
        let short_url = ShortUrl::new(
            name.clone(),
            Url::parse("https://example.com/original").unwrap(),
        );
        repo.insert_url(&short_url).unwrap();
        let result = repo.get_url(short_url.short_url()).unwrap();
        assert_eq!(result, Some(short_url));

        // Udates existing url
        let short_url = ShortUrl::new(name, Url::parse("https://example.com/changed").unwrap());
        repo.insert_url(&short_url).unwrap();
        let result = repo.get_url(short_url.short_url()).unwrap();
        assert_eq!(result, Some(short_url));
    }
}
