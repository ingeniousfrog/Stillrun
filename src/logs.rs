use std::path::Path;

use crate::{Result, StillrunError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRotationReport {
    pub rotated: bool,
    pub rotated_path: Option<std::path::PathBuf>,
}

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

pub fn rotate_log_file(path: &Path, max_bytes: u64) -> Result<LogRotationReport> {
    if !path.exists() {
        return Err(StillrunError::not_found(format!(
            "log file '{}'",
            path.display()
        )));
    }
    let metadata = std::fs::metadata(path)?;
    if max_bytes > 0 && metadata.len() <= max_bytes {
        return Ok(LogRotationReport {
            rotated: false,
            rotated_path: None,
        });
    }

    let rotated_path = rotated_log_path(path);
    std::fs::copy(path, &rotated_path)?;
    std::fs::File::create(path)?;
    Ok(LogRotationReport {
        rotated: true,
        rotated_path: Some(rotated_path),
    })
}

fn rotated_log_path(path: &Path) -> std::path::PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(".1");
    std::path::PathBuf::from(value)
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
