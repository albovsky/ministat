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
    pub runner_roots: Vec<PathBuf>,
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
            runner_roots: ["pett-macmini-simple", "pett-review"]
                .iter()
                .map(|name| home.join("actions-runners").join(name))
                .collect(),
        }
    }
    pub fn load(path: &Path) -> io::Result<Self> {
        let value = if path.exists() {
            crate::storage::read_json(path)?
        } else {
            Self::for_home(&home_dir()?)
        };
        let config: Self = value;
        // `..` could name a different directory once symlinks resolve; require a plain path.
        let plain = |p: &Path| !p.components().any(|c| c == std::path::Component::ParentDir);
        if !config.external_mount.is_absolute()
            || !plain(&config.external_mount)
            || !config.homekit_data.is_absolute()
            || config.runner_roots.len() > 10
            || config.runner_roots.iter().any(|p| !p.is_absolute())
        {
            return Err(io::Error::other(
                "Configuration paths must be absolute (no `..` in external_mount); at most 10 CI runners",
            ));
        }
        Ok(config)
    }
}
