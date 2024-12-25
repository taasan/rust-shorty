use cgi::cgi_env::{CgiEnv, PathInfo};
use cgi::controller::{Controller, ErrorController, QuotationController, ShortUrlController};
use cgi::{serialize_response, text_response};
use http::StatusCode;
use matchit::{Match, MatchError, Router};
use shorty::repo::Repo;
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

    match handle_request() {
        Ok(response) => {
            serialize_response(response, &mut out).unwrap();
        }
        Err(err) => {
            let _ = serialize_response(
                text_response(StatusCode::INTERNAL_SERVER_ERROR, format!("{err:#?}")),
                &mut out,
            );
        }
    }
}

fn handle_request() -> Result<http::Response<String>, Box<dyn core::error::Error>> {
    let mut router = Router::new();
    router
        .insert(format!("/{{{SHORT_URL_PARAM}}}"), Route::ShortUrl)
        .unwrap();
    router.insert("/", Route::Home).unwrap();
    router.insert("", Route::Home).unwrap();
    router.insert("/debug/env", Route::Debug).unwrap();
    handle(&router)
}

fn repo_from_env() -> Result<Repo, Box<dyn core::error::Error>> {
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
    Ok(Repo::open(path)?)
}

fn handle(router: &Router<Route>) -> Result<http::Response<String>, Box<dyn core::error::Error>> {
    let request = CgiEnv::new_request()?;
    let path_info = request.extensions().get::<PathInfo>().unwrap();
    match router.at(path_info.as_ref()) {
        Ok(Match {
            value: Route::Home,
            params: _params,
        }) => {
            let uri = request.uri();
            if (uri.query().unwrap_or_default()).is_empty() {
                let repo = repo_from_env()?;
                let controller = QuotationController::new(repo);
                let response = controller.handle_request(&request, ())?;
                Ok(response)
            } else {
                let controller = ErrorController {};
                let response = controller.handle_request(&request, StatusCode::BAD_REQUEST);
                Ok(response)
            }
        }
        Ok(Match {
            value: Route::ShortUrl,
            params,
        }) => {
            let uri = request.uri();
            if (uri.query().unwrap_or_default()).is_empty() {
                let short_url = params.get(SHORT_URL_PARAM).unwrap().to_string();
                match ShortUrlName::new(short_url) {
                    Ok(short_url) => {
                        let repo = repo_from_env()?;
                        let controller = ShortUrlController::new(repo);
                        let response = controller.handle_request(&request, short_url)?;
                        Ok(response)
                    }
                    Err(_) => {
                        Ok(ErrorController {}.handle_request(&request, StatusCode::NOT_FOUND))
                    }
                }
            } else {
                let controller = ErrorController {};
                let response = controller.handle_request(&request, StatusCode::BAD_REQUEST);
                Ok(response)
            }
        }
        Err(MatchError::NotFound) => {
            Ok(ErrorController {}.handle_request(&request, StatusCode::NOT_FOUND))
        }
        Ok(Match {
            value: Route::Debug,
            params: _params,
        }) => {
            let env = std::env::vars_os().collect::<std::collections::HashMap<_, _>>();
            let response = text_response(
                StatusCode::OK,
                format!(
                    "Full env\n{env:#?}\n\nCGI env:\n{:#?}\n\n{request:#?}\n",
                    CgiEnv::new()
                ),
            );
            Ok(response)
        }
    }
}
