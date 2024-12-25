use core::str::FromStr;
use serde::{Deserialize, Serialize};
use serde_plain::{derive_display_from_serialize, derive_fromstr_from_deserialize};
use url::Url;

#[derive(Clone)]
pub struct CgiEnv {
    _inner: core::marker::PhantomData<()>,
}

impl core::fmt::Debug for CgiEnv {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_map()
            .entries(std::env::vars_os().filter(|(k, _)| {
                k.to_str()
                    .map_or(false, |k| MetaVariableKind::from_str(k).is_ok())
            }))
            .finish()
    }
}

#[derive(Debug)]
pub enum CgiEnvError {
    InvalidMetaVariable(MetaVariableKind),
    HttpError(http::Error),
    InvalidUrl(url::ParseError),
}

impl core::error::Error for CgiEnvError {}

impl core::fmt::Display for CgiEnvError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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
        f.debug_struct("CgiEnvError").finish()?;

        Ok(())
    }
}

impl From<http::Error> for CgiEnvError {
    fn from(value: http::Error) -> Self {
        Self::HttpError(value)
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
impl CgiEnv {
    #[must_use]
    #[allow(clippy::new_without_default)]
    // FIXME: Singleton
    pub const fn new() -> Self {
        Self {
            _inner: core::marker::PhantomData,
        }
    }
    /// # Errors
    /// If required environment variables are missing or if the url cannot be built.
    pub fn new_request() -> Result<http::Request<()>, CgiEnvError> {
        use http::version::Version;
        use MetaVariableKind::{
            RequestMethod, RequestScheme, RequestUri, ServerName, ServerProtocol,
        };

        #[allow(clippy::option_if_let_else)]
        let protocol_version = match Self::getenv(ServerProtocol) {
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
        let host = Self::getenv(ServerName).ok_or(CgiEnvError::InvalidMetaVariable(ServerName))?;
        let request_uri =
            Self::getenv(RequestUri).ok_or(CgiEnvError::InvalidMetaVariable(RequestUri))?;
        let scheme =
            Self::getenv(RequestScheme).ok_or(CgiEnvError::InvalidMetaVariable(RequestScheme))?;
        #[allow(clippy::from_iter_instead_of_collect)]
        let headers: http::HeaderMap<http::HeaderValue> =
            http::HeaderMap::from_iter(std::env::vars_os().filter_map(|(k, v)| {
                if let Ok((k, v)) = k.into_string().and_then(|k| Ok((k, v.into_string()?))) {
                    if k.starts_with("HTTP_") {
                        let k: String = k.chars().skip(5).collect();
                        let k = k.replace('_', "-");
                        Some((k.as_str().try_into().ok()?, v.try_into().ok()?))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }));

        let url = Url::parse(&format!("{scheme}://{host}{request_uri}"))
            .map_err(CgiEnvError::InvalidUrl)?;
        let mut req = http::Request::builder()
            .method(
                Self::getenv(RequestMethod)
                    .ok_or(CgiEnvError::InvalidMetaVariable(RequestMethod))?
                    .as_str(),
            )
            .uri(url.to_string())
            .version(protocol_version)
            .extension(url)
            .extension(PathInfo(
                Self::getenv(MetaVariableKind::PathInfo).unwrap_or_default(),
            ))
            .body(())?;
        req.headers_mut().extend(headers);
        Ok(req)
    }

    // Environment
    #[must_use]
    pub fn getenv(key: MetaVariableKind) -> Option<String> {
        std::env::var(key.to_string()).ok()
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
    UniqueID,
    UserName,
}

derive_fromstr_from_deserialize!(MetaVariableKind);
derive_display_from_serialize!(MetaVariableKind);
