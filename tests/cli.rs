use std::process::Command;

#[test]
fn missing_collector_has_a_machine_readable_error_and_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ministat"))
        .args(["status", "--json", "--state-dir"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], "collector_unavailable");
    assert!(output.stderr.is_empty());
}

#[test]
fn invalid_flags_are_rejected_instead_of_silently_ignored() {
    let output = Command::new(env!("CARGO_BIN_EXE_ministat"))
        .args(["status", "--json", "--typo"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], "invalid_arguments");
}

#[test]
fn current_stale_and_malformed_snapshots_have_distinct_safe_results() {
    let dir = tempfile::tempdir().unwrap();
    let collect = Command::new(env!("CARGO_BIN_EXE_ministat"))
        .args(["collect", "--once", "--state-dir"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(collect.status.success());
    let status = || {
        Command::new(env!("CARGO_BIN_EXE_ministat"))
            .args(["status", "--json", "--state-dir"])
            .arg(dir.path())
            .output()
            .unwrap()
    };
    let current = status();
    assert_eq!(current.status.code(), Some(0));
    let mut value: serde_json::Value = serde_json::from_slice(&current.stdout).unwrap();
    assert!(value["cpu_percent"].is_number());
    assert_eq!(value["schema_version"], 1);
    value["sampled_at_unix"] = 1.into();
    value["health"]["homekit"]["state"] = "healthy".into();
    ministat::storage::write_json(&dir.path().join("snapshot.json"), &value).unwrap();
    let stale = status();
    assert_eq!(stale.status.code(), Some(2));
    let mut value: serde_json::Value = serde_json::from_slice(&stale.stdout).unwrap();
    assert_eq!(value["freshness"], "stale");
    assert_eq!(value["health"]["homekit"]["state"], "unknown");
    value["disks"] = serde_json::json!([]);
    ministat::storage::write_json(&dir.path().join("snapshot.json"), &value).unwrap();
    let invalid = status();
    let value: serde_json::Value = serde_json::from_slice(&invalid.stdout).unwrap();
    assert_eq!(value["error"]["code"], "collector_unavailable");
}
