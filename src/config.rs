use serde::{Deserialize, Serialize};
use std::{
    io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub external_mount: PathBuf,
    pub homekit_data: PathBuf,
    pub jellyfin_port: u16,
    /// Explicit runner directories. When omitted, registered runners under
    /// `~/actions-runners` are discovered on every health check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runner_roots: Option<Vec<PathBuf>>,
    #[serde(skip)]
    pub runners_dir: PathBuf,
}

/// Most runners a health check reports, discovered or configured.
pub const MAX_RUNNERS: usize = 10;

/// Registered GitHub Actions runners directly under `dir`, in name order.
///
/// A runner is a real directory (not a symlink) holding both the `.runner`
/// registration file and `bin/Runner.Listener`. `config.sh remove` deletes
/// `.runner`, so a deregistered leftover is not expected to be running.
pub fn discover_runners(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut roots: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_dir()))
        .map(|entry| entry.path())
        .filter(|root| root.join(".runner").is_file() && root.join("bin/Runner.Listener").is_file())
        .collect();
    roots.sort();
    roots.truncate(MAX_RUNNERS);
    roots
}

pub fn home_dir() -> io::Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("HOME is not set"))
}
pub fn state_dir() -> io::Result<PathBuf> {
    Ok(home_dir()?.join("Library/Application Support/Ministat"))
}
impl Config {
    pub fn for_home(home: &Path) -> Self {
        Self {
            external_mount: "/Volumes/Media".into(),
            homekit_data: home.join("Projects/govee-lights/data"),
            jellyfin_port: 8096,
            runner_roots: None,
            runners_dir: home.join("actions-runners"),
        }
    }
    /// The runners to check now: the configured list, or those discovered.
    pub fn runner_roots(&self) -> Vec<PathBuf> {
        match &self.runner_roots {
            Some(roots) => roots.clone(),
            None => discover_runners(&self.runners_dir),
        }
    }
    pub fn load(path: &Path) -> io::Result<Self> {
        let home = home_dir()?;
        let mut config: Self = if path.exists() {
            crate::storage::read_json(path)?
        } else {
            Self::for_home(&home)
        };
        config.runners_dir = home.join("actions-runners");
        let configured = config.runner_roots.as_deref().unwrap_or_default();
        // `..` could name a different directory once symlinks resolve; require a plain path.
        let plain = |p: &Path| !p.components().any(|c| c == std::path::Component::ParentDir);
        if !config.external_mount.is_absolute()
            || !plain(&config.external_mount)
            || !config.homekit_data.is_absolute()
            || configured.len() > MAX_RUNNERS
            || configured.iter().any(|p| !p.is_absolute())
        {
            return Err(io::Error::other(
                "Configuration paths must be absolute (no `..` in external_mount); at most 10 CI runners",
            ));
        }
        Ok(config)
    }
}
