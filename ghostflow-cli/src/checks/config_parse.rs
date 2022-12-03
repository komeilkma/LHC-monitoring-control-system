use git_checks_core::impl_prelude::*;

use crate::config::Config;
use crate::host::GHOSTFLOW_CONFIG_FILENAME;

#[derive(Debug, Default, Clone, Copy)]
pub struct ValidGhostflowConfig;

impl ValidGhostflowConfig {
    pub fn new() -> Self {
        ValidGhostflowConfig
    }
}

impl ContentCheck for ValidGhostflowConfig {
    fn name(&self) -> &str {
        "valid-ghostflow-config"
    }

    fn check(
        &self,
        ctx: &CheckGitContext,
        content: &dyn Content,
    ) -> Result<CheckResult, Box<dyn Error>> {
        let mut result = CheckResult::new();

        let config_spec = format!(":{}", GHOSTFLOW_CONFIG_FILENAME);
        let cat_file = ctx
            .git()
            .arg("cat-file")
            .arg("-e")
            .arg(&config_spec)
            .output()
            .map_err(|err| GitError::subcommand("cat-file -e", err))?;
        if cat_file.status.success() {
            let cat_file = ctx
                .git()
                .arg("cat-file")
                .arg("blob")
                .arg(config_spec)
                .output()
                .map_err(|err| GitError::subcommand("cat-file blob", err))?;

            let _: std::result::Result<Config, _> = serde_yaml::from_slice(&cat_file.stdout)
                .map_err(|err| {
                    let msg = format!(
                        "{}the {} configuration file is not valid YAML: `{}`",
                        commit_prefix_str(content, "is not allowed because"),
                        GHOSTFLOW_CONFIG_FILENAME,
                        err,
                    );
                    result.add_error(msg);
                })
                .and_then(|doc| {
                    yaml_merge_keys::merge_keys_serde(doc).map_err(|err| {
                        let msg = format!(
                            "{}the {} configuration file has invalid merge keys: `{}`",
                            commit_prefix_str(content, "is not allowed because"),
                            GHOSTFLOW_CONFIG_FILENAME,
                            err,
                        );
                        result.add_error(msg);
                    })
                })
                .and_then(|doc| {
                    serde_yaml::from_value(doc).map_err(|err| {
                        let msg = format!(
                            "{}the {} configuration file structure is not valid: `{}`",
                            commit_prefix_str(content, "is not allowed because"),
                            GHOSTFLOW_CONFIG_FILENAME,
                            err,
                        );
                        result.add_error(msg);
                    })
                })
                .and_then(|read| {
                    Config::load(read).map_err(|err| {
                        let msg = format!(
                            "{}the {} configuration file could not be loaded: `{}`",
                            commit_prefix_str(content, "is not allowed because"),
                            GHOSTFLOW_CONFIG_FILENAME,
                            err,
                        );
                        result.add_error(msg);
                    })
                });
        }

        Ok(result)
    }
}

pub(crate) mod config {
    use git_checks_config::{register_checks, CommitCheckConfig, IntoCheck, TopicCheckConfig};
    use serde::Deserialize;
    #[cfg(test)]
    use serde_json::json;

    use super::ValidGhostflowConfig;

    #[derive(Deserialize, Debug)]
    pub struct ValidGhostflowConfigConfig {}

    impl IntoCheck for ValidGhostflowConfigConfig {
        type Check = ValidGhostflowConfig;

        fn into_check(self) -> Self::Check {
            ValidGhostflowConfig::new()
        }
    }

    register_checks! {
        ValidGhostflowConfigConfig {
            "valid_ghostflow_config" => CommitCheckConfig,
            "valid_ghostflow_config/topic" => TopicCheckConfig,
        },
    }

    #[test]
    fn test_valid_ghostflow_config_deserialize() {
        let json = json!({});
        serde_json::from_value::<ValidGhostflowConfigConfig>(json).unwrap();
    }
}
