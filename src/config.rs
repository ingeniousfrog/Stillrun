use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{paths::StillrunPaths, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StillrunConfig {
    pub max_output_bytes: usize,
    pub redact_keys: BTreeSet<String>,
}

impl Default for StillrunConfig {
    fn default() -> Self {
        Self {
            max_output_bytes: 1_048_576,
            redact_keys: [
                "token",
                "secret",
                "password",
                "passwd",
                "api_key",
                "apikey",
                "credential",
                "private_key",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        }
    }
}

impl StillrunConfig {
    pub fn load(paths: &StillrunPaths) -> Result<Self> {
        if !paths.config_path.exists() {
            return Ok(Self::default());
        }
        let config_text = std::fs::read_to_string(&paths.config_path)?;
        Ok(toml::from_str(&config_text)?)
    }
}
