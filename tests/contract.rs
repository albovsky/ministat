use ministat::model::{Disk, Freshness, compact_duration_label, freshness};
use ministat::storage::{CollectorLock, read_json, write_json};
use serde_json::json;

#[test]
fn menu_uptime_is_compact_without_losing_its_units() {
    for (seconds, expected) in [
        (0, "0m"),
        (59 * 60, "59m"),
        (3600, "1h"),
        (4260, "1h11m"),
        (86400, "1d"),
        (86400 + 3 * 3600, "1d3h"),
    ] {
        assert_eq!(compact_duration_label(seconds), expected);
    }
}

#[test]
fn storage_capacity_accounts_for_shared_free_space_and_missing_volumes() {
    let disk = Disk::mounted("Internal", "/System/Volumes/Data", 1000, 400, 350);
    assert_eq!(disk.used_bytes, Some(600));
    assert_eq!(disk.used_percent, Some(60.0));
    assert_eq!(disk.available_bytes, Some(350));
    assert_eq!(Disk::missing("Media", "/Volumes/Media").used_percent, None);
    assert_eq!(Disk::mounted("Empty", "/", 0, 0, 0).used_percent, None);
}

#[test]
fn stale_and_future_samples_are_never_reported_as_current() {
    assert_eq!(freshness(100, 110, 15), Freshness::Current);
    assert_eq!(freshness(100, 116, 15), Freshness::Stale);
    assert_eq!(freshness(200, 100, 15), Freshness::ClockChanged);
}

#[test]
fn snapshot_writes_are_private_atomic_and_reader_safe() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state/snapshot.json");
    write_json(&path, &json!({"sequence": 1})).unwrap();
    write_json(&path, &json!({"sequence": 2})).unwrap();
    let value: serde_json::Value = read_json(&path).unwrap();
    assert_eq!(value["sequence"], 2);
    assert_eq!(path.metadata().unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(
        path.parent()
            .unwrap()
            .metadata()
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
}

#[test]
fn a_second_collector_cannot_replace_the_live_collectors_state() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("collector.lock");
    let first = CollectorLock::acquire(&path).unwrap();
    assert!(CollectorLock::acquire(&path).is_err());
    drop(first);
    assert!(CollectorLock::acquire(&path).is_ok());
}

#[test]
fn snapshots_from_older_collectors_still_parse() {
    let old = json!({
        "schema_version": 1, "sampled_at_unix": 1, "boot_time_unix": 1, "collector_pid": 1,
        "hostname": "mini", "uptime_seconds": 1, "awake_seconds": 1, "cpu_percent": null,
        "cpu_history": [], "disks": [],
        "memory": {"total_bytes": 1, "used_bytes": 1, "available_bytes": 1, "used_percent": 1.0,
                   "swap_used_bytes": 0, "pressure": "normal"},
        "health": {"sampled_at_unix": 1, "homekit": {"state": "healthy", "summary": "", "details": {}},
                   "jellyfin": {"state": "healthy", "summary": "", "details": {}},
                   "ci": {"state": "healthy", "summary": "", "details": {}}},
    });
    let snapshot: ministat::model::Snapshot = serde_json::from_value(old).unwrap();
    assert!(snapshot.load.is_none());
    assert_eq!(
        snapshot.health.hermes.state,
        ministat::model::State::Unknown
    );
    assert_eq!(
        snapshot.health.tailscale.state,
        ministat::model::State::Unknown
    );
}

#[test]
fn stale_health_hides_every_service_including_hermes_and_tailscale() {
    use ministat::model::{Snapshot, State, StatusView};
    let mut value = json!({
        "schema_version": 1, "sampled_at_unix": 100, "boot_time_unix": 7, "collector_pid": 1,
        "hostname": "mini", "uptime_seconds": 1, "awake_seconds": 1, "cpu_percent": 1.0,
        "cpu_history": [], "disks": [], "disks_sampled_at_unix": 100,
        "load": {"one_minute": 2.0, "five_minutes": 2.5, "fifteen_minutes": 3.0, "logical_cpus": 8},
        "memory": {"total_bytes": 1, "used_bytes": 1, "available_bytes": 1, "used_percent": 1.0,
                   "swap_used_bytes": 0, "pressure": "normal"},
        "health": {"sampled_at_unix": 1},
    });
    for service in ["homekit", "jellyfin", "ci", "hermes", "tailscale"] {
        value["health"][service] = json!({"state": "healthy", "summary": "ok", "details": {}});
    }
    let snapshot: Snapshot = serde_json::from_value(value).unwrap();
    assert_eq!(snapshot.load.as_ref().unwrap().logical_cpus, 8);
    let view = StatusView::new(snapshot, 100, 7);
    let health = &view.snapshot.health;
    for service in [
        &health.homekit,
        &health.jellyfin,
        &health.ci,
        &health.hermes,
        &health.tailscale,
    ] {
        assert_eq!(service.state, State::Unknown);
    }
}
