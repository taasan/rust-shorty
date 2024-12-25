use core::result::Result;
use std::path::Path;

use crate::types::{ShortUrl, ShortUrlName};
use rusqlite::{Connection, OpenFlags, OptionalExtension};

use super::{Repository, RepositoryError};

#[derive(Debug)]
pub struct Sqlite3Repo {
    conn: Connection,
}

impl Sqlite3Repo {
    pub const fn new(conn: rusqlite::Connection) -> Self {
        Self { conn }
    }

    /// # Errors
    ///
    /// Will return `Err` if `path` cannot be converted to a C-compatible
    /// string or if the underlying SQLite open call fails.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, RepositoryError> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        Ok(Self::new(conn))
    }
}

impl Repository for Sqlite3Repo {
    fn migrate(&self) -> Result<(), RepositoryError> {
        Ok(self.conn.execute_batch(
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
        )?)
    }

    fn get_url(&self, id: &ShortUrlName) -> Result<Option<ShortUrl>, RepositoryError> {
        let query = "SELECT shortUrl, url FROM urls WHERE shortUrl = ? LIMIT 1";
        Ok(self
            .conn
            .query_row(query, rusqlite::params![id.as_ref()], |row| {
                Ok(ShortUrl::new(row.get(0)?, row.get(1)?))
            })
            .optional()?)
    }

    fn insert_url(&self, short_url: &ShortUrl) -> Result<(), RepositoryError> {
        let query = "INSERT OR REPLACE INTO urls (shortUrl, url) VALUES (?, ?)";
        self.conn
            .execute(query, rusqlite::params![short_url.name, short_url.url])?;
        Ok(())
    }

    fn get_random_quote(&self) -> Result<String, RepositoryError> {
        let query = "SELECT quote FROM quotations ORDER BY RANDOM() LIMIT 1";
        Ok(self
            .conn
            .query_row(query, rusqlite::params![], |row| row.get(0))
            .optional()?
            .unwrap_or_else(|| "Don't panic\n    -- Douglas Adams".to_string()))
    }

    fn insert_quotation(&self, collection: &str) -> Result<(), RepositoryError> {
        let query = "INSERT INTO quotations (collection, quote) VALUES (?, ?)";
        self.conn
            .execute(query, rusqlite::params!["default", collection])?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use rusqlite::Connection;

    use super::Sqlite3Repo;
    use crate::{repository::Repository, types::ShortUrl};

    fn repo() -> Sqlite3Repo {
        let repo = Sqlite3Repo::new(Connection::open_in_memory().unwrap());
        repo.migrate().unwrap();
        repo
    }

    #[test]
    fn test_insert_and_get() {
        const NAME: &str = "test";
        let short_url = ShortUrl::try_from((NAME, "https://example.com")).unwrap();
        let repo = repo();

        // No match returns None
        let result = repo.get_url(&short_url.name).unwrap();
        assert!(result.is_none());

        // Fetches newly inserted url
        repo.insert_url(&short_url).unwrap();
        let result = repo.get_url(&short_url.name).unwrap();
        assert_eq!(result, Some(short_url));

        // Udates existing url
        let short_url = ShortUrl::try_from((NAME, "https://example.com/changed")).unwrap();
        repo.insert_url(&short_url).unwrap();
        let result = repo.get_url(&short_url.name).unwrap();
        assert_eq!(result, Some(short_url));
    }
}
