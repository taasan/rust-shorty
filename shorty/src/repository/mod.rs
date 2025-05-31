use crate::types::{ShortUrl, ShortUrlName, Url};

pub mod sqlite;

pub trait Repository {
    /// # Errors
    ///
    /// May return a `RepositoryError` if database communication fail.
    fn get_url(&self, name: &ShortUrlName) -> Result<Option<ShortUrl>, anyhow::Error>;

    /// # Errors
    ///
    /// May return a `Error` if database communication fail.
    fn for_each_short_url(
        &self,
        callback: &dyn Fn(ShortUrl) -> anyhow::Result<()>,
    ) -> anyhow::Result<()>;

    // fn for_each_short_url<F>(&self, callback: F) -> anyhow::Result<()>
    // where
    //     F: Fn(ShortUrl) -> anyhow::Result<()>;

    /// # Errors
    ///
    /// May return a `Error` if database communication fail.
    fn for_each_name(
        &self,
        callback: &dyn Fn(ShortUrlName) -> anyhow::Result<()>,
    ) -> anyhow::Result<()>;
    // fn for_each_name<F>(&self, callback: F) -> anyhow::Result<()>
    // where
    //     F: Fn(ShortUrlName) -> anyhow::Result<()>;

    /// # Errors
    /// May return a `RepositoryError` if database communication fails.
    fn get_random_quote(&self) -> Result<String, anyhow::Error>;

    /// # Errors
    /// May return a `RepositoryError` if database communication fails.
    fn has_latest_migrations(&self) -> Result<bool, anyhow::Error>;
}

pub trait WritableRepository: Repository {
    /// # Errors
    ///
    /// May return a `RepositoryError` if the migration fails.
    fn migrate(&mut self) -> Result<(), anyhow::Error>;

    /// # Errors
    /// May return a `RepositoryError` if database communication fails.
    fn insert_url(&mut self, name: &ShortUrlName, url: &Url) -> Result<(), anyhow::Error>;

    /// # Errors
    /// May return a `RepositoryError` if database communication fails.
    fn insert_quotation(&mut self, collection: &str) -> Result<(), anyhow::Error>;
}
