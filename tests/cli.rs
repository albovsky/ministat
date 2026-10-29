use std::process::Command;

#[test]
fn one_shot_waits_for_slow_initial_health_results() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        time::Duration,
    };
    fn slow_http(body: &'static str, delay: Duration) -> (u16, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            let mut connection = loop {
                if let Ok((connection, _)) = listener.accept() {
                    break connection;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "Health request never arrived"
                );
                std::thread::sleep(Duration::from_millis(10));
            };
            connection
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0; 4096];
            assert!(connection.read(&mut request).unwrap() > 0);
            std::thread::sleep(delay);
            let _ = write!(
                connection,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
        });
        (port, task)
    }
    let dir = tempfile::tempdir().unwrap();
    let bridge = dir.path().join("bridge");
    let (jellyfin_port, jellyfin) = slow_http("Healthy", Duration::from_millis(1400));
    let (api_port, homekit) = slow_http("[{\"available\":true}]", Duration::from_millis(1200));
    let hap = TcpListener::bind("127.0.0.1:0").unwrap();
    let save = |path: &std::path::Path, value: serde_json::Value| {
        ministat::storage::write_json(path, &value).unwrap()
    };
    save(
        &bridge.join("api-token.json"),
        serde_json::json!({"token":"private-test-token"}),
    );
    save(
        &bridge.join("config.json"),
        serde_json::json!({"devices":[{}],"api_port":api_port}),
    );
    save(
        &bridge.join("runtime.json"),
        serde_json::json!({"running":true,"address":"127.0.0.1","homekit_port":hap.local_addr().unwrap().port()}),
    );
    save(
        &dir.path().join("config.json"),
        serde_json::json!({"external_mount":"/Volumes/MinistatTestMissing","homekit_data":bridge,"jellyfin_port":jellyfin_port,"runner_roots":[]}),
    );
    let output = Command::new(env!("CARGO_BIN_EXE_ministat"))
        .args(["collect", "--once", "--state-dir"])
        .arg(dir.path())
        .output()
        .unwrap();
    jellyfin.join().unwrap();
    homekit.join().unwrap();
    assert!(output.status.success());
    let snapshot: serde_json::Value =
        ministat::storage::read_json(&dir.path().join("snapshot.json")).unwrap();
    assert!(snapshot["health"]["sampled_at_unix"].as_u64().unwrap() > 0);
    assert_eq!(snapshot["health"]["homekit"]["state"], "healthy");
    assert_eq!(snapshot["health"]["jellyfin"]["state"], "healthy");
    assert!(snapshot["disks_sampled_at_unix"].as_u64().unwrap() > 0);
}

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
