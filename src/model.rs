use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;
pub const STALE_AFTER_SECONDS: u64 = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    Current,
    Stale,
    ClockChanged,
}

pub fn freshness(sampled: u64, now: u64, max_age: u64) -> Freshness {
    if sampled > now {
        Freshness::ClockChanged
    } else if now - sampled > max_age {
        Freshness::Stale
    } else {
        Freshness::Current
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Disk {
    pub name: String,
    pub mount: String,
    pub mounted: bool,
    pub total_bytes: Option<u64>,
    pub used_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
    pub used_percent: Option<f64>,
}

impl Disk {
    pub fn mounted(name: &str, mount: &str, total: u64, free: u64, available: u64) -> Self {
        let used = total.saturating_sub(free);
        Self {
            name: name.into(),
            mount: mount.into(),
            mounted: true,
            total_bytes: Some(total),
            used_bytes: Some(used),
            available_bytes: Some(available),
            used_percent: (total > 0).then(|| used as f64 / total as f64 * 100.0),
        }
    }
    pub fn missing(name: &str, mount: &str) -> Self {
        Self {
            name: name.into(),
            mount: mount.into(),
            mounted: false,
            total_bytes: None,
            used_bytes: None,
            available_bytes: None,
            used_percent: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub used_percent: f64,
    pub swap_used_bytes: u64,
    pub pressure: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Healthy,
    Degraded,
    Down,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Service {
    pub state: State,
    pub summary: String,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Health {
    pub sampled_at_unix: u64,
    pub homekit: Service,
    pub jellyfin: Service,
    pub ci: Service,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema_version: u32,
    pub sampled_at_unix: u64,
    pub boot_time_unix: u64,
    pub collector_pid: u32,
    pub hostname: String,
    pub uptime_seconds: u64,
    pub awake_seconds: u64,
    pub cpu_percent: Option<f32>,
    pub cpu_history: Vec<f32>,
    pub memory: Memory,
    pub disks: Vec<Disk>,
    #[serde(default)]
    pub disks_sampled_at_unix: u64,
    pub health: Health,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusView {
    pub freshness: Freshness,
    pub age_seconds: u64,
    pub health_freshness: Freshness,
    pub health_age_seconds: u64,
    pub disks_freshness: Freshness,
    pub disks_age_seconds: u64,
    #[serde(flatten)]
    pub snapshot: Snapshot,
}
impl StatusView {
    pub fn disk_gauge_percent(&self, index: usize) -> f64 {
        if self.freshness != Freshness::Current || self.disks_freshness != Freshness::Current {
            return 0.0;
        }
        self.snapshot
            .disks
            .get(index)
            .and_then(|disk| disk.used_percent)
            .unwrap_or(0.0)
    }

    pub fn new(mut snapshot: Snapshot, now: u64, boot: u64) -> Self {
        let mut state = freshness(snapshot.sampled_at_unix, now, STALE_AFTER_SECONDS);
        if snapshot.boot_time_unix != boot {
            state = Freshness::Stale;
        }
        let health_state = freshness(snapshot.health.sampled_at_unix, now, 90);
        let health_age = now.saturating_sub(snapshot.health.sampled_at_unix);
        let disks_state = freshness(snapshot.disks_sampled_at_unix, now, 90);
        let disks_age = now.saturating_sub(snapshot.disks_sampled_at_unix);
        if state != Freshness::Current || health_state != Freshness::Current {
            for service in [
                &mut snapshot.health.homekit,
                &mut snapshot.health.jellyfin,
                &mut snapshot.health.ci,
            ] {
                service.state = State::Unknown;
                service.summary = "Waiting for current health data".into();
            }
        }
        Self {
            freshness: state,
            age_seconds: now.saturating_sub(snapshot.sampled_at_unix),
            health_freshness: health_state,
            health_age_seconds: health_age,
            disks_freshness: disks_state,
            disks_age_seconds: disks_age,
            snapshot,
        }
    }
}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn duration_label(seconds: u64) -> String {
    let (days, hours, minutes) = (seconds / 86400, seconds / 3600 % 24, seconds / 60 % 60);
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

pub fn compact_duration_label(seconds: u64) -> String {
    let (days, hours, minutes) = (seconds / 86400, seconds / 3600 % 24, seconds / 60 % 60);
    match (days, hours, minutes) {
        (days, 0, _) if days > 0 => format!("{days}d"),
        (days, hours, _) if days > 0 => format!("{days}d{hours}h"),
        (_, hours, 0) if hours > 0 => format!("{hours}h"),
        (_, hours, minutes) if hours > 0 => format!("{hours}h{minutes}m"),
        _ => format!("{minutes}m"),
    }
}
