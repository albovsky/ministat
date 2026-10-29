use ministat::{
    health::{Process, ci_state, homekit_state, http_get},
    model::State,
};
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::Duration,
};

#[test]
fn homekit_requires_all_expected_lights_and_a_working_listener() {
    assert_eq!(
        homekit_state(&json!([{"available":true},{"available":true}]), 2, true).state,
        State::Healthy
    );
    assert_eq!(
        homekit_state(&json!([{"available":true},{"available":false}]), 2, true).state,
        State::Degraded
    );
    assert_eq!(homekit_state(&json!([]), 3, true).state, State::Degraded);
    assert_eq!(
        homekit_state(&json!([{"available":true}]), 1, false).state,
        State::Down
    );
    assert_eq!(
        homekit_state(&json!([{"available":"yes"}]), 1, true).state,
        State::Unknown
    );
}

#[test]
fn ci_tracks_each_configured_runner_and_its_worker_descendants() {
    let processes = vec![
        Process::parse("11 1 0.1 900 /Users/me/runner-a/bin/Runner.Listener").unwrap(),
        Process::parse("12 11 5.0 900 /Users/me/runner-a/bin/Runner.Worker").unwrap(),
        Process::parse("21 1 0.1 900 /Users/me/runner-b/bin/Runner.Listener").unwrap(),
    ];
    let roots = vec!["/Users/me/runner-a".into(), "/Users/me/runner-b".into()];
    let health = ci_state(&processes, &roots);
    assert_eq!(health.state, State::Healthy);
    assert_eq!(health.details["runners"][0]["busy"], true);
    assert_eq!(health.details["runners"][1]["busy"], false);
    assert_eq!(health.details["github_connectivity"], "unverified");
    assert_eq!(ci_state(&processes[..1], &roots).state, State::Degraded);
    assert_eq!(ci_state(&[], &roots).state, State::Down);
}

#[test]
fn a_successful_http_probe_is_bounded_and_sends_the_token_only_as_a_header() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut connection, _) = listener.accept().unwrap();
        let mut bytes = [0; 4096];
        let n = connection.read(&mut bytes).unwrap();
        let request = String::from_utf8_lossy(&bytes[..n]).to_lowercase();
        assert!(request.starts_with("get /v1/devices http/1.1"));
        assert!(request.contains("authorization: bearer private-test-token"));
        connection
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n[]")
            .unwrap();
    });
    assert_eq!(
        http_get(
            port,
            "/v1/devices",
            Some("private-test-token"),
            Duration::from_secs(1)
        )
        .unwrap(),
        b"[]"
    );
    server.join().unwrap();
}

#[test]
fn redirected_or_slow_probes_do_not_appear_healthy() {
    for slow in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut connection, _) = listener.accept().unwrap();
            if slow {
                std::thread::sleep(Duration::from_millis(120));
            } else {
                let _ = connection.write_all(b"HTTP/1.1 302 Found\r\nLocation: http://example.com/\r\nContent-Length: 0\r\n\r\n");
            }
        });
        assert!(http_get(port, "/health", None, Duration::from_millis(40)).is_err());
        server.join().unwrap();
    }
}
