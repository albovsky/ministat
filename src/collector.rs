use crate::{
    config::Config,
    health::check_health,
    metrics::{Sampler, disks},
    model::Health,
    storage::{CollectorLock, write_json},
};
use std::{
    io,
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

pub fn collect(state_dir: &Path, config: Config, once: bool) -> io::Result<()> {
    let _lock = CollectorLock::acquire(&state_dir.join("collector.lock"))?;
    let health = Arc::new(Mutex::new(Health::default()));
    let shared = health.clone();
    let health_config = config.clone();
    // Blocking probes run separately: a slow service cannot stall CPU/RAM sampling.
    thread::spawn(move || {
        loop {
            let result = check_health(&health_config);
            if let Ok(mut guard) = shared.lock() {
                *guard = result;
            }
            thread::sleep(Duration::from_secs(30));
        }
    });
    let storage = Arc::new(Mutex::new((
        vec![
            crate::model::Disk::missing("Internal", "/System/Volumes/Data"),
            crate::model::Disk::missing("Media", &config.external_mount.to_string_lossy()),
        ],
        0,
    )));
    let disk_shared = storage.clone();
    thread::spawn(move || {
        loop {
            let result = disks(&config.external_mount);
            if let Ok(mut guard) = disk_shared.lock() {
                *guard = (result, crate::model::now_unix());
            }
            thread::sleep(Duration::from_secs(30));
        }
    });
    let mut sampler = Sampler::default();
    loop {
        let started = Instant::now();
        let (storage, disk_time) = storage
            .lock()
            .map_err(|_| io::Error::other("Disk worker lock poisoned"))?
            .clone();
        let report = health
            .lock()
            .map_err(|_| io::Error::other("Health worker lock poisoned"))?
            .clone();
        let snapshot = sampler.sample(storage, disk_time, report);
        write_json(&state_dir.join("snapshot.json"), &snapshot)?;
        if once && snapshot.cpu_percent.is_some() {
            return Ok(());
        }
        thread::sleep(Duration::from_secs(2).saturating_sub(started.elapsed()));
    }
}
