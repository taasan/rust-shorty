use cgi::cgi_env::{CgiEnv, Environment, MetaVariableKind, OsEnvironment, PathInfo};
use cgi::controller::{
    Controller, ErrorController, QuotationController, ShortUrlController, ShortUrlControllerParams,
};
#[cfg(feature = "sentry")]
use cgi::sentry::SentryConfig;
use cgi::{serialize_response, text_response};
use core::fmt;
use core::str::FromStr;
use http::StatusCode;
use matchit::{Match, MatchError, Router};
use shorty::repository::{open_sqlite3_repository, Repository};
use shorty::types::ShortUrlName;
use std::sync::Once;
use std::{env, fs, path::Path, path::PathBuf};

const SHORT_URL_PARAM: &str = "short_url";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    Home,
    ShortUrl,
    Debug,
    ErrorDocument,
}

fn main() -> Result<(), Box<dyn core::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if !matches!(args.len(), 2 | 3) {
        eprintln!("Usage: shorty [--migrate] config.toml");
        return Err("Missing config file argument".into());
    }
    let exe_path = args.last().ok_or("Exe path not found")?;
    let exe_path = fs::canonicalize(exe_path)?;
    let config = read_config(exe_path)?;

    #[cfg(feature = "sentry")]
    let _guard = match &config.sentry {
        Some(SentryConfig { enabled: true, dsn }) => Some(sentry::init((
            dsn,
            sentry::ClientOptions {
                release: Some(cgi::VERSION.into()),
                session_mode: sentry::SessionMode::Request,
                ..Default::default()
            },
        ))),
        _ => None,
    };
    let cgi_env = &CgiEnv::new(OsEnvironment);
    if cgi_env.is_cgi() {
        cgi_main(&config, cgi_env);
    } else if args.len() == 3 && args[1] == *"--migrate" {
        run_migrations(config.database_file)?;
    } else {
        return Err("Unknown command".into());
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct Config {
    pub database_file: PathBuf,
    #[cfg(feature = "sentry")]
    pub sentry: Option<SentryConfig>,
}

fn read_config<P: AsRef<Path>>(path: P) -> Result<Config, anyhow::Error> {
    let content = fs::read_to_string(&path)?;
    let config_start = content.lines().skip(1).collect::<Vec<_>>().join("\n");
    Ok(toml::from_str(&config_start)?)
}

fn run_migrations<P: AsRef<Path>>(path: P) -> Result<(), anyhow::Error> {
    let mut repo = open_sqlite3_repository(path)?;
    repo.migrate()
}

fn setup_cgi() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let next = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            let mut out = std::io::stdout().lock();
            #[allow(clippy::option_if_let_else)]
            let payload = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
                format!("panic occurred: {s:?}")
            } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
                format!("panic occurred: {s:?}")
            } else {
                "<none>".to_string()
            };
            let backtrace = std::backtrace::Backtrace::capture();
            let _ = serialize_response(
                text_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("{panic_info:#?}\n\nPayload: {payload}\n\n{backtrace}"),
                ),
                &mut out,
            );
            next(panic_info);
        }));
    });
}

fn cgi_main<T: fmt::Debug + Environment>(config: &Config, cgi_env: &CgiEnv<T>) {
    setup_cgi();
    let mut out = std::io::stdout().lock();

    #[allow(clippy::unwrap_used)]
    match run(config, cgi_env) {
        Ok(response) => {
            serialize_response(response, &mut out).unwrap();
        }
        Err(err) => {
            #[cfg(feature = "sentry")]
            sentry::integrations::anyhow::capture_anyhow(&err);
            serialize_response(
                ErrorController {}
                    .respond((StatusCode::INTERNAL_SERVER_ERROR, format!("{err:#?}")))
                    .unwrap(),
                &mut out,
            )
            .unwrap();
        }
    }
}

fn run<T: fmt::Debug + Environment>(
    config: &Config,
    cgi_env: &CgiEnv<T>,
) -> Result<http::Response<String>, anyhow::Error> {
    let mut router = Router::new();
    router.insert(format!("/{{{SHORT_URL_PARAM}}}"), Route::ShortUrl)?;
    router.insert("/", Route::Home)?;
    router.insert("", Route::Home)?;
    router.insert("/error/doc", Route::ErrorDocument)?;
    router.insert("/debug/env", Route::Debug)?;
    handle(config, cgi_env, &router)
}

fn repo_from_config(config: &Config) -> Result<impl Repository, anyhow::Error> {
    let path = config.database_file.clone();
    open_sqlite3_repository(path)
}

fn handle<T: fmt::Debug + Environment>(
    config: &Config,
    cgi_env: &CgiEnv<T>,
    router: &Router<Route>,
) -> Result<http::Response<String>, anyhow::Error> {
    let request = &cgi_env.new_request()?;
    #[cfg(feature = "sentry")]
    {
        cgi::sentry::add_request_context(request);
        cgi::sentry::add_cgi_context(cgi_env);
    }
    if request.method() != http::Method::GET {
        return ErrorController {}.respond((StatusCode::METHOD_NOT_ALLOWED, String::new()));
    }
    #[allow(clippy::unwrap_used)]
    let path_info = request.extensions().get::<PathInfo>().unwrap();
    let res = match router.at(path_info.as_ref()) {
        Ok(Match {
            value: Route::Home,
            params: _params,
        }) => {
            let uri = request.uri();
            if (uri.query().unwrap_or_default()).is_empty() {
                let repo = repo_from_config(config)?;
                let controller = QuotationController::new(repo);
                let response = controller.respond(())?;
                Ok(response)
            } else {
                let controller = ErrorController {};
                let response = controller.respond((StatusCode::BAD_REQUEST, String::new()))?;
                Ok(response)
            }
        }
        Ok(Match {
            value: Route::ShortUrl,
            params,
        }) => {
            let uri = request.uri();
            if (uri.query().unwrap_or_default()).is_empty() {
                #[allow(clippy::unwrap_used)]
                let short_url = params.get(SHORT_URL_PARAM).unwrap().to_string();
                match ShortUrlName::try_from(short_url.as_str()) {
                    Ok(short_url) => {
                        let repo = repo_from_config(config)?;
                        let controller = ShortUrlController::new(repo);
                        let params = ShortUrlControllerParams {
                            name: short_url,
                            page_url: request.uri().clone(),
                        };
                        let response = controller.respond(params)?;
                        Ok(response)
                    }
                    Err(_) => {
                        Ok(ErrorController {}.respond((StatusCode::NOT_FOUND, String::new()))?)
                    }
                }
            } else {
                Ok(ErrorController {}.respond((StatusCode::BAD_REQUEST, String::new()))?)
            }
        }
        Err(MatchError::NotFound) => {
            Ok(ErrorController {}.respond((StatusCode::NOT_FOUND, String::new()))?)
        }
        Ok(Match {
            value: Route::Debug,
            params: _params,
        }) => {
            let response =
                text_response(StatusCode::OK, format!("{cgi_env:#?}\n\n{request:#?}\n",));
            Ok(response)
        }
        Ok(Match {
            value: Route::ErrorDocument,
            params: _params,
        }) => {
            let status_code = cgi_env
                .getenv(MetaVariableKind::RedirectStatus)
                .and_then(|x| http::StatusCode::from_str(&x).ok())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Ok(ErrorController {}.respond((status_code, String::new()))?)
        }
    };

    res
}
