use std::fmt;
use std::num::ParseIntError;

use log::warn;
use termcolor::{Color, ColorSpec};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Component {
    Red,
    Green,
    Blue,
}

impl fmt::Display for Component {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let s = match self {
            Component::Red => "red",
            Component::Green => "green",
            Component::Blue => "blue",
        };

        write!(f, "{}", s)
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ColorError {
    #[error("bad color length ({})", _0)]
    BadLength(usize),
    #[error("missing `#` prefix")]
    MissingSigil,
    #[error("invalid {} value: {}", component, source)]
    InvalidComponent {
        component: Component,
        #[source]
        source: ParseIntError,
    },
}

impl ColorError {
    fn invalid_component(component: Component, source: ParseIntError) -> Self {
        ColorError::InvalidComponent {
            component,
            source,
        }
    }
}

type ColorResult<T> = Result<T, ColorError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ColorsSet {
    Neither,
    JustForeground,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GitColorAttr {
    Bold,
    Dim,
    Ul,
    Blink,
    Reverse,
    Italic,
    Strike,
}

impl ColorsSet {
    fn set_color(&mut self, spec: &mut ColorSpec, color: Color) {
        if self.can_set_fg() {
            spec.set_fg(Some(color));
        } else if self.can_set_bg() {
            spec.set_bg(Some(color));
        } else {
            warn!(
                "found a color ({:?}) where no colors are allowed; ignoring",
                color,
            );
        }

        self.next()
    }

    fn from_rgb(color: &str) -> ColorResult<Color> {
        let color = color.as_bytes();

        if color.len() != 7 {
            return Err(ColorError::BadLength(color.len()));
        }

        if color[0] != b'#' {
            return Err(ColorError::MissingSigil);
        }

        let parse_color = |hex, component| {
            u8::from_str_radix(&String::from_utf8_lossy(hex), 16)
                .map_err(|err| ColorError::invalid_component(component, err))
        };

        let r = parse_color(&color[1..3], Component::Red)?;
        let g = parse_color(&color[3..5], Component::Green)?;
        let b = parse_color(&color[5..7], Component::Blue)?;
        Ok(Color::Rgb(r, g, b))
    }

    fn set_color_parse(&mut self, spec: &mut ColorSpec, color_str: &str) {
        let color = match color_str.parse() {
            Ok(ansi256) => Some(Color::Ansi256(ansi256)),
            Err(_) => {
                Self::from_rgb(color_str)
                    .map_err(|err| {
                        warn!("unparsable Git color: `{}`: {:?}", color_str, err);
                    })
                    .ok()
            },
        };

        if self.can_set_fg() {
            spec.set_fg(color);
        } else if self.can_set_bg() {
            spec.set_bg(color);
        } else {
            warn!(
                "found a color ({}) where no colors are allowed; ignoring",
                color_str,
            );
        }

        self.next()
    }

    fn attr(&mut self, spec: &mut ColorSpec, attr: GitColorAttr, val: bool) {
        match attr {
            GitColorAttr::Bold => {
                spec.set_bold(val);
            },
            GitColorAttr::Ul => {
                spec.set_underline(val);
            },
            GitColorAttr::Dim
            | GitColorAttr::Blink
            | GitColorAttr::Reverse
            | GitColorAttr::Italic
            | GitColorAttr::Strike => (),
        };

        self.in_attrs()
    }

    fn set_attr(&mut self, spec: &mut ColorSpec, attr: GitColorAttr) {
        self.attr(spec, attr, true)
    }

    fn unset_attr(&mut self, spec: &mut ColorSpec, attr: GitColorAttr) {
        self.attr(spec, attr, false)
    }

    fn in_attrs(&mut self) {
        *self = ColorsSet::Both;
    }

    fn next(&mut self) {
        *self = match *self {
            ColorsSet::Neither => ColorsSet::JustForeground,
            _ => ColorsSet::Both,
        };
    }

    fn can_set_fg(self) -> bool {
        self == ColorsSet::Neither
    }

    fn can_set_bg(self) -> bool {
        self != ColorsSet::Both
    }
}

pub fn parse(color: &str) -> ColorSpec {
    let mut spec = ColorSpec::new();
    let mut colors_set = ColorsSet::Neither;
    for word in color.split(' ').filter(|w| !w.is_empty()) {
        match word {
            // Basic colors.
            "normal" => colors_set.next(),
            "black" => colors_set.set_color(&mut spec, Color::Black),
            "red" => colors_set.set_color(&mut spec, Color::Red),
            "green" => colors_set.set_color(&mut spec, Color::Green),
            "yellow" => colors_set.set_color(&mut spec, Color::Yellow),
            "blue" => colors_set.set_color(&mut spec, Color::Blue),
            "magenta" => colors_set.set_color(&mut spec, Color::Magenta),
            "cyan" => colors_set.set_color(&mut spec, Color::Cyan),
            "white" => colors_set.set_color(&mut spec, Color::White),

            // Attributes.
            "bold" => colors_set.set_attr(&mut spec, GitColorAttr::Bold),
            "dim" => colors_set.set_attr(&mut spec, GitColorAttr::Dim),
            "ul" => colors_set.set_attr(&mut spec, GitColorAttr::Ul),
            "blink" => colors_set.set_attr(&mut spec, GitColorAttr::Blink),
            "reverse" => colors_set.set_attr(&mut spec, GitColorAttr::Reverse),
            "italic" => colors_set.set_attr(&mut spec, GitColorAttr::Italic),
            "strike" => colors_set.set_attr(&mut spec, GitColorAttr::Strike),

            // Negated attributes.
            "no-bold" | "nobold" => colors_set.unset_attr(&mut spec, GitColorAttr::Bold),
            "no-dim" | "nodim" => colors_set.unset_attr(&mut spec, GitColorAttr::Dim),
            "no-ul" | "noul" => colors_set.unset_attr(&mut spec, GitColorAttr::Ul),
            "no-blink" | "noblink" => colors_set.unset_attr(&mut spec, GitColorAttr::Blink),
            "no-reverse" | "noreverse" => colors_set.unset_attr(&mut spec, GitColorAttr::Reverse),
            "no-italic" | "noitalic" => colors_set.unset_attr(&mut spec, GitColorAttr::Italic),
            "no-strike" | "nostrike" => colors_set.unset_attr(&mut spec, GitColorAttr::Strike),

            // Handle 256 and RGB colors.
            color => colors_set.set_color_parse(&mut spec, color),
        }
    }

    spec
}

#[cfg(test)]
mod tests {
    use log::{Level, LevelFilter, Log, Metadata, Record};
    use termcolor::{Color, ColorSpec};

    use crate::host::local::git_color::{self, ColorError, ColorsSet, Component};

    pub fn setup_logging() {
        struct SimpleLogger;

        impl Log for SimpleLogger {
            fn enabled(&self, metadata: &Metadata) -> bool {
                metadata.level() <= Level::Debug
            }

            fn log(&self, record: &Record) {
                if self.enabled(record.metadata()) {
                    println!("[{}] {}", record.level(), record.args());
                }
            }

            fn flush(&self) {}
        }

        static LOGGER: SimpleLogger = SimpleLogger;

        // Since the tests run in parallel, this may get called multiple times. Just ignore errors.
        let _ = log::set_logger(&LOGGER);
        log::set_max_level(LevelFilter::Debug);
    }

    #[test]
    fn color_parse_simple() {
        setup_logging();

        let actual = git_color::parse("");
        let expected = ColorSpec::new();
        assert_eq!(actual, expected);

        let actual = git_color::parse("normal");
        let expected = ColorSpec::new();
        assert_eq!(actual, expected);

        let actual = git_color::parse("normal normal");
        let expected = ColorSpec::new();
        assert_eq!(actual, expected);

        let actual = git_color::parse("normal  normal");
        let expected = ColorSpec::new();
        assert_eq!(actual, expected);

        let actual = git_color::parse("normal red");
        let mut expected = ColorSpec::new();
        expected.set_bg(Some(Color::Red));
        assert_eq!(actual, expected);

        let actual = git_color::parse("normal 254");
        let mut expected = ColorSpec::new();
        expected.set_bg(Some(Color::Ansi256(254)));
        assert_eq!(actual, expected);
    }

    #[test]
    fn color_parse_attr() {
        setup_logging();

        let actual = git_color::parse("bold");
        let mut expected = ColorSpec::new();
        expected.set_bold(true);
        assert_eq!(actual, expected);

        let actual = git_color::parse("bold red");
        let mut expected = ColorSpec::new();
        expected.set_bold(true);
        assert_eq!(actual, expected);

        let actual = git_color::parse("bold nobold");
        let expected = ColorSpec::new();
        assert_eq!(actual, expected);
    }

    #[test]
    fn color_parse_ansi256() {
        setup_logging();

        let actual = git_color::parse("10");
        let mut expected = ColorSpec::new();
        expected.set_fg(Some(Color::Ansi256(10)));
        assert_eq!(actual, expected);

        let actual = git_color::parse("300");
        let expected = ColorSpec::new();
        assert_eq!(actual, expected);
    }

    #[test]
    fn color_parse_rgb() {
        setup_logging();

        let actual = git_color::parse("#123456");
        let mut expected = ColorSpec::new();
        expected.set_fg(Some(Color::Rgb(0x12, 0x34, 0x56)));
        assert_eq!(actual, expected);

        match ColorsSet::from_rgb("bad length").unwrap_err() {
            ColorError::BadLength(len) => assert_eq!(len, 10),
            err => panic!("unexpected error: {:?}", err),
        }

        match ColorsSet::from_rgb("missing").unwrap_err() {
            ColorError::MissingSigil => (),
            err => panic!("unexpected error: {:?}", err),
        }

        match ColorsSet::from_rgb("#xxffff").unwrap_err() {
            ColorError::InvalidComponent {
                component, ..
            } => assert_eq!(component, Component::Red),
            err => panic!("unexpected error: {:?}", err),
        }

        match ColorsSet::from_rgb("#ffxxff").unwrap_err() {
            ColorError::InvalidComponent {
                component, ..
            } => assert_eq!(component, Component::Green),
            err => panic!("unexpected error: {:?}", err),
        }

        match ColorsSet::from_rgb("#ffffxx").unwrap_err() {
            ColorError::InvalidComponent {
                component, ..
            } => assert_eq!(component, Component::Blue),
            err => panic!("unexpected error: {:?}", err),
        }
    }
}
