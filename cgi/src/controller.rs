use askama::Template;
use core::fmt::Display;
use core::time::Duration;
use headers::{Expires, HeaderMapExt};
use http::{Method, Request, Response, StatusCode};
use shorty::{
    repo::{Repo, RepoError},
    types::ShortUrlName,
};
use std::time::SystemTime;
use url::Url;

use crate::{
    html_response,
    templates::{HttpErrorTemplate, QuotationTemplate, ShortUrlTemplate},
};

#[derive(Debug)]
pub enum HandleError {
    RepoError(RepoError),
    Askama(::askama::Error),
}

impl Display for HandleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RepoError(err) => write!(f, "{err}"),
            Self::Askama(err) => write!(f, "{err}"),
        }
    }
}

impl core::error::Error for HandleError {}

impl From<RepoError> for HandleError {
    fn from(value: RepoError) -> Self {
        Self::RepoError(value)
    }
}

impl From<::askama::Error> for HandleError {
    fn from(value: ::askama::Error) -> Self {
        Self::Askama(value)
    }
}

pub struct ShortUrlController {
    repo: Repo,
}

impl ShortUrlController {
    pub const fn new(repo: Repo) -> Self {
        Self { repo }
    }

    fn respond(
        &self,
        page_url: Url,
        short_url: &ShortUrlName,
    ) -> Result<Response<String>, HandleError> {
        match self.repo.get_url(short_url) {
            Ok(Some(url)) => {
                let hello = ShortUrlTemplate {
                    page_url,
                    short_url: url.short_url().clone(),
                    url: url.url().clone(),
                };
                let body = hello.render()?;
                let response = html_response(StatusCode::OK, body);
                Ok(response)
            }
            Ok(None) => Ok(ErrorController::respond(StatusCode::NOT_FOUND)),
            Err(err) => Err(HandleError::RepoError(err)),
        }
    }
}

impl Controller for ShortUrlController {
    type Params = ShortUrlName;
    type Result = Result<Response<String>, HandleError>;

    fn handle_request<T>(&self, request: &Request<T>, params: Self::Params) -> Self::Result {
        match request.method() {
            &Method::GET => {
                self.respond(request.extensions().get::<Url>().unwrap().clone(), &params)
            }
            _ => Ok(ErrorController::respond(StatusCode::METHOD_NOT_ALLOWED)),
        }
    }
}

pub struct QuotationController {
    repo: Repo,
}

impl QuotationController {
    pub const fn new(repo: Repo) -> Self {
        Self { repo }
    }
}

impl Controller for QuotationController {
    type Params = ();
    type Result = Result<Response<String>, HandleError>;

    fn handle_request<T>(&self, request: &Request<T>, _params: Self::Params) -> Self::Result {
        match request.method() {
            &Method::GET => Ok(match self.repo.get_random_quote() {
                Ok(Some(quote)) => {
                    let hello = QuotationTemplate { quote };
                    let body = hello.render()?;
                    let time = SystemTime::now() + Duration::from_secs(60 * 60 * 24);

                    let mut response = html_response(StatusCode::OK, body);
                    response.headers_mut().typed_insert(Expires::from(time));
                    response
                }
                Ok(None) => error_response(StatusCode::NOT_FOUND),
                Err(err) => html_response(StatusCode::INTERNAL_SERVER_ERROR, format!("{err:?}")),
            }),

            _ => Ok(ErrorController::respond(StatusCode::METHOD_NOT_ALLOWED)),
        }
    }
}

fn error_response(status_code: StatusCode) -> Response<String> {
    let template = HttpErrorTemplate { status_code };
    html_response(status_code, template.render().unwrap())
}

pub struct ErrorController {}

impl ErrorController {
    fn respond(status_code: StatusCode) -> Response<String> {
        let template = HttpErrorTemplate { status_code };
        html_response(status_code, template.render().unwrap())
    }
}

impl Controller for ErrorController {
    type Params = StatusCode;
    type Result = Response<String>;

    fn handle_request<T>(&self, _request: &Request<T>, params: Self::Params) -> Self::Result {
        Self::respond(params)
    }
}

pub trait Controller {
    type Params;
    type Result;
    fn handle_request<T>(&self, request: &Request<T>, params: Self::Params) -> Self::Result;
}
