use ghostflow::actions::reformat::{Reformat, ReformatError};
use serde::Deserialize;

use crate::checks::formatter::Formatter;

#[derive(Debug, Default, Deserialize)]
pub struct Read(Vec<String>);

pub struct Config {
    formatters: Vec<String>,
}

impl Config {
    pub fn load(read: Read) -> Self {
        Self {
            formatters: read.0,
        }
    }

    pub fn add_formatters(&self, reformat: &mut Reformat) -> Result<(), ReformatError> {
        Formatter::action(reformat, self.formatters.iter().cloned())
    }
}
