use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Serialize};

use crate::redact;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandContext {
    pub cwd: PathBuf,
    pub git_repo: Option<PathBuf>,
    pub git_branch: Option<String>,
    pub env: BTreeMap<String, String>,
}

impl CommandContext {
    pub fn capture(cwd: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref().to_path_buf();
        let git_repo = git_output(&cwd, &["rev-parse", "--show-toplevel"]).map(PathBuf::from);
        let git_branch = git_output(&cwd, &["rev-parse", "--abbrev-ref", "HEAD"]);
        let env = std::env::vars()
            .map(|(key, value)| {
                let redacted = redact::redact_env_value(&key, &value);
                (key, redacted)
            })
            .collect();
        Self {
            cwd,
            git_repo,
            git_branch,
            env,
        }
    }

    pub fn restorable_env(&self) -> BTreeMap<String, String> {
        self.env
            .iter()
            .filter(|(_, value)| value.as_str() != redact::REDACTED)
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }
}

fn git_output(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}
