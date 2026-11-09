use crate::{
    config::Config,
    model::{Health, Service, State, now_unix},
    storage::read_json,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    io::{self, Read},
    net::{IpAddr, SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub fn http_get(
    port: u16,
    path: &str,
    token: Option<&str>,
    timeout: Duration,
) -> io::Result<Vec<u8>> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .proxy(None)
        .max_redirects(0)
        .timeout_global(Some(timeout))
        .max_response_header_size(16 * 1024)
        .input_buffer_size(4096)
        .output_buffer_size(4096)
        .build()
        .into();
    let mut request = agent.get(format!("http://127.0.0.1:{port}{path}"));
    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {token}"));
    }
    let mut response = request
        .call()
        .map_err(|_| io::Error::other("Loopback health request failed"))?;
    if response.status() != 200 {
        return Err(io::Error::other("Health endpoint did not return 200"));
    }
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(256 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 256 * 1024 {
        return Err(io::Error::other("Health response too large"));
    }
    Ok(bytes)
}

fn service(state: State, summary: impl Into<String>, details: Value) -> Service {
    Service {
        state,
        summary: summary.into(),
        details,
    }
}

pub fn homekit_state(rows: &Value, expected: usize, listener: bool) -> Service {
    let Some(rows) = rows.as_array() else {
        return service(State::Unknown, "Invalid device response", json!({}));
    };
    if rows.iter().any(|r| !r["available"].is_boolean()) {
        return service(State::Unknown, "Invalid device availability", json!({}));
    }
    let available = rows.iter().filter(|r| r["available"] == true).count();
    let state = if !listener {
        State::Down
    } else if expected > 0 && rows.len() == expected && available == expected {
        State::Healthy
    } else {
        State::Degraded
    };
    service(
        state,
        if listener {
            format!("{available}/{expected} lights responding")
        } else {
            "HomeKit listener unavailable".into()
        },
        json!({"available": available, "expected": expected, "reported": rows.len(), "listener_reachable": listener}),
    )
}

fn homekit(config: &Config) -> Service {
    let probe = || -> io::Result<Service> {
        let token: Value = read_json(&config.homekit_data.join("api-token.json"))?;
        let token = token["token"]
            .as_str()
            .ok_or_else(|| io::Error::other("Token missing"))?;
        let settings: Value = read_json(&config.homekit_data.join("config.json"))?;
        let expected = settings["devices"].as_array().map_or(0, Vec::len);
        let api_port = settings["api_port"].as_u64().unwrap_or(8765);
        let port = u16::try_from(api_port).map_err(io::Error::other)?;
        let rows: Value = serde_json::from_slice(&http_get(
            port,
            "/v1/devices",
            Some(token),
            Duration::from_secs(2),
        )?)?;
        let runtime: Value = read_json(&config.homekit_data.join("runtime.json"))?;
        let ip: IpAddr = runtime["address"]
            .as_str()
            .unwrap_or("")
            .parse()
            .map_err(io::Error::other)?;
        let hap_port = u16::try_from(runtime["homekit_port"].as_u64().unwrap_or(51828))
            .map_err(io::Error::other)?;
        let listener = runtime["running"] == true
            && TcpStream::connect_timeout(
                &SocketAddr::new(ip, hap_port),
                Duration::from_millis(500),
            )
            .is_ok();
        Ok(homekit_state(&rows, expected, listener))
    };
    probe().unwrap_or_else(|_| {
        service(
            State::Down,
            "Bridge unavailable or access denied",
            json!({}),
        )
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct Process {
    pub pid: u32,
    pub parent_pid: u32,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub executable: String,
}
impl Process {
    pub fn parse(line: &str) -> Option<Self> {
        let mut parts = line.split_whitespace();
        Some(Self {
            pid: parts.next()?.parse().ok()?,
            parent_pid: parts.next()?.parse().ok()?,
            cpu_percent: parts.next()?.parse().ok()?,
            memory_bytes: parts.next()?.parse::<u64>().ok()? * 1024,
            executable: parts.collect::<Vec<_>>().join(" "),
        })
    }
}

pub fn processes() -> io::Result<Vec<Process>> {
    let output = Command::new("/bin/ps")
        .args(["-axo", "pid=,ppid=,pcpu=,rss=,comm="])
        .env("LC_ALL", "C")
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other("Unable to inspect local processes"));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(Process::parse)
        .collect())
}

pub fn ci_state(processes: &[Process], roots: &[PathBuf]) -> Service {
    let mut running = 0;
    let mut busy_count = 0;
    let runners: Vec<Value> = roots
        .iter()
        .map(|root| {
            let executable = root.join("bin/Runner.Listener");
            let listener = processes
                .iter()
                .find(|p| std::path::Path::new(&p.executable) == executable);
            let mut children = listener.map(|p| vec![p.pid]).unwrap_or_default();
            for _ in 0..8 {
                let next: Vec<u32> = processes
                    .iter()
                    .filter(|p| children.contains(&p.parent_pid) && !children.contains(&p.pid))
                    .map(|p| p.pid)
                    .collect();
                if next.is_empty() {
                    break;
                }
                children.extend(next);
            }
            let busy = processes
                .iter()
                .any(|p| children.contains(&p.pid) && p.executable.ends_with("/Runner.Worker"));
            if listener.is_some() {
                running += 1;
            }
            if busy {
                busy_count += 1;
            }
            json!({"name": root.file_name().unwrap_or_default().to_string_lossy(),
            "running": listener.is_some(), "busy": busy, "pid": listener.map(|p| p.pid)})
        })
        .collect();
    let state = if roots.is_empty() {
        State::Unknown
    } else if running == 0 {
        State::Down
    } else if running == roots.len() {
        State::Healthy
    } else {
        State::Degraded
    };
    service(
        state,
        format!(
            "{running}/{} locally ready · {busy_count} busy",
            roots.len()
        ),
        json!({"runners": runners, "github_connectivity": "unverified"}),
    )
}

/// Hermes `/api/status`: healthy when the gateway runs and the overall status is ok.
/// `None` means the endpoint did not answer 200 within the deadline.
pub fn hermes_state(status: Option<&Value>, port: u16) -> Service {
    let Some(status) = status else {
        return service(
            State::Down,
            "Status endpoint not responding",
            json!({"port": port}),
        );
    };
    let (Some(running), Some(overall)) = (
        status["gateway_running"].as_bool(),
        status["overall"].as_str(),
    ) else {
        return service(
            State::Unknown,
            "Invalid status response",
            json!({"port": port}),
        );
    };
    let version = status["version"].as_str().unwrap_or("unknown");
    let details = json!({"port": port, "version": version, "overall": overall,
        "gateway_state": status["gateway_state"].as_str()});
    if !running {
        service(State::Down, "Gateway stopped", details)
    } else if overall != "ok" {
        service(State::Degraded, format!("Status {overall}"), details)
    } else {
        service(
            State::Healthy,
            format!("v{version} · gateway running"),
            details,
        )
    }
}

/// Seconds since the Unix epoch for an RFC 3339 time such as Go's
/// `2027-05-04T04:29:10.123Z` or `2027-05-04T06:29:10+02:00`. Times before 1970,
/// including Go's zero time, are `None`.
pub fn unix_from_rfc3339(text: &str) -> Option<u64> {
    let number = |range: std::ops::Range<usize>| -> Option<i64> {
        let digits = text.get(range)?;
        digits
            .bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| digits.parse().ok())?
    };
    let bytes = text.as_bytes();
    if bytes.len() < 20 || [bytes[4], bytes[7], bytes[10], bytes[13], bytes[16]] != *b"--T::" {
        return None;
    }
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let mut zone = &text[19..];
    if let Some(fraction) = zone.strip_prefix('.') {
        let digits = fraction.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        zone = &fraction[digits..];
    }
    let offset = match zone.as_bytes() {
        b"Z" | b"z" => 0,
        [sign @ (b'+' | b'-'), h1, h2, b':', m1, m2]
            if [h1, h2, m1, m2].iter().all(|b| b.is_ascii_digit()) =>
        {
            let value = |a: &u8, b: &u8| i64::from((a - b'0') * 10 + (b - b'0'));
            let minutes = value(h1, h2) * 60 + value(m1, m2);
            if *sign == b'+' {
                minutes * 60
            } else {
                -minutes * 60
            }
        }
        _ => return None,
    };
    // Days from civil date (Howard Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let seconds = days * 86_400 + hour * 3600 + minute * 60 + second - offset;
    u64::try_from(seconds).ok()
}

/// Warn this many days before the node key expires; after that the Mac drops off the tailnet.
pub const KEY_EXPIRY_WARNING_DAYS: u64 = 30;

/// `tailscale status --json --peers=false`. Warning texts are counted, never copied.
pub fn tailscale_state(status: &Value, now: u64) -> Service {
    let (Some(backend), Some(online)) = (
        status["BackendState"].as_str(),
        status["Self"]["Online"].as_bool(),
    ) else {
        return service(State::Unknown, "Invalid Tailscale status", json!({}));
    };
    let expiry = status["Self"]["KeyExpiry"]
        .as_str()
        .and_then(unix_from_rfc3339);
    let days = expiry.map(|at| at.saturating_sub(now) / 86_400);
    let warnings = status["Health"].as_array().map_or(0, Vec::len);
    let details = json!({"backend_state": backend, "online": online, "key_expiry_unix": expiry,
        "key_expires_in_days": days, "warnings": warnings});
    let (state, summary) = if backend != "Running" {
        (State::Down, format!("Not connected ({backend})"))
    } else if expiry.is_some_and(|at| at <= now) {
        (State::Down, "Node key expired".into())
    } else if !online {
        (State::Degraded, "Running, but not online".into())
    } else if let Some(days) = days.filter(|d| *d < KEY_EXPIRY_WARNING_DAYS) {
        (State::Degraded, format!("Key expires in {days}d"))
    } else if warnings > 0 {
        let plural = if warnings == 1 { "" } else { "s" };
        (
            State::Degraded,
            format!("{warnings} Tailscale warning{plural}"),
        )
    } else {
        let expiry = days.map_or("key expiry off".into(), |d| format!("key expires in {d}d"));
        (State::Healthy, format!("Connected · {expiry}"))
    };
    service(state, summary, details)
}

/// Runs `program args`, returning stdout only if it exits successfully within `timeout`.
fn run_with_deadline(program: &Path, args: &[&str], timeout: Duration) -> io::Result<Vec<u8>> {
    let mut child = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("No stdout"))?;
    // Drain stdout concurrently so a large reply cannot block the child on a full pipe.
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = (&mut stdout).take(256 * 1024 + 1).read_to_end(&mut bytes);
        bytes
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other("Command timed out"));
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let bytes = reader
        .join()
        .map_err(|_| io::Error::other("Reader failed"))?;
    if !status.success() || bytes.len() > 256 * 1024 {
        return Err(io::Error::other("Command failed"));
    }
    Ok(bytes)
}

fn tailscale(config: &Config) -> Service {
    if !config.tailscale_cli.is_file() {
        return service(State::Unknown, "Tailscale CLI not found", json!({}));
    }
    let status = run_with_deadline(
        &config.tailscale_cli,
        &["status", "--json", "--peers=false"],
        Duration::from_secs(3),
    )
    .ok()
    .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    match status {
        Some(status) => tailscale_state(&status, now_unix()),
        None => service(State::Unknown, "Tailscale status unavailable", json!({})),
    }
}

pub fn check_health(config: &Config) -> Health {
    let jellyfin = match http_get(
        config.jellyfin_port,
        "/health",
        None,
        Duration::from_secs(2),
    ) {
        Ok(body) if body == b"Healthy" || body == b"Healthy\n" => service(
            State::Healthy,
            "Server responding",
            json!({"port":config.jellyfin_port}),
        ),
        _ => service(
            State::Down,
            "Server health check failed",
            json!({"port":config.jellyfin_port}),
        ),
    };
    Health {
        sampled_at_unix: now_unix(),
        homekit: homekit(config),
        jellyfin,
        ci: processes()
            .map(|p| ci_state(&p, &config.runner_roots()))
            .unwrap_or_else(|_| service(State::Unknown, "Process check unavailable", json!({}))),
        hermes: hermes_state(
            http_get(
                config.hermes_port,
                "/api/status",
                None,
                Duration::from_secs(2),
            )
            .ok()
            .and_then(|body| serde_json::from_slice::<Value>(&body).ok())
            .as_ref(),
            config.hermes_port,
        ),
        tailscale: tailscale(config),
    }
}
