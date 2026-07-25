use std::path::Path;

use crate::{Result, StillrunError};

pub fn prepare_follow_log_file(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if !path.exists() {
        std::fs::File::create(path)?;
    }
    Ok(())
}

pub fn tail_log_file(path: &Path, lines: usize) -> Result<String> {
    if !path.exists() {
        return Err(StillrunError::not_found(format!(
            "log file '{}'",
            path.display()
        )));
    }
    let text = std::fs::read_to_string(path)?;
    let selected = text
        .lines()
        .rev()
        .take(lines)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");
    if selected.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!("{selected}\n"))
    }
}

pub async fn follow_log_file(path: &Path) -> Result<()> {
    let status = tokio::process::Command::new("tail")
        .arg("-f")
        .arg(path)
        .status()
        .await?;
    if status.success() {
        Ok(())
    } else {
        Err(StillrunError::invalid(format!(
            "tail exited with status {status}"
        )))
    }
}
