use core::result::Result;
use std::path::Path;

use crate::types::{ShortUrl, ShortUrlName};
use anyhow::{anyhow, Context};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use super::Repository;

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
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, anyhow::Error> {
        let conn = Connection::open(path)?;
        Ok(Self::new(conn))
    }
}

impl Repository for Sqlite3Repo {
    fn migrate(&mut self) -> Result<(), anyhow::Error> {
        // EXCLUSIVE ensures that it starts with an exclusive write lock. No other
        // readers will be allowed. This generally shouldn't be needed if there is
        // a file lock, but might be helpful in cases where cargo's `FileLock`
        // failed.
        let migrations = [include_str!("migrations/sqlite/1.up.sql")];
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Exclusive)?;
        let user_version =
            tx.query_row("SELECT user_version FROM pragma_user_version", [], |row| {
                row.get(0)
            })?;
        if user_version < migrations.len() {
            for migration in &migrations[user_version..] {
                tx.execute_batch(migration)?;
            }
            tx.pragma_update(None, "user_version", migrations.len())?;
        }
        tx.commit()?;
        Ok(())
    }

    fn get_url(&self, id: &ShortUrlName) -> Result<Option<ShortUrl>, anyhow::Error> {
        let query = "SELECT shortUrl, url FROM urls WHERE shortUrl = ? LIMIT 1";
        match self
            .conn
            .query_row(query, rusqlite::params![id.as_ref()], |row| {
                Ok((row.get::<_, ShortUrlName>(0)?, row.get::<_, String>(1)?))
            })
            .optional()?
        {
            Some((name, url)) => {
                let url = url::Url::parse(&url)
                    .map_err(anyhow::Error::new)
                    .and_then(|url| Ok(crate::types::Url::try_from(url)?))
                    .context(anyhow!("Invalid URL {name}"))?;
                Ok(Some(ShortUrl::new(name, url)))
            }
            None => Ok(None),
        }
    }

    fn insert_url(&mut self, short_url: &ShortUrl) -> Result<(), anyhow::Error> {
        let query = "INSERT OR REPLACE INTO urls (shortUrl, url) VALUES (?, ?)";
        self.conn
            .execute(query, rusqlite::params![short_url.name, short_url.url])?;
        Ok(())
    }

    fn get_random_quote(&self) -> Result<String, anyhow::Error> {
        let query = "SELECT quote FROM quotations ORDER BY RANDOM() LIMIT 1";
        Ok(self
            .conn
            .query_row(query, rusqlite::params![], |row| row.get(0))
            .optional()?
            .unwrap_or_else(|| "Don't panic\n    -- Douglas Adams".to_string()))
    }

    fn insert_quotation(&mut self, collection: &str) -> Result<(), anyhow::Error> {
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
        let mut repo = Sqlite3Repo::new(Connection::open_in_memory().unwrap());
        repo.migrate().unwrap();
        repo
    }

    #[test]
    fn test_insert_and_get() {
        const NAME: &str = "test";
        let short_url = ShortUrl::try_from((NAME, "https://example.com")).unwrap();
        let mut repo = repo();

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
