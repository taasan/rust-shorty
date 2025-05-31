use askama::Template;
use core::time::Duration;
use headers::{CacheControl, ETag, Expires, Header as _, HeaderMapExt as _, LastModified};
use http::{Response, StatusCode};
use shorty::anyhow;
use shorty::{repository::Repository, types::ShortUrlName};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    html_response,
    templates::{HttpErrorTemplate, QuotationTemplate, ShortUrlTemplate},
    VERSION,
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
                let last_modified = short_url.last_modified.0;
                let etag = format!("\"{VERSION}-{last_modified}\"")
                    .parse::<ETag>()
                    .expect("Failed to create ETag");
                let template = ShortUrlTemplate {
                    page_url: params.page_url,
                    short_url,
                };
                let body = template.render()?;
                let mut response = html_response(StatusCode::OK, body);
                response.headers_mut().typed_insert(etag);
                response.headers_mut().typed_insert(LastModified::from(
                    UNIX_EPOCH + Duration::from_secs(last_modified),
                ));
                // TODO: headers::CacheControl doesn't support all this yet
                response.headers_mut().insert(
                    CacheControl::name(),
                    "public, s-maxage=300, proxy-revalidate"
                        .try_into()
                        .expect("Failed to create CacheControl"),
                );
                Ok(response)
            }
            Ok(None) => ErrorController {}.respond((StatusCode::NOT_FOUND, String::new())),
            Err(err) => Err(err),
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

    use shorty::{
        repository::{sqlite::open_writable_in_memory_repository, WritableRepository},
        types::{ShortUrl, UnixTimestamp},
    };

    fn repo(migrate: bool) -> impl WritableRepository {
        let mut repo = open_writable_in_memory_repository().unwrap();
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
            .contains("<blockquote>A&#60;&#62;&#39;&#34;</blockquote>"));
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
        let short_url = ShortUrl {
            name: "surl".try_into().unwrap(),
            url: "https://example.com".try_into().unwrap(),
            last_modified: UnixTimestamp::default(),
        };
        repo.insert_url(&short_url.name, &short_url.url).unwrap();
        let controller = ShortUrlController::new(repo);
        let params = ShortUrlControllerParams {
            page_url: http::Uri::from_static("https://example.org/surl"),
            name: short_url.name,
        };
        let res = controller.respond(params).unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert!(res.headers().contains_key(headers::ETag::name()));
        assert!(res.headers().contains_key(headers::CacheControl::name()));
        assert!(res.headers().contains_key(headers::LastModified::name()));
        assert!(res
            .body()
            .contains(r#"<a href="https://example.com/">Go to surl"#));
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
        assert!(res.body().contains("<h2>418 I&#39;m a teapot</h2>"));
    }
}
