use serde::{Serialize, de::DeserializeOwned};
use std::os::{
    fd::AsRawFd,
    unix::fs::{OpenOptionsExt, PermissionsExt},
};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::Path,
};

pub fn read_json<T: DeserializeOwned>(path: &Path) -> io::Result<T> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    if file.metadata()?.len() > 1024 * 1024 {
        return Err(io::Error::other("JSON file exceeds 1 MiB limit"));
    }
    let mut data = Vec::new();
    file.take(1024 * 1024 + 1).read_to_end(&mut data)?;
    serde_json::from_slice(&data).map_err(io::Error::other)
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Missing parent directory"))?;
    fs::create_dir_all(parent)?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    let temporary = parent.join(format!(".snapshot-{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&temporary)?;
        serde_json::to_writer(&mut file, value).map_err(io::Error::other)?;
        file.write_all(b"\n")?;
        // Telemetry is disposable: atomic replacement, without forcing an SSD fsync every 2s.
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

pub struct CollectorLock(File);
impl CollectorLock {
    pub fn acquire(path: &Path) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)?;
        // SAFETY: valid, owned descriptor; flock does not access Rust memory.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(io::Error::other(
                "Another collector already owns this state directory",
            ));
        }
        Ok(Self(file))
    }
}
impl Drop for CollectorLock {
    fn drop(&mut self) {
        // SAFETY: descriptor remains valid until this owned File is dropped.
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}
