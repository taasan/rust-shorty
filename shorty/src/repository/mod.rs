use std::path::Path;

use crate::types::{ShortUrl, ShortUrlName};

mod sqlite;

pub trait Repository {
    /// # Errors
    ///
    /// May return a `RepositoryError` if the migration fails.
    fn migrate(&mut self) -> Result<(), anyhow::Error>;

    /// # Errors
    ///
    /// May return a `RepositoryError` if database communication fail.
    fn get_url(&self, name: &ShortUrlName) -> Result<Option<ShortUrl>, anyhow::Error>;

    /// # Errors
    /// May return a `RepositoryError` if database communication fails.
    fn insert_url(&mut self, short_url: &ShortUrl) -> Result<(), anyhow::Error>;

    /// # Errors
    /// May return a `RepositoryError` if database communication fails.
    fn get_random_quote(&self) -> Result<String, anyhow::Error>;

    /// # Errors
    /// May return a `RepositoryError` if database communication fails.
    fn insert_quotation(&mut self, collection: &str) -> Result<(), anyhow::Error>;
}

/// # Errors
///
/// Will return `Err` if `path` cannot be converted to a C-compatible
/// string or if the underlying SQLite open call fails.
pub fn open_sqlite3_repository<P: AsRef<Path>>(path: P) -> Result<impl Repository, anyhow::Error> {
    sqlite::Sqlite3Repo::open(path)
}

/// # Errors
///
/// Will return `Err` if the underlying SQLite open call fails.
pub fn open_sqlite3_repository_in_memory() -> Result<impl Repository, anyhow::Error> {
    Ok(sqlite::Sqlite3Repo::new(
        rusqlite::Connection::open_in_memory()?,
    ))
}
