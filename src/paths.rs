use std::{
    env,
    path::{Path, PathBuf},
};

use crate::{Result, StillrunError};

#[derive(Debug, Clone)]
pub struct StillrunPaths {
    pub home: PathBuf,
    pub db_path: PathBuf,
    pub logs_dir: PathBuf,
    pub config_path: PathBuf,
    pub launch_agents_dir: PathBuf,
}

impl StillrunPaths {
    pub fn discover() -> Result<Self> {
        let home = match env::var_os("STILLRUN_HOME") {
            Some(path) => PathBuf::from(path),
            None => default_home()?,
        };
        Ok(Self::from_home(home))
    }

    pub fn from_home(home: impl Into<PathBuf>) -> Self {
        let home = home.into();
        Self {
            db_path: home.join("stillrun.db"),
            logs_dir: home.join("logs"),
            config_path: home.join("config.toml"),
            launch_agents_dir: default_launch_agents_dir(),
            home,
        }
    }

    pub fn ensure(&self) -> Result<()> {
        std::fs::create_dir_all(&self.home)?;
        std::fs::create_dir_all(&self.logs_dir)?;
        Ok(())
    }
}

pub fn default_home() -> Result<PathBuf> {
    let user_home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| StillrunError::invalid("HOME is not set"))?;
    if cfg!(target_os = "macos") {
        Ok(user_home.join("Library/Application Support/Stillrun"))
    } else {
        Ok(user_home.join(".local/share/stillrun"))
    }
}

pub fn default_launch_agents_dir() -> PathBuf {
    if let Some(path) = env::var_os("STILLRUN_LAUNCH_AGENTS_DIR") {
        return PathBuf::from(path);
    }
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(".").to_path_buf())
        .join("Library/LaunchAgents")
}
