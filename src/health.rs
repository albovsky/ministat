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
    path::PathBuf,
    process::Command,
    time::Duration,
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
    }
}
