use ministat::{
    health::{
        Process, ci_state, hermes_state, homekit_state, http_get, tailscale_state,
        unix_from_rfc3339,
    },
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

#[test]
fn hermes_is_healthy_only_when_its_gateway_runs_and_reports_ok() {
    let ok =
        json!({"version":"0.23.0","gateway_running":true,"gateway_state":"running","overall":"ok"});
    let healthy = hermes_state(Some(&ok), 9120);
    assert_eq!(healthy.state, State::Healthy);
    assert_eq!(healthy.summary, "v0.23.0 · gateway running");
    let mut degraded = ok.clone();
    degraded["overall"] = "degraded".into();
    assert_eq!(hermes_state(Some(&degraded), 9120).state, State::Degraded);
    let mut stopped = ok.clone();
    stopped["gateway_running"] = false.into();
    assert_eq!(hermes_state(Some(&stopped), 9120).state, State::Down);
    assert_eq!(hermes_state(None, 9120).state, State::Down);
    assert_eq!(hermes_state(Some(&json!([])), 9120).state, State::Unknown);
}

const NOW: u64 = 1_794_189_601; // 2026-11-09T02:00:01Z

fn tailnet(expiry: Option<&str>) -> serde_json::Value {
    let mut status = json!({"BackendState":"Running","Health":[],"Self":{"Online":true}});
    if let Some(expiry) = expiry {
        status["Self"]["KeyExpiry"] = expiry.into();
    }
    status
}

#[test]
fn rfc3339_times_convert_to_unix_seconds() {
    assert_eq!(unix_from_rfc3339("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(
        unix_from_rfc3339("2027-05-04T04:29:10Z"),
        Some(1_809_404_950)
    );
    assert_eq!(
        unix_from_rfc3339("2027-05-04T04:29:10.123456789Z"),
        Some(1_809_404_950)
    );
    assert_eq!(
        unix_from_rfc3339("2027-05-04T06:29:10+02:00"),
        Some(1_809_404_950)
    );
    assert_eq!(unix_from_rfc3339("0001-01-01T00:00:00Z"), None);
    assert_eq!(unix_from_rfc3339("2027-13-04T04:29:10Z"), None);
    assert_eq!(unix_from_rfc3339("soon"), None);
}

#[test]
fn tailscale_reports_connection_and_days_until_the_node_key_expires() {
    let healthy = tailscale_state(&tailnet(Some("2027-05-04T04:29:10Z")), NOW);
    assert_eq!(healthy.state, State::Healthy);
    assert_eq!(healthy.summary, "Connected · key expires in 176d");
    assert_eq!(healthy.details["key_expires_in_days"], 176);
    let disabled = tailscale_state(&tailnet(None), NOW);
    assert_eq!(disabled.state, State::Healthy);
    assert_eq!(disabled.summary, "Connected · key expiry off");
}

#[test]
fn tailscale_warns_before_expiry_and_is_down_once_disconnected_or_expired() {
    let soon = tailscale_state(&tailnet(Some("2026-11-20T00:00:00Z")), NOW);
    assert_eq!(soon.state, State::Degraded);
    assert_eq!(soon.summary, "Key expires in 10d");
    let expired = tailscale_state(&tailnet(Some("2026-11-01T00:00:00Z")), NOW);
    assert_eq!(expired.state, State::Down);
    assert_eq!(expired.summary, "Node key expired");
    let mut stopped = tailnet(None);
    stopped["BackendState"] = "NeedsLogin".into();
    let stopped = tailscale_state(&stopped, NOW);
    assert_eq!(stopped.state, State::Down);
    assert_eq!(stopped.summary, "Not connected (NeedsLogin)");
    let mut offline = tailnet(None);
    offline["Self"]["Online"] = false.into();
    assert_eq!(tailscale_state(&offline, NOW).state, State::Degraded);
    let mut warned = tailnet(None);
    warned["Health"] = json!(["private warning text"]);
    let warned = tailscale_state(&warned, NOW);
    assert_eq!(warned.state, State::Degraded);
    assert_eq!(warned.summary, "1 Tailscale warning");
    assert!(!warned.details.to_string().contains("private warning text"));
    assert_eq!(tailscale_state(&json!({}), NOW).state, State::Unknown);
}
