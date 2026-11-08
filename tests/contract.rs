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
