use serde::Deserialize;
use thiserror::Error;

use crate::checks::formatter;
use crate::command::{check, reformat};

#[derive(Debug, Deserialize)]
pub struct Read {
    #[serde(default)]
    checks: check::Read,
    #[serde(default)]
    reformat: reformat::Read,
    #[serde(default)]
    formatters: formatter::Read,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ReadError {
    #[error("failed to parse YAML document: {}", source)]
    YamlParse {
        #[from]
        source: serde_yaml::Error,
    },
    #[error("failed to perform YAML merge keys: {}", source)]
    YamlMergeKeys {
        #[from]
        source: yaml_merge_keys::MergeKeyError,
    },
}

type ReadResult<T> = Result<T, ReadError>;

impl Read {
    fn from_bytes_impl(data: &[u8]) -> ReadResult<Self> {
        serde_yaml::from_slice(data)
            .map_err(ReadError::from)
            .and_then(|doc| yaml_merge_keys::merge_keys_serde(doc).map_err(ReadError::from))
            .and_then(|doc| serde_yaml::from_value(doc).map_err(ReadError::from))
    }

    pub fn from_bytes<D>(data: D) -> ReadResult<Self>
    where
        D: AsRef<[u8]>,
    {
        Self::from_bytes_impl(data.as_ref())
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigError {
    #[error("failed to read the configuration: {}", source)]
    Read {
        #[from]
        source: ReadError,
    },
    #[error("failed to load check configuration: {}", source)]
    Check {
        #[from]
        source: check::ConfigError,
    },
}

type ConfigResult<T> = Result<T, ConfigError>;

pub struct Config {
    pub checks: check::Config,
    pub reformat: reformat::Config,
    pub formatters: formatter::Config,
}

impl Config {
    pub fn load(read: Read) -> ConfigResult<Self> {
        Ok(Self {
            formatters: formatter::Config::load(read.formatters),
            checks: check::Config::load(read.checks)?,
            reformat: reformat::Config::load(read.reformat),
        })
    }

    fn from_bytes_impl(data: &[u8]) -> ConfigResult<Self> {
        let ast = Read::from_bytes(data)?;
        Self::load(ast)
    }

    pub fn from_bytes<D>(data: D) -> ConfigResult<Self>
    where
        D: AsRef<[u8]>,
    {
        Self::from_bytes_impl(data.as_ref())
    }
}
