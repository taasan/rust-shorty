use std::path::Path;
use thiserror::Error;

use crate::types::{InvalidShortUrlName, ShortUrl, ShortUrlName};

mod sqlite;

#[derive(Error, Debug)]
pub enum RepositoryError {
    #[error("{0:?}")]
    DatabaseError(#[from] rusqlite::Error),

    #[error("invalid short url name")]
    InvalidShortUrlName(InvalidShortUrlName),

    #[error("{0:?}")]
    MigrationError(#[from] rusqlite_migration::Error),
}

pub trait Repository {
    /// # Errors
    ///
    /// May return a `RepositoryError` if the migration fails.
    fn migrate(&mut self) -> Result<(), RepositoryError>;

    /// # Errors
    ///
    /// May return a `RepositoryError` if database communication fail.
    fn get_url(&self, name: &ShortUrlName) -> Result<Option<ShortUrl>, RepositoryError>;

    /// # Errors
    /// May return a `RepositoryError` if database communication fails.
    fn insert_url(&mut self, short_url: &ShortUrl) -> Result<(), RepositoryError>;

    /// # Errors
    /// May return a `RepositoryError` if database communication fails.
    fn get_random_quote(&self) -> Result<String, RepositoryError>;

    /// # Errors
    /// May return a `RepositoryError` if database communication fails.
    fn insert_quotation(&mut self, collection: &str) -> Result<(), RepositoryError>;
}

/// # Errors
///
/// Will return `Err` if `path` cannot be converted to a C-compatible
/// string or if the underlying SQLite open call fails.
pub fn open_sqlite3_repository<P: AsRef<Path>>(
    path: P,
) -> Result<impl Repository, RepositoryError> {
    sqlite::Sqlite3Repo::open(path)
}

/// # Errors
///
/// Will return `Err` if the underlying SQLite open call fails.
pub fn open_sqlite3_repository_in_memory() -> Result<impl Repository, RepositoryError> {
    Ok(sqlite::Sqlite3Repo::new(
        rusqlite::Connection::open_in_memory()?,
    ))
}
