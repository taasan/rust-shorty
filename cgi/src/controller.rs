use askama::Template;
use core::time::Duration;
use headers::{Expires, HeaderMapExt};
use http::{Response, StatusCode};
use shorty::{repository::Repository, types::ShortUrlName};
use std::time::SystemTime;

use crate::{
    html_response,
    templates::{HttpErrorTemplate, QuotationTemplate, ShortUrlTemplate},
};

pub struct ShortUrlController<T> {
    repo: T,
}

impl<T> ShortUrlController<T> {
    pub const fn new(repo: T) -> Self {
        Self { repo }
    }
}

pub struct ShortUrlControllerParams {
    pub name: ShortUrlName,
    pub page_url: http::Uri,
}

impl<T> Controller for ShortUrlController<T>
where
    T: Repository,
{
    type Params = ShortUrlControllerParams;
    type Result = Result<Response<String>, anyhow::Error>;

    fn respond(&self, params: Self::Params) -> Self::Result {
        match self.repo.get_url(&params.name) {
            Ok(Some(short_url)) => {
                let template = ShortUrlTemplate {
                    page_url: params.page_url,
                    short_url,
                };
                let body = template.render()?;
                let response = html_response(StatusCode::OK, body);
                Ok(response)
            }
            Ok(None) => ErrorController {}.respond((StatusCode::NOT_FOUND, String::new())),
            Err(err) => Err(err.into()),
        }
    }
}

pub struct QuotationController<T> {
    repo: T,
}

impl<T> QuotationController<T>
where
    T: Repository,
{
    pub const fn new(repo: T) -> Self {
        Self { repo }
    }
}

impl<T> Controller for QuotationController<T>
where
    T: Repository,
{
    type Params = ();
    type Result = Result<Response<String>, anyhow::Error>;

    fn respond(&self, (): Self::Params) -> Self::Result {
        let quote = self.repo.get_random_quote()?;
        let template = QuotationTemplate { quote };
        let body = template.render()?;
        let time = SystemTime::now() + Duration::from_secs(60 * 60 * 24);

        let mut response = html_response(StatusCode::OK, body);
        response.headers_mut().typed_insert(Expires::from(time));
        Ok(response)
    }
}

pub struct ErrorController {}

impl Controller for ErrorController {
    type Params = (StatusCode, String);
    type Result = Result<Response<String>, anyhow::Error>;

    fn respond(&self, params: Self::Params) -> Self::Result {
        let template = HttpErrorTemplate {
            status_code: params.0,
            details: params.1,
        };
        Ok(html_response(params.0, template.render()?))
    }
}

pub trait Controller {
    type Params;
    type Result;
    fn respond(&self, params: Self::Params) -> Self::Result;
}

#[cfg(test)]
mod test {
    use super::*;

    use shorty::{repository::open_sqlite3_repository_in_memory, types::ShortUrl};

    fn repo(migrate: bool) -> impl Repository {
        let mut repo = open_sqlite3_repository_in_memory().unwrap();
        if migrate {
            repo.migrate().unwrap();
        }
        repo
    }
    #[test]
    fn test_quotation_controller_no_quotes_in_db() {
        let controller = QuotationController::new(repo(true));

        let res = controller.respond(()).unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        assert!(res.body().contains("<blockquote>"));
    }

    #[test]
    fn test_quotation_controller() {
        const QUOTE: &str = "A<>'\"";
        let mut repo = repo(true);
        repo.insert_quotation(QUOTE).unwrap();
        let controller = QuotationController::new(repo);

        let res = controller.respond(()).unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        assert!(res
            .body()
            .contains("<blockquote>A&lt;&gt;&#x27;&quot;</blockquote>"));
    }

    #[test]
    fn test_quotation_controller_error() {
        let repo = repo(false);
        let controller = QuotationController::new(repo);

        let res = controller.respond(());

        assert!(res.is_err());
    }

    #[test]
    fn test_short_url_controller() {
        let mut repo = repo(true);
        let short_url = ShortUrl::try_from(("surl", "https://example.com")).unwrap();
        repo.insert_url(&short_url).unwrap();
        let controller = ShortUrlController::new(repo);
        let params = ShortUrlControllerParams {
            page_url: http::Uri::from_static("https://example.org/surl"),
            name: short_url.name,
        };
        let res = controller.respond(params).unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert!(res
            .body()
            .contains(r#"<p><a href="https://example.com/">Go to surl</a></p>"#));
        assert!(res.body().contains(
            r#"<img alt="QR code" title="https://example.org/surl" src="data:image/svg+xml;base64,"#
        ));
    }

    #[test]
    fn test_short_url_controller_no_quotes_in_db() {
        let controller = ShortUrlController::new(repo(true));
        let params = ShortUrlControllerParams {
            page_url: http::Uri::from_static("https://example.org/surl"),
            name: "abc".try_into().unwrap(),
        };

        let res = controller.respond(params).unwrap();

        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_short_url_controller_db_not_initialized() {
        let repo = repo(false);
        let controller = ShortUrlController::new(repo);
        let params = ShortUrlControllerParams {
            page_url: http::Uri::from_static("https://example.org/surl"),
            name: "abc".try_into().unwrap(),
        };

        let res = controller.respond(params);

        assert!(res.is_err());
    }

    #[test]
    fn test_error_controller() {
        let controller = ErrorController {};

        let res = controller
            .respond((StatusCode::IM_A_TEAPOT, String::new()))
            .unwrap();

        assert_eq!(res.status(), StatusCode::IM_A_TEAPOT);
        assert!(res.body().contains("<h2>418 I&#x27;m a teapot</h2>"));
    }
}
