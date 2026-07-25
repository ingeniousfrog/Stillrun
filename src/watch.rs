use std::path::Path;

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

use crate::Result;

pub fn watch_path(path: &Path) -> Result<RecommendedWatcher> {
    let mut watcher = RecommendedWatcher::new(
        move |event| {
            if let Err(err) = event {
                tracing::warn!(error = %err, "file watch event failed");
            }
        },
        Config::default(),
    )
    .map_err(|err| crate::StillrunError::invalid(err.to_string()))?;
    watcher
        .watch(path, RecursiveMode::NonRecursive)
        .map_err(|err| crate::StillrunError::invalid(err.to_string()))?;
    Ok(watcher)
}
