use crate::cgi_env::{CgiEnv, Environment};
use sentry::{Breadcrumb, Level};

#[derive(Debug, serde::Deserialize)]
pub struct SentryConfig {
    #[serde(default)]
    pub enabled: bool,
    pub dsn: sentry::types::Dsn,
}

pub fn add_breadcrumb(category: &str, message: String) {
    sentry::add_breadcrumb(Breadcrumb {
        category: Some(category.into()),
        message: Some(message),
        level: Level::Info,
        ..Default::default()
    });
}

pub fn add_request_context<T>(request: &http::Request<T>) {
    sentry::configure_scope(|scope| {
        let mut map = std::collections::BTreeMap::new();
        map.insert(String::from("method"), request.method().to_string().into());
        map.insert(String::from("uri"), request.uri().to_string().into());
        scope.set_context("request", sentry::protocol::Context::Other(map));
    });
}

pub fn add_cgi_context<E: Environment>(cgi_env: &CgiEnv<E>) {
    sentry::configure_scope(|scope| {
        let map: std::collections::BTreeMap<String, _> = cgi_env
            .iter()
            .map(|(k, v)| (k.to_string(), v.into()))
            .collect();
        scope.set_context("cgi_environment", sentry::protocol::Context::Other(map));
    });
}
