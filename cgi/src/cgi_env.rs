use core::{
    fmt::{self, Debug},
    str::FromStr,
};
use http::uri::InvalidUri;
use serde::{Deserialize, Serialize};
use serde_plain::{derive_display_from_serialize, derive_fromstr_from_deserialize};
use std::{collections::BTreeMap, ffi::OsString};

pub trait Environment {
    fn vars(&self) -> impl Iterator<Item = (OsString, OsString)>;
    fn var(&self, key: String) -> Option<String>;
}

#[derive(Debug)]
pub struct OsEnvironment;

#[allow(clippy::disallowed_methods)]
impl Environment for OsEnvironment {
    fn vars(&self) -> impl Iterator<Item = (OsString, OsString)> {
        std::env::vars_os()
    }

    fn var(&self, key: String) -> Option<String> {
        std::env::var(key).ok()
    }
}

#[derive(Clone)]
pub struct CgiEnv<E> {
    env: E,
}

impl<E> Debug for CgiEnv<E>
where
    E: Environment,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let vars: BTreeMap<_, _> = self.env.vars().collect();
        let cgi_vars: BTreeMap<_, _> = vars
            .iter()
            .filter(|(k, _)| {
                k.to_str()
                    .is_some_and(|k| MetaVariableKind::from_str(k).is_ok())
            })
            .collect();
        f.debug_struct("CgiEnv")
            .field("os_env", &vars)
            .field("cgi_env", &cgi_vars)
            .finish()
    }
}

#[derive(Debug)]
pub enum CgiEnvError {
    InvalidMetaVariable(MetaVariableKind),
    HttpError(http::Error),
    InvalidUrl(InvalidUri),
}

impl core::error::Error for CgiEnvError {}

impl fmt::Display for CgiEnvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMetaVariable(x) => {
                f.debug_tuple("CgiEnvError::InvalidMetaVariable")
                    .field(x)
                    .finish()?;
            }
            Self::HttpError(x) => {
                f.debug_tuple("CgiEnvError::HttpError").field(x).finish()?;
            }
            Self::InvalidUrl(x) => {
                f.debug_tuple("CgiEnvError::InvalidUrl").field(x).finish()?;
            }
        }

        Ok(())
    }
}

impl From<http::Error> for CgiEnvError {
    fn from(value: http::Error) -> Self {
        Self::HttpError(value)
    }
}

impl From<InvalidUri> for CgiEnvError {
    fn from(value: InvalidUri) -> Self {
        Self::InvalidUrl(value)
    }
}

#[derive(Debug, Clone)]
pub struct PathInfo(String);

impl AsRef<str> for PathInfo {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// Not meant to be a generic solution. Just a simple implementation.
// This works for Apache with suexec cgi.
impl<E> CgiEnv<E>
where
    E: Environment,
{
    #[must_use]
    pub const fn new(env: E) -> Self {
        Self { env }
    }

    /// # Errors
    /// If required environment variables are missing or if the url cannot be built.
    pub fn new_request(&self) -> Result<http::Request<()>, CgiEnvError> {
        use http::version::Version;
        use MetaVariableKind::{
            RequestMethod, RequestScheme, RequestUri, ServerName, ServerProtocol,
        };

        #[allow(clippy::option_if_let_else)]
        let protocol_version = match self.getenv(ServerProtocol) {
            Some(x) => match x.as_str() {
                "HTTP/0.9" => Ok(Version::HTTP_09),
                "HTTP/1.0" => Ok(Version::HTTP_10),
                "HTTP/1.1" => Ok(Version::HTTP_11),
                "HTTP/2.0" => Ok(Version::HTTP_2),
                "HTTP/3.0" => Ok(Version::HTTP_3),
                _ => Err(CgiEnvError::InvalidMetaVariable(ServerProtocol)),
            },
            None => Err(CgiEnvError::InvalidMetaVariable(ServerProtocol)),
        }?;
        let host = self.try_getenv(ServerName)?;
        let request_uri = self.try_getenv(RequestUri)?;
        let scheme = self.try_getenv(RequestScheme)?;

        let headers = self.http_headers();
        let url = http::Uri::from_maybe_shared(format!("{scheme}://{host}{request_uri}"))?;
        let mut req = http::Request::builder()
            .method(self.try_getenv(RequestMethod)?.as_str())
            .uri(url.to_string())
            .version(protocol_version)
            .extension(PathInfo(
                self.getenv(MetaVariableKind::PathInfo).unwrap_or_default(),
            ))
            .body(())?;
        req.headers_mut().extend(headers);
        Ok(req)
    }

    pub fn is_cgi(&self) -> bool {
        self.getenv(MetaVariableKind::GatewayInterface).is_some()
    }

    // Environment
    #[must_use]
    pub fn getenv(&self, key: MetaVariableKind) -> Option<String> {
        self.env.var(key.to_string())
    }

    fn try_getenv(&self, key: MetaVariableKind) -> Result<String, CgiEnvError> {
        self.getenv(key)
            .ok_or(CgiEnvError::InvalidMetaVariable(key))
    }

    fn http_headers(&self) -> http::HeaderMap<http::HeaderValue> {
        self.env
            .vars()
            .filter_map(|(k, v)| {
                if let Ok((k, v)) = k.into_string().and_then(|k| Ok((k, v.into_string()?))) {
                    let prefix = "HTTP_";
                    if k.starts_with(prefix) {
                        let k: String = k.chars().skip(prefix.len()).collect();
                        let k = k.replace('_', "-");
                        Some((k.try_into().ok()?, v.try_into().ok()?))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }
}

// https://datatracker.ietf.org/doc/html/rfc3875#section-4.1
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MetaVariableKind {
    AuthType,
    ContentLength,
    ContentType,
    GatewayInterface,
    PathInfo,
    PathTranslated,
    QueryString,
    RemoteAddr,
    RemoteHost,
    RemoteUser,
    RequestIdent,
    RequestMethod,
    ScriptName,
    ServerName,
    ServerPort,
    ServerProtocol,
    ServerSoftware,

    // Apache suexec safe variables
    ContextDocumentRoot,
    ContextPrefix,
    DateGmt,
    DateLocal,
    DocumentName,
    DocumentPathInfo,
    DocumentRoot,
    DocumentUri,
    Https,
    LastModified,
    Path,
    QueryStringUnescaped,
    RedirectErrorNotes,
    RedirectHandler,
    RedirectQueryString,
    RedirectRemoteUser,
    RedirectScriptFilename,
    RedirectStatus,
    RedirectUrl,
    RemoteIdent,
    RemotePort,
    RequestScheme,
    RequestUri,
    ScriptFilename,
    ScriptUri,
    ScriptUrl,
    ServerAddr,
    ServerAdmin,
    ServerSignature,
    Tz,
    UniqueId,
    UserName,
}

derive_fromstr_from_deserialize!(MetaVariableKind);
derive_display_from_serialize!(MetaVariableKind);

#[cfg(test)]
mod tests {
    use super::*;
    use http::Method;
    use std::collections::HashMap;

    #[derive(Debug)]
    struct TestEnvironment {
        vars: HashMap<OsString, OsString>,
    }

    impl Environment for TestEnvironment {
        fn vars(&self) -> impl Iterator<Item = (OsString, OsString)> {
            self.vars.iter().map(|(k, v)| (k.clone(), v.clone()))
        }

        fn var(&self, key: String) -> Option<String> {
            self.vars
                .get(&OsString::from(key))
                .and_then(|v| v.clone().into_string().ok())
        }
    }

    impl Default for TestEnvironment {
        fn default() -> Self {
            let vars: HashMap<OsString, OsString> = vec![
                ("PATH_INFO".into(), "/path".into()),
                ("REQUEST_METHOD".into(), "GET".into()),
                ("REQUEST_SCHEME".into(), "http".into()),
                ("REQUEST_URI".into(), "/test".into()),
                ("SERVER_NAME".into(), "localhost".into()),
                ("SERVER_PROTOCOL".into(), "HTTP/1.1".into()),
                ("HTTP_TEST_HEADER".into(), "test_value".into()),
            ]
            .into_iter()
            .collect();
            Self { vars }
        }
    }

    fn environ() -> CgiEnv<TestEnvironment> {
        let env = TestEnvironment::default();
        CgiEnv::new(env)
    }

    fn empty_environ() -> CgiEnv<TestEnvironment> {
        CgiEnv::new(TestEnvironment {
            vars: HashMap::new(),
        })
    }

    #[test]
    fn test_new_request_success() {
        let result = environ().new_request();
        // assert_eq!(format!("{result:?}"), "akjsdkasj");
        assert!(result.is_ok());
        let req = result.unwrap();
        assert_eq!(req.method(), Method::GET);
        assert_eq!(req.uri(), "http://localhost/test");
        assert_eq!(req.version(), http::Version::HTTP_11);
        assert_eq!(req.headers().get("TEST-HEADER").unwrap(), "test_value");
        assert_eq!(
            req.extensions().get::<PathInfo>().unwrap().as_ref(),
            "/path"
        );
    }

    #[test]
    fn test_new_request_missing_env_vars() {
        let result = empty_environ().new_request();
        assert!(result.is_err());
    }

    #[test]
    fn test_new_request_invalid_protocol() {
        let mut env = environ();
        env.env
            .vars
            .insert("SERVER_PROTOCOL".into(), "INVALID_PROTOCOL".into());

        let result = env.new_request();
        assert!(result.is_err());
    }
}
