use cgi::cgi_env::{CgiEnv, Environment, OsEnvironment, PathInfo};
use cgi::controller::{
    Controller, ErrorController, QuotationController, ShortUrlController, ShortUrlControllerParams,
};
use cgi::{serialize_response, text_response};
use core::fmt;
use http::StatusCode;
use matchit::{Match, MatchError, Router};
use shorty::repository::{open_sqlite3_repository, Repository};
use shorty::types::ShortUrlName;

const SHORT_URL_PARAM: &str = "short_url";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    Home,
    ShortUrl,
    Debug,
}

fn main() {
    std::panic::set_hook(Box::new(|panic_info| {
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
    }));

    let mut out = std::io::stdout().lock();

    #[allow(clippy::unwrap_used)]
    match run() {
        Ok(response) => {
            serialize_response(response, &mut out).unwrap();
        }
        Err(err) => {
            serialize_response(
                ErrorController {}
                    .respond((StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
                    .unwrap(),
                &mut out,
            )
            .unwrap();
        }
    }
}

fn run() -> Result<http::Response<String>, Box<dyn core::error::Error>> {
    let mut router = Router::new();
    router.insert(format!("/{{{SHORT_URL_PARAM}}}"), Route::ShortUrl)?;
    router.insert("/", Route::Home)?;
    router.insert("", Route::Home)?;
    router.insert("/debug/env", Route::Debug)?;
    handle(&CgiEnv::new(OsEnvironment), &router)
}

fn repo_from_env() -> Result<impl Repository, Box<dyn core::error::Error>> {
    use std::env;
    // Linux
    //
    // https://lists.gnu.org/archive/html/bug-bash/2008-05/msg00052.html
    //
    // It's actually the kernel that interprets that line, not bash.  The
    // historical behavior is that space separates the interpreter from an
    // optional argument, and there is no escape mechanism, so there's no way
    // to specify an interpreter with a space in the path.  It's unlikely
    // that this would ever change, since that would break existing scripts
    // that rely on thecurrent behavior.
    //
    // Create an executable file with the following shebang:
    // #! <path to binary> <path to database>
    let path = env::args_os().nth(1).ok_or_else(|| {
        core::convert::Into::<Box<dyn core::error::Error>>::into(
            "missing argument: path to database",
        )
    })?;
    Ok(open_sqlite3_repository(path)?)
}

fn handle<T: fmt::Debug + Environment>(
    cgi_env: &CgiEnv<T>,
    router: &Router<Route>,
) -> Result<http::Response<String>, Box<dyn core::error::Error>> {
    let request = cgi_env.new_request()?;
    if request.method() != http::Method::GET {
        return Ok(ErrorController {}.respond((StatusCode::METHOD_NOT_ALLOWED, String::new()))?);
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
                let repo = repo_from_env()?;
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
                        let repo = repo_from_env()?;
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
    };

    drop(request);
    res
}
