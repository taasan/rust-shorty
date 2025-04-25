use askama::Template;
use http::StatusCode;
use qrcode::{render::svg, types::QrError, QrCode};
use shorty::types::ShortUrl;

use crate::VERSION;

#[derive(Template)]
#[template(path = "http_error.html")]
pub struct HttpErrorTemplate {
    pub status_code: StatusCode,
    pub details: String,
}

#[derive(Template)]
#[template(path = "short_url.html")]
pub struct ShortUrlTemplate {
    pub page_url: http::Uri,
    pub short_url: ShortUrl,
}

#[derive(Template)]
#[template(path = "quotation.html")]
pub struct QuotationTemplate {
    pub quote: String,
}

#[allow(clippy::unnecessary_wraps)]
mod filters {
    use core::convert::Infallible;
    use core::fmt;

    use ::askama::filters::Safe;
    use fmt::Display;

    pub fn qrcode<T: Display>(s: T, _: &dyn askama::Values) -> askama::Result<String> {
        super::qr_svg(s.to_string()).map_err(|err| ::askama::Error::Custom(Box::new(err)))
    }

    pub fn base64<T: Display>(s: T, _: &dyn askama::Values) -> Result<String, Infallible> {
        use base64::prelude::*;
        Ok(BASE64_STANDARD.encode(s.to_string()))
    }

    // Creates an HTML/SVG/MathML comment.
    //
    // Replaces -- with __
    //
    // Comments start with the string <!-- and end with the string
    // -->, generally with text in between. This text cannot start
    // with the string > or ->, cannot contain the strings --> or
    // --!>, nor end with the string <!-, though <! is allowed.
    //
    // The above is true for XML comments as well. In addition, in
    // XML, such as in SVG or MathML markup, a comment cannot contain
    // the character sequence --
    #[allow(clippy::doc_markdown)]
    pub fn comment<T: Display>(s: T, _: &dyn askama::Values) -> Result<Safe<String>, Infallible> {
        Ok(Safe(format!(
            "<!-- {} -->",
            s.to_string().replace("--", "__")
        )))
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_http_error_template_valid_html() {
        let template = HttpErrorTemplate {
            status_code: http::StatusCode::NOT_FOUND,
            details: String::new(),
        };
        let output = template.render().unwrap();
        let errors = html::validate(&output);
        assert_eq!(errors.borrow().len(), 0, "{errors:#?}");
    }

    #[test]
    fn test_short_url_template_valid_html() {
        let template = ShortUrlTemplate {
            page_url: http::Uri::from_static("https://example.com/#ch-1"),
            short_url: ShortUrl {
                name: "abc".try_into().unwrap(),
                url: "https://example.com#ch-1".try_into().unwrap(),
            },
        };
        let output = template.render().unwrap();
        let errors = html::validate(&output);
        assert_eq!(errors.borrow().len(), 0, "{errors:#?}");
    }

    #[test]
    fn test_quotation_template_valid_html() {
        let template = QuotationTemplate {
            quote: "Don't panic\n    -- <Douglas Adams>".to_string(),
        };
        let output = template.render().unwrap();
        let errors = html::validate(&output);
        assert_eq!(errors.borrow().len(), 0, "{errors:#?}");
    }
    mod html {
        // Mostly copied from https://github.com/servo/html5ever/blob/8415d500150d3232036bd2fb9681e7820fd7ecea/html5ever/examples/noop-tree-builder.rs
        use core::cell::{Cell, RefCell};
        use std::borrow::Cow;
        use std::collections::HashMap;

        use html5ever::tendril::*;
        use html5ever::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
        use html5ever::{parse_document, ParseOpts};
        use html5ever::{Attribute, ExpandedName, QualName};

        struct Sink {
            next_id: Cell<usize>,
            names: RefCell<HashMap<usize, &'static QualName>>,
            errors: RefCell<Vec<Cow<'static, str>>>,
        }

        impl Sink {
            fn get_id(&self) -> usize {
                let id = self.next_id.get();
                self.next_id.set(id + 2);
                id
            }
        }

        /// By implementing the `TreeSink` trait we determine how the data from the tree building step
        /// is processed. In this case the DOM elements are written into the "names" hashmap.
        ///
        /// For deeper understating of each function go to the `TreeSink` declaration.
        impl TreeSink for Sink {
            type Handle = usize;
            type Output = Self;
            type ElemName<'a> = ExpandedName<'a>;
            fn finish(self) -> Self {
                self
            }

            fn get_document(&self) -> usize {
                0
            }

            fn get_template_contents(&self, target: &usize) -> usize {
                if self.names.borrow().get(target).map(|n| n.expanded())
                    == Some(expanded_name!(html "template"))
                {
                    target + 1
                } else {
                    panic!("not a template element")
                }
            }

            fn same_node(&self, x: &usize, y: &usize) -> bool {
                x == y
            }

            fn elem_name(&self, target: &usize) -> ExpandedName<'_> {
                self.names
                    .borrow()
                    .get(target)
                    .expect("not an element")
                    .expanded()
            }

            fn create_element(&self, name: QualName, _: Vec<Attribute>, _: ElementFlags) -> usize {
                let id = self.get_id();
                // N.B. We intentionally leak memory here to minimize the implementation complexity
                //      of this example code. A real implementation would either want to use a real
                //      real DOM tree implentation, or else use an arena as the backing store for
                //      memory used by the parser.
                self.names
                    .borrow_mut()
                    .insert(id, Box::leak(Box::new(name)));
                id
            }

            fn create_comment(&self, _text: StrTendril) -> usize {
                self.get_id()
            }

            #[allow(unused_variables)]
            fn create_pi(&self, target: StrTendril, value: StrTendril) -> usize {
                unimplemented!()
            }

            fn append_before_sibling(&self, _sibling: &usize, _new_node: NodeOrText<usize>) {}

            fn append_based_on_parent_node(
                &self,
                _element: &usize,
                _prev_element: &usize,
                _new_node: NodeOrText<usize>,
            ) {
            }

            fn parse_error(&self, msg: Cow<'static, str>) {
                self.errors.borrow_mut().push(msg);
            }
            fn set_quirks_mode(&self, _mode: QuirksMode) {}
            fn append(&self, _parent: &usize, _child: NodeOrText<usize>) {}

            fn append_doctype_to_document(&self, _: StrTendril, _: StrTendril, _: StrTendril) {}
            fn add_attrs_if_missing(&self, target: &usize, _attrs: Vec<Attribute>) {
                assert!(self.names.borrow().contains_key(target), "not an element");
            }
            fn remove_from_parent(&self, _target: &usize) {}
            fn reparent_children(&self, _node: &usize, _new_parent: &usize) {}
            fn mark_script_already_started(&self, _node: &usize) {}
        }

        pub fn validate(string: &str) -> RefCell<Vec<Cow<'static, str>>> {
            let sink = Sink {
                next_id: Cell::new(1),
                names: RefCell::new(HashMap::new()),
                errors: RefCell::new(Vec::new()),
            };

            let res = parse_document(sink, ParseOpts::default())
                .from_utf8()
                .read_from(&mut string.as_bytes())
                .unwrap()
                .finish();
            res.errors
        }
    }
}
