use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{paths::StillrunPaths, redact::RedactionPolicy, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StillrunConfig {
    pub max_output_bytes: usize,
    pub redact_keys: BTreeSet<String>,
}

impl Default for StillrunConfig {
    fn default() -> Self {
        Self {
            max_output_bytes: 1_048_576,
            redact_keys: crate::redact::default_sensitive_keys()
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

    pub fn save(&self, paths: &StillrunPaths) -> Result<()> {
        if let Some(parent) = paths.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        std::fs::write(&paths.config_path, text)?;
        Ok(())
    }

    pub fn redaction_policy(&self) -> RedactionPolicy {
        RedactionPolicy::from_keys(self.redact_keys.iter())
    }

    pub fn set_value(&mut self, key: &str, value: &str) -> Result<()> {
        match normalize_key(key).as_str() {
            "max_output_bytes" => {
                let parsed = value.parse::<usize>().map_err(|_| {
                    crate::StillrunError::invalid("max-output-bytes must be a positive integer")
                })?;
                if parsed == 0 {
                    return Err(crate::StillrunError::invalid(
                        "max-output-bytes must be greater than zero",
                    ));
                }
                self.max_output_bytes = parsed;
                Ok(())
            }
            other => Err(crate::StillrunError::invalid(format!(
                "unknown config key '{other}'"
            ))),
        }
    }

    pub fn add_redact_key(&mut self, key: &str) -> Result<bool> {
        let key = normalize_redact_key(key)?;
        Ok(self.redact_keys.insert(key))
    }

    pub fn remove_redact_key(&mut self, key: &str) -> Result<bool> {
        let key = normalize_redact_key(key)?;
        Ok(self.redact_keys.remove(&key))
    }
}

fn normalize_key(key: &str) -> String {
    key.trim().replace('-', "_")
}

fn normalize_redact_key(key: &str) -> Result<String> {
    let key = key.trim().to_ascii_lowercase();
    if key.is_empty() {
        return Err(crate::StillrunError::invalid("redact key cannot be empty"));
    }
    Ok(key)
}
