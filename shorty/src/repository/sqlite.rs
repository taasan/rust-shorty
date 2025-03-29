use core::result::Result;
use std::path::Path;
use std::sync::OnceLock;

use crate::types::{ShortUrl, ShortUrlName};
use anyhow::{anyhow, Context};
use rusqlite::{Connection, OptionalExtension};
use rusqlite_migration::{Migrations, M};

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

fn migrations() -> Migrations<'static> {
    static MIGRATIONS: OnceLock<Migrations<'_>> = OnceLock::new();
    MIGRATIONS
        .get_or_init(|| {
            Migrations::new(vec![
                M::up(include_str!("migrations/sqlite/1.up.sql"))
                    .down(include_str!("migrations/sqlite/1.down.sql")),
                // In the future, add more migrations here:
            ])
        })
        .clone()
}

impl Repository for Sqlite3Repo {
    fn migrate(&mut self) -> Result<(), anyhow::Error> {
        Ok(migrations().to_latest(&mut self.conn)?)
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
    use crate::{
        repository::{sqlite::migrations, Repository},
        types::ShortUrl,
    };

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

    #[test]
    fn migrations_test() {
        assert!(migrations().validate().is_ok());
    }
}
