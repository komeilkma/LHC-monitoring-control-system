use std::collections::BTreeMap;
use std::sync::RwLock;
use std::time::Duration;

use ghostflow::actions::reformat::{self, Reformat};
use git_checks::Formatting;
use lazy_static::lazy_static;
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FormatterError {
    #[error(
        "formatters must be specified as <kind>=<path>; missing a `=` in `{}`?",
        argument
    )]
    MissingFormatterPath { argument: String },
}

impl FormatterError {
    fn missing_formatter_path(argument: String) -> Self {
        FormatterError::MissingFormatterPath {
            argument,
        }
    }
}

type FormatterResult<T> = Result<T, FormatterError>;

lazy_static! {
    static ref FORMATTER_REGISTRY: RwLock<BTreeMap<String, Formatter>> =
        RwLock::new(BTreeMap::new());
}

static LOCK_POISONED: &str = "formatter registry lock poisoned";

#[derive(Deserialize, Debug, Clone)]
pub struct Formatter {
    #[serde(default)]
    name: Option<String>,
    formatter: String,
    #[serde(default)]
    config_files: Vec<String>,
    #[serde(default)]
    timeout: Option<u64>,
}

pub type Read = BTreeMap<String, Formatter>;

#[derive(Debug, Clone, Copy)]
pub struct Config;

impl Config {
    pub fn load(read: Read) -> Self {
        read.into_iter()
            .for_each(|(kind, formatter)| formatter.add_as(kind));

        Self
    }
}

impl Formatter {
    fn add_as<N>(self, name: N)
    where
        N: Into<String>,
    {
        let mut registry = FORMATTER_REGISTRY.write().expect(LOCK_POISONED);
        // Do not override a previous entry.
        registry.entry(name.into()).or_insert(self);
    }

    fn set_path<K, P>(kind: K, path: P) -> Option<()>
    where
        K: AsRef<str>,
        P: Into<String>,
    {
        let mut registry = FORMATTER_REGISTRY.write().expect(LOCK_POISONED);
        registry
            .get_mut(kind.as_ref())
            .map(|fmt| fmt.formatter = path.into())
    }

    pub fn parse_args<'a, I>(formatters: I) -> FormatterResult<()>
    where
        // FIXME(rust#49802): Use `values_of_os` so that non-utf-8 paths may be used. The `OsStr`
        // type is missing the `splitn` method.
        I: Iterator<Item = &'a str>,
    {
        for formatter_spec in formatters {
            let mut split = formatter_spec.splitn(2, '=');
            let kind = split.next().expect("failed to split first entry?");
            let path = if let Some(path) = split.next() {
                path
            } else {
                return Err(FormatterError::missing_formatter_path(
                    formatter_spec.into(),
                ));
            };

            Self::set_path(kind, path);
        }

        Ok(())
    }

    fn check<K>(kind: K, fix_message: Option<String>) -> Formatting
    where
        K: Into<String>,
    {
        let registry = FORMATTER_REGISTRY.read().expect(LOCK_POISONED);
        let kind = kind.into();
        let formatter = registry
            .get(&kind)
            .unwrap_or_else(|| panic!("unknown formatter '{}'", kind))
            .clone();

        let mut builder = Formatting::builder();
        builder
            .kind(kind)
            .formatter(formatter.formatter)
            .config_files(formatter.config_files);

        formatter.name.map(|name| builder.name(name));
        fix_message.map(|fix_message| builder.fix_message(fix_message));
        formatter
            .timeout
            .map(|timeout| builder.timeout(Duration::from_secs(timeout)));

        builder.build().unwrap()
    }

    pub fn action<I, K>(reformat: &mut Reformat, kinds: I) -> Result<(), reformat::ReformatError>
    where
        I: IntoIterator<Item = K>,
        K: Into<String>,
    {
        let registry = FORMATTER_REGISTRY.read().expect(LOCK_POISONED);

        let reformatters = kinds
            .into_iter()
            .map(Into::into)
            .map(|kind| {
                let formatter = registry.get(&kind).expect("TODO");
                let mut reformatter = reformat::Formatter::new(kind, &formatter.formatter)?;
                reformatter.add_config_files(formatter.config_files.iter());
                formatter
                    .timeout
                    .map(|timeout| reformatter.with_timeout(Duration::from_secs(timeout)));
                Ok(reformatter)
            })
            .collect::<Result<Vec<_>, reformat::ReformatError>>()?;
        reformat.add_formatters(reformatters);

        Ok(())
    }
}

mod config {
    use git_checks::Formatting;
    use git_checks_config::{register_checks, CommitCheckConfig, IntoCheck, TopicCheckConfig};
    use serde::Deserialize;
    #[cfg(test)]
    use serde_json::json;

    use super::Formatter;

    #[derive(Deserialize, Debug)]
    pub struct FormatterConfig {
        kind: String,
        #[serde(default)]
        fix_message: Option<String>,
    }

    impl IntoCheck for FormatterConfig {
        type Check = Formatting;

        fn into_check(self) -> Self::Check {
            Formatter::check(self.kind, self.fix_message)
        }
    }

    register_checks! {
        FormatterConfig {
            "formatter" => CommitCheckConfig,
            "formatter/topic" => TopicCheckConfig,
        },
    }

    #[test]
    fn test_valid_formatter_deserialize() {
        let exp_kind = "kind";
        let exp_fix_message: String = "instructions for fixing".into();
        let json = json!({
            "kind": exp_kind,
            "fix_message": exp_fix_message,
        });
        let check: FormatterConfig = serde_json::from_value(json).unwrap();

        assert_eq!(check.kind, exp_kind);
        assert_eq!(check.fix_message, Some(exp_fix_message))
    }

    #[test]
    fn test_valid_formatter_deserialize_defaults() {
        let exp_kind = "kind";
        let json = json!({
            "kind": exp_kind,
        });
        let check: FormatterConfig = serde_json::from_value(json).unwrap();

        assert_eq!(check.kind, exp_kind);
        assert_eq!(check.fix_message, None)
    }
}
