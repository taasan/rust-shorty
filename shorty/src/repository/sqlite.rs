use core::result::Result;
use std::path::Path;

use crate::types::{ShortUrl, ShortUrlName, UnixTimestamp, Url};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior};

use super::{Repository, WritableRepository};

#[derive(Debug)]
pub(crate) struct Sqlite3Repo {
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
        let query = "SELECT shortUrl, url, last_modified FROM urls WHERE shortUrl = ?";
        Ok(self
            .conn
            .query_row(query, rusqlite::params![id.as_ref()], |row| {
                Ok(ShortUrl {
                    name: row.get::<_, ShortUrlName>(0)?,
                    url: row.get::<_, Url>(1)?,
                    last_modified: row.get::<_, Option<UnixTimestamp>>(2)?,
                })
            })
            .optional()?)
    }

    fn for_each_short_url(
        &self,
        callback: &dyn Fn(ShortUrl) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let query = "SELECT shorturl, url, last_modified FROM urls";
        let mut stmt = self.conn.prepare(query)?;
        let rows = stmt.query_map([], |row| {
            Ok(ShortUrl {
                name: row.get::<_, ShortUrlName>(0)?,
                url: row.get::<_, Url>(1)?,
                last_modified: row.get::<_, Option<UnixTimestamp>>(2)?,
            })
        })?;
        for row in rows {
            let Ok(row) = row else { continue };
            callback(row)?;
        }
        Ok(())
    }

    fn for_each_name(
        &self,
        callback: &dyn Fn(ShortUrlName) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let query = "SELECT shortUrl FROM urls";
        let mut stmt = self.conn.prepare(query)?;
        let rows = stmt.query_map([], |row| {
            let value: ShortUrlName = row.get(0)?;
            Ok(value)
        })?;
        for row in rows {
            let Ok(row) = row else { continue };
            callback(row)?;
        }
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

    fn has_latest_migrations(&self) -> Result<bool, anyhow::Error> {
        let migrations = migrations();
        let user_version: u32 =
            self.conn
                .query_row("SELECT user_version FROM pragma_user_version", [], |row| {
                    row.get(0)
                })?;
        Ok(user_version as usize == migrations.len())
    }
}

#[inline]
const fn migrations() -> [&'static str; 2] {
    [
        include_str!("migrations/sqlite/1.up.sql"),
        include_str!("migrations/sqlite/2.up.sql"),
    ]
}

impl WritableRepository for Sqlite3Repo {
    fn migrate(&mut self) -> Result<(), anyhow::Error> {
        // EXCLUSIVE ensures that it starts with an exclusive write lock. No other
        // readers will be allowed. This generally shouldn't be needed if there is
        // a file lock, but might be helpful in cases where cargo's `FileLock`
        // failed.
        let migrations = migrations();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Exclusive)?;
        let user_version: u32 =
            tx.query_row("SELECT user_version FROM pragma_user_version", [], |row| {
                row.get(0)
            })?;
        if (user_version as usize) < migrations.len() {
            for migration in &migrations[(user_version as usize)..] {
                tx.execute_batch(migration)?;
            }
            tx.pragma_update(None, "user_version", u32::try_from(migrations.len())?)?;
        }
        tx.commit()?;
        Ok(())
    }

    fn insert_url(
        &mut self,
        name: &ShortUrlName,
        url: &crate::types::Url,
    ) -> Result<(), anyhow::Error> {
        let query = "INSERT INTO urls (shorturl, url) VALUES (?1, ?2) ON CONFLICT(shorturl) DO UPDATE SET url = excluded.url";
        self.conn.execute(query, rusqlite::params![name, url])?;
        Ok(())
    }

    fn insert_quotation(&mut self, collection: &str) -> Result<(), anyhow::Error> {
        let query = "INSERT INTO quotations (collection, quote) VALUES (?, ?)";
        self.conn
            .execute(query, rusqlite::params!["default", collection])?;
        Ok(())
    }
}

/// # Errors
///
/// Will return `Err` if `path` cannot be converted to a C-compatible
/// string or if the underlying SQLite open call fails.
pub fn open_readonly_repository<P: AsRef<Path>>(path: P) -> Result<impl Repository, anyhow::Error> {
    Sqlite3Repo::open(path, Some(OpenFlags::SQLITE_OPEN_READ_ONLY))
}

/// # Errors
///
/// Will return `Err` if `path` cannot be converted to a C-compatible
/// string or if the underlying SQLite open call fails.
pub fn open_writable_repository<P: AsRef<Path>>(
    path: P,
) -> Result<impl WritableRepository, anyhow::Error> {
    Sqlite3Repo::open(path, None)
}

/// # Errors
///
/// Will return `Err` if the underlying SQLite open call fails.
#[doc(hidden)]
pub fn open_writable_in_memory_repository() -> Result<impl WritableRepository, anyhow::Error> {
    Ok(Sqlite3Repo::new(rusqlite::Connection::open_in_memory()?))
}

#[cfg(test)]
mod test {
    use rusqlite::Connection;

    use super::Sqlite3Repo;
    use crate::{
        repository::{Repository, WritableRepository},
        types::{ShortUrl, ShortUrlName},
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
            last_modified: None,
        };
        let mut repo = repo();

        // No match returns None
        let result = repo.get_url(&short_url.name).unwrap();
        assert!(result.is_none());

        // Fetches newly inserted url
        repo.insert_url(&short_url.name, &short_url.url).unwrap();
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
        repo.insert_url(&short_url.name, &short_url.url).unwrap();
        let result = repo.get_url(&name).unwrap();
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.last_modified > short_url.last_modified);
        assert_eq!(result.name, short_url.name);
        assert_eq!(result.url, short_url.url);
    }
}
