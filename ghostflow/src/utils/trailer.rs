//! Trailer extraction functions.
//!
//! Trailers are key/value pairs of strings at the end of commit messages which provide metadata
//! about people involved with the commit and/or branch such as those who reported the issue fixed
//! in the commit, reviewers, copyright notices, etc.

use std::fmt::{self, Display};

use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref TRAILER_RE: Regex = Regex::new(
        "^\
         (?P<token>[[:alpha:]-]+)\
         :\\s+\
         (?P<value>.+?)\
         \\s*\
         $"
    )
    .unwrap();
}

/// A trailer from a commit message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrailerRef<'a> {
    /// The name of the trailer.
    pub token: &'a str,
    /// The value for the trailer.
    pub value: &'a str,
}

impl<'a> TrailerRef<'a> {
    /// Create a new trailer reference.
    fn new(token: &'a str, value: &'a str) -> Self {
        TrailerRef {
            token,
            value,
        }
    }

    /// Extract trailers from a commit message.
    pub fn extract(content: &'a str) -> Vec<Self> {
        let mut trailers = content
            .lines()
            .rev()
            .skip_while(|line| line.is_empty())
            .map(|line| TRAILER_RE.captures(line))
            .while_some()
            .map(|trailer| {
                Self::new(
                    trailer
                        .name("token")
                        .expect("the trailer regex should have a 'token' group")
                        .as_str(),
                    trailer
                        .name("value")
                        .expect("the trailer regex should have a 'value' group")
                        .as_str(),
                )
            })
            .collect::<Vec<_>>();

        trailers.reverse();

        trailers
    }
}

impl<'a> PartialEq<Trailer> for TrailerRef<'a> {
    fn eq(&self, other: &Trailer) -> bool {
        self.token == other.token && self.value == other.value
    }
}

impl<'a> Display for TrailerRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.token, self.value)
    }
}

/// A trailer from a commit message.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Trailer {
    /// The name of the trailer.
    pub token: String,
    /// The value for the trailer.
    pub value: String,
}

impl Trailer {
    /// Create a new trailer.
    pub fn new<T, V>(token: T, value: V) -> Self
    where
        T: Into<String>,
        V: Into<String>,
    {
        Self {
            token: token.into(),
            value: value.into(),
        }
    }

    /// The trailer as a `TrailerRef`.
    pub fn as_ref(&self) -> TrailerRef {
        TrailerRef::new(&self.token, &self.value)
    }
}

impl<'a> From<TrailerRef<'a>> for Trailer {
    fn from(trailer_ref: TrailerRef<'a>) -> Self {
        Self::new(trailer_ref.token, trailer_ref.value)
    }
}

impl<'a> PartialEq<TrailerRef<'a>> for Trailer {
    fn eq(&self, other: &TrailerRef<'a>) -> bool {
        self.token == other.token && self.value == other.value
    }
}

impl Display for Trailer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.token, self.value)
    }
}

#[cfg(test)]
mod test {
    use crate::utils::TrailerRef;

    fn check_content(content: &str, expected: &[(&str, &str)]) {
        assert_eq!(
            TrailerRef::extract(content),
            expected
                .iter()
                .map(|trailer| {
                    let &(token, value) = trailer;
                    TrailerRef::new(token, value)
                })
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn test_trailers_extract_no_trailers() {
        let content = "Some simple content.";
        let expected = &[];

        check_content(content, expected);
    }

    #[test]
    fn test_trailers_extract_simple() {
        let content = "Some simple content.\n\
                       \n\
                       Token: value";
        let expected = &[("Token", "value")];

        check_content(content, expected);
    }

    #[test]
    fn test_trailers_extract_extra_whitespace_between() {
        let content = "Some simple content.\n\
                       \n\
                       Token:   value";
        let expected = &[("Token", "value")];

        check_content(content, expected);
    }

    #[test]
    fn test_trailers_extract_extra_whitespace_trailing() {
        let content = "Some simple content.\n\
                       \n\
                       Token: value  ";
        let expected = &[("Token", "value")];

        check_content(content, expected);
    }

    #[test]
    fn test_trailers_extract_extra_whitespace_trailing_newline() {
        let content = "Some simple content.\n\
                       \n\
                       Token: value  \n";
        let expected = &[("Token", "value")];

        check_content(content, expected);
    }

    #[test]
    fn test_trailers_extract_extra_whitespace_both() {
        let content = "Some simple content.\n\
                       \n\
                       Token:   value  ";
        let expected = &[("Token", "value")];

        check_content(content, expected);
    }

    #[test]
    fn test_trailers_extract_trailers_trailing_newline() {
        let content = "Some simple content.\n\
                       \n\
                       Token: value\n";
        let expected = &[("Token", "value")];

        check_content(content, expected);
    }

    #[test]
    fn test_trailers_extract_trailers_trailing_whitespace_line() {
        let content = "Some simple content.\n\
                       \n\
                       Token: value\n            ";
        let expected = &[];

        check_content(content, expected);
    }

    #[test]
    fn test_trailers_extract_multiple_trailers() {
        let content = "Some simple content.\n\
                       \n\
                       Token: value\n\
                       Other-token: value\n";
        let expected = &[("Token", "value"), ("Other-token", "value")];

        check_content(content, expected);
    }

    #[test]
    fn test_trailers_extract_handle_blank_lines_mid() {
        let content = "Some simple content.\n\
                       \n\
                       Token: value\n\
                       \n\
                       Other-token: value\n";
        let expected = &[("Other-token", "value")];

        check_content(content, expected);
    }

    #[test]
    fn test_trailers_extract_trailing_blank_line() {
        let content = "Some simple content.\n\
                       \n\
                       Token: value\n\
                       \n";
        let expected = &[("Token", "value")];

        check_content(content, expected);
    }

    #[test]
    fn test_trailers_extract_bogus() {
        let content = "Some simple content.\n\
                       \n\
                       Missed: value\n\
                       \n\
                       Token: value";
        let expected = &[("Token", "value")];

        check_content(content, expected);
    }
}
