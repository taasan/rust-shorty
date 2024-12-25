use askama::Template;
use http::StatusCode;
use qrcode::{render::svg, types::QrError, QrCode};
use shorty::types::ShortUrlName;
use url::Url;

#[derive(Template)]
#[template(path = "http_error.html")]
pub struct HttpErrorTemplate {
    pub status_code: StatusCode,
}

#[derive(Template)]
#[template(path = "short_url.html")]
pub struct ShortUrlTemplate {
    pub page_url: Url,
    pub short_url: ShortUrlName,
    pub url: Url,
}

#[derive(Template)]
#[template(path = "quotation.html")]
pub struct QuotationTemplate {
    pub quote: String,
}

mod filters {
    use ::askama::Result;
    use ::core::fmt::Display;

    pub fn qrcode<T: Display>(s: T) -> Result<String> {
        super::qr_svg(s.to_string()).map_err(|err| ::askama::Error::Custom(Box::new(err)))
    }

    #[allow(clippy::unnecessary_wraps)]
    pub fn base64<T: Display>(s: T) -> Result<String> {
        use base64::prelude::*;
        Ok(BASE64_STANDARD.encode(s.to_string()))
    }
}

fn qr_svg<D>(data: D) -> Result<String, QrError>
where
    D: AsRef<[u8]>,
{
    let code = QrCode::new(data)?;
    let image = code
        .render()
        .min_dimensions(200, 200)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();
    Ok(image)
}
