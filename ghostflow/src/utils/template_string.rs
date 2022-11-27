//! Template strings.
//!
//! Supports replacing template parameters (`{name}`) with values from a lookup map.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::{self, Debug};

use lazy_static::lazy_static;
use log::warn;
use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
enum TemplatePart {
    Literal { start: usize, end: usize },
    NamedField { name: String },
}

lazy_static! {
    static ref TEMPLATE_REPLACEMENT: Regex =
        Regex::new(r"(?P<expand>\{(?P<name>[A-Za-z0-9_]+)\})").unwrap();
}

#[derive(Clone)]
pub(crate) struct TemplateString {
    template: String,
    parts: Vec<TemplatePart>,
}

impl TemplateString {
    pub(crate) fn new<T>(template: T) -> Self
    where
        T: Into<String>,
    {
        Self::new_impl(template.into())
    }

    fn new_impl(template: String) -> Self {
        let fields = TEMPLATE_REPLACEMENT
            .captures_iter(&template)
            .map(|capture| {
                let expand = capture
                    .name("expand")
                    .expect("the template regex should have a 'expand' group");
                let name = capture
                    .name("name")
                    .expect("the template regex should have a 'name' group");

                (name.as_str(), expand.start(), expand.end())
            });
        let mut last_end = 0;
        let mut parts = Vec::new();

        for field in fields {
            let (name, start, end) = field;

            if last_end < start {
                parts.push(TemplatePart::Literal {
                    start: last_end,
                    end: start,
                })
            }

            last_end = end;
            parts.push(TemplatePart::NamedField {
                name: name.into(),
            });
        }

        if last_end < template.len() {
            parts.push(TemplatePart::Literal {
                start: last_end,
                end: template.len(),
            })
        }

        Self {
            template,
            parts,
        }
    }

    pub(crate) fn replace(&self, context: &HashMap<&str, Cow<str>>) -> String {
        let mut result = String::new();

        for part in &self.parts {
            match part {
                TemplatePart::Literal {
                    start,
                    end,
                } => {
                    result.push_str(&self.template[*start..*end]);
                },
                TemplatePart::NamedField {
                    name,
                } => {
                    if let Some(value) = context.get(name as &str) {
                        result.push_str(value);
                    } else {
                        warn!(
                            target: "ghostflow/template_string",
                            "unknown template replacement for `{}`",
                            name,
                        );
                    }
                },
            }
        }

        result
    }
}

impl Debug for TemplateString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TemplateString")
            .field("template", &self.template)
            .finish()
    }
}

#[cfg(test)]
mod test {
    use std::borrow::Cow;

    use super::{TemplatePart, TemplateString};

    #[test]
    fn test_template_string_parse_literal() {
        let ts = TemplateString::new("literal");
        itertools::assert_equal(
            ts.parts,
            [TemplatePart::Literal {
                start: 0,
                end: 7,
            }],
        );
    }

    #[test]
    fn test_template_string_parse_empty_field() {
        let ts = TemplateString::new("{}");
        itertools::assert_equal(
            ts.parts,
            [TemplatePart::Literal {
                start: 0,
                end: 2,
            }],
        );
    }

    #[test]
    fn test_template_string_parse_field() {
        let ts = TemplateString::new("{literal}");
        itertools::assert_equal(
            ts.parts,
            [TemplatePart::NamedField {
                name: "literal".into(),
            }],
        );
    }

    #[test]
    fn test_template_string_parse_field_invalid() {
        let ts = TemplateString::new("{invalid-literal}");
        itertools::assert_equal(
            ts.parts,
            [TemplatePart::Literal {
                start: 0,
                end: 17,
            }],
        );
    }

    #[test]
    fn test_template_string_parse_complex() {
        let ts = TemplateString::new("This is a {adjective} complex {addition}{noun}.");
        itertools::assert_equal(
            ts.parts,
            [
                TemplatePart::Literal {
                    start: 0,
                    end: 10,
                },
                TemplatePart::NamedField {
                    name: "adjective".into(),
                },
                TemplatePart::Literal {
                    start: 21,
                    end: 30,
                },
                TemplatePart::NamedField {
                    name: "addition".into(),
                },
                TemplatePart::NamedField {
                    name: "noun".into(),
                },
                TemplatePart::Literal {
                    start: 46,
                    end: 47,
                },
            ],
        );
    }

    #[test]
    fn test_template_string_replace() {
        let lookup = [
            ("id", Cow::Borrowed("id")),
            ("name", Cow::Borrowed("value")),
            ("confusing", Cow::Borrowed("{name}")),
        ]
        .iter()
        .cloned()
        .collect();

        let ts = TemplateString::new("literal");
        assert_eq!(ts.replace(&lookup), "literal");

        let ts = TemplateString::new("simple {replacement}");
        assert_eq!(ts.replace(&lookup), "simple ");

        let ts = TemplateString::new("{id}");
        assert_eq!(ts.replace(&lookup), "id");

        let ts = TemplateString::new("Your name is '{name}'.");
        assert_eq!(ts.replace(&lookup), "Your name is 'value'.");

        let ts = TemplateString::new("This can be a {confusing} replacement for {name}.");
        assert_eq!(
            ts.replace(&lookup),
            "This can be a {name} replacement for value.",
        );
    }
}
