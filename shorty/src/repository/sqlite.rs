use core::result::Result;
use std::path::Path;

use crate::types::{ShortUrl, ShortUrlName, UnixTimestamp};
use anyhow::{anyhow, Context};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior};

use super::{Repository, WritableRepository};

#[derive(Debug)]
pub struct Sqlite3Repo {
    conn: Connection,
}

impl Sqlite3Repo {
    pub(crate) const fn new(conn: rusqlite::Connection) -> Self {
        Self { conn }
    }

    /// # Errors
    ///
    /// Will return `Err` if `path` cannot be converted to a C-compatible
    /// string or if the underlying SQLite open call fails.
    pub(crate) fn open<P: AsRef<Path>>(
        path: P,
        flags: Option<OpenFlags>,
    ) -> Result<Self, anyhow::Error> {
        let conn = Connection::open_with_flags(path, flags.unwrap_or_default())?;
        Ok(Self::new(conn))
    }
}

impl Repository for Sqlite3Repo {
    fn get_url(&self, id: &ShortUrlName) -> Result<Option<ShortUrl>, anyhow::Error> {
        let query = "SELECT shortUrl, url, last_modified FROM urls WHERE shortUrl = ? LIMIT 1";
        match self
            .conn
            .query_row(query, rusqlite::params![id.as_ref()], |row| {
                Ok((
                    row.get::<_, ShortUrlName>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, UnixTimestamp>(2)?,
                ))
            })
            .optional()?
        {
            Some((name, url, last_modified)) => {
                let url = crate::types::Url::try_from(url.as_str())
                    .map_err(anyhow::Error::new)
                    .context(anyhow!("Invalid URL {name}"))?;
                Ok(Some(ShortUrl {
                    name,
                    url,
                    last_modified,
                }))
            }
            None => Ok(None),
        }
    }

    fn get_random_quote(&self) -> Result<String, anyhow::Error> {
        let query = "SELECT quote FROM quotations ORDER BY RANDOM() LIMIT 1";
        Ok(self
            .conn
            .query_row(query, rusqlite::params![], |row| row.get(0))
            .optional()?
            .unwrap_or_else(|| "Don't panic\n    -- Douglas Adams".to_string()))
    }
}

impl WritableRepository for Sqlite3Repo {
    fn migrate(&mut self) -> Result<(), anyhow::Error> {
        // EXCLUSIVE ensures that it starts with an exclusive write lock. No other
        // readers will be allowed. This generally shouldn't be needed if there is
        // a file lock, but might be helpful in cases where cargo's `FileLock`
        // failed.
        let migrations = [
            include_str!("migrations/sqlite/1.up.sql"),
            include_str!("migrations/sqlite/2.up.sql"),
        ];
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

    fn insert_url(&mut self, short_url: &ShortUrl) -> Result<(), anyhow::Error> {
        let query = "INSERT OR REPLACE INTO urls (shortUrl, url) VALUES (?, ?)";
        self.conn
            .execute(query, rusqlite::params![short_url.name, short_url.url])?;
        Ok(())
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
        repository::{Repository, WritableRepository},
        types::{ShortUrl, ShortUrlName, UnixTimestamp},
    };

    fn repo() -> Sqlite3Repo {
        let mut repo = Sqlite3Repo::new(Connection::open_in_memory().unwrap());
        repo.migrate().unwrap();
        repo
    }

    #[test]
    fn test_insert_and_get() {
        let name: ShortUrlName = "test".try_into().unwrap();
        let short_url = ShortUrl {
            name: name.clone(),
            url: "https://example.com".try_into().unwrap(),
            last_modified: UnixTimestamp::default(),
        };
        let mut repo = repo();

        // No match returns None
        let result = repo.get_url(&short_url.name).unwrap();
        assert!(result.is_none());

        // Fetches newly inserted url
        repo.insert_url(&short_url).unwrap();
        let inserted_result = repo.get_url(&name).unwrap();
        assert!(inserted_result.is_some());
        let inserted_result = inserted_result.unwrap();
        assert_ne!(inserted_result.last_modified, short_url.last_modified);
        assert_eq!(inserted_result.name, short_url.name);
        assert_eq!(inserted_result.url, short_url.url);

        // Sleep to make sure we get a new last_modified
        std::thread::sleep(core::time::Duration::from_secs(2));

        // Udates existing url
        let short_url = ShortUrl {
            name: name.clone(),
            url: "https://example.com/changed".try_into().unwrap(),
            last_modified: inserted_result.last_modified,
        };
        repo.insert_url(&short_url).unwrap();
        let result = repo.get_url(&name).unwrap();
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.last_modified > short_url.last_modified);
        assert_eq!(result.name, short_url.name);
        assert_eq!(result.url, short_url.url);
    }
}
