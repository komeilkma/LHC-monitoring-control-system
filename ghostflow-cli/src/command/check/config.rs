use std::collections::hash_map::HashMap;
use std::error::Error;
use std::fmt;

use erased_serde::Deserializer;
use git_checks_config::{BranchCheckConfig, CommitCheckConfig, TopicCheckConfig};
use git_checks_core::{BranchCheck, Check, GitCheckConfiguration, TopicCheck};
use lazy_static::lazy_static;
use log::warn;
use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug)]
pub enum CheckKind {
    Branch,
    Commit,
    Topic,
}

impl fmt::Display for CheckKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let s = match self {
            CheckKind::Branch => "branch",
            CheckKind::Commit => "commit",
            CheckKind::Topic => "topic",
        };

        write!(f, "{}", s)
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigError {
    #[error("invalid {} check configuration for {}", kind, name)]
    InvalidConfiguration {
        kind: CheckKind,
        name: String,
        #[source]
        source: Box<dyn Error + Send + Sync>,
    },
}

impl ConfigError {
    fn invalid_configuration(
        kind: CheckKind,
        name: String,
        source: Box<dyn Error + Send + Sync>,
    ) -> Self {
        ConfigError::InvalidConfiguration {
            kind,
            name,
            source,
        }
    }
}

type ConfigResult<T> = Result<T, ConfigError>;

lazy_static! {
    static ref CHECK_REGISTRY: Registry = Registry::new();
}

type CheckMap<T> = HashMap<String, &'static T>;

struct Registry {
    branch: CheckMap<BranchCheckConfig>,
    commit: CheckMap<CommitCheckConfig>,
    topic: CheckMap<TopicCheckConfig>,
}

impl Registry {
    fn new() -> Self {
        let mut branch = CheckMap::new();
        let mut commit = CheckMap::new();
        let mut topic = CheckMap::new();

        for check in git_checks_config::inventory::iter::<BranchCheckConfig> {
            if let Some(old) = branch.insert(check.name().into(), check) {
                warn!("duplicate branch check {}", old.name());
            }
        }

        for check in git_checks_config::inventory::iter::<CommitCheckConfig> {
            if let Some(old) = commit.insert(check.name().into(), check) {
                warn!("duplicate commit check {}", old.name());
            }
        }

        for check in git_checks_config::inventory::iter::<TopicCheckConfig> {
            if let Some(old) = topic.insert(check.name().into(), check) {
                warn!("duplicate topic check {}", old.name());
            }
        }

        Self {
            branch,
            commit,
            topic,
        }
    }
}

fn default_empty_object() -> Value {
    Value::Object(Default::default())
}

#[derive(Debug, Deserialize)]
struct CheckRead {
    kind: String,
    #[serde(default = "default_empty_object")]
    config: Value,
}

#[derive(Debug, Default, Deserialize)]
pub struct Read(Vec<CheckRead>);

type CheckVec<T> = Vec<Box<T>>;

pub struct Config {
    branch: CheckVec<dyn BranchCheck>,
    commit: CheckVec<dyn Check>,
    topic: CheckVec<dyn TopicCheck>,
}

impl Config {
    pub fn load(read: Read) -> ConfigResult<Self> {
        let registry = &*CHECK_REGISTRY;
        let mut branch = Vec::new();
        let mut commit = Vec::new();
        let mut topic = Vec::new();

        for conf in read.0 {
            let (kind, mut value) = (conf.kind, <dyn Deserializer>::erase(conf.config));

            if let Some(config) = registry.branch.get(&kind) {
                let check = config.create(&mut value).map_err(|err| {
                    ConfigError::invalid_configuration(CheckKind::Branch, kind, err)
                })?;

                branch.push(check);
                continue;
            }

            if let Some(config) = registry.commit.get(&kind) {
                let check = config.create(&mut value).map_err(|err| {
                    ConfigError::invalid_configuration(CheckKind::Commit, kind, err)
                })?;

                commit.push(check);
                continue;
            }

            if let Some(config) = registry.topic.get(&kind) {
                let check = config.create(&mut value).map_err(|err| {
                    ConfigError::invalid_configuration(CheckKind::Topic, kind, err)
                })?;

                topic.push(check);
                continue;
            }
        }

        Ok(Self {
            branch,
            commit,
            topic,
        })
    }

    pub fn check_configuration(&self) -> GitCheckConfiguration {
        let mut conf = GitCheckConfiguration::new();

        for check in &self.branch {
            conf.add_branch_check(check.as_ref());
        }

        for check in &self.commit {
            conf.add_check(check.as_ref());
        }

        for check in &self.topic {
            conf.add_topic_check(check.as_ref());
        }

        conf
    }

    pub fn commit_check_configuration(&self) -> GitCheckConfiguration {
        let mut conf = GitCheckConfiguration::new();

        for check in &self.commit {
            conf.add_check(check.as_ref());
        }

        conf
    }
}
