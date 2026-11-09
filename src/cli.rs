use crate::{
    config::{Config, state_dir},
    model::{Freshness, SCHEMA_VERSION, Snapshot, State, StatusView, duration_label, now_unix},
    storage::read_json,
};
use serde_json::json;
use std::{
    io,
    path::{Path, PathBuf},
};

pub fn load_view(dir: &Path) -> io::Result<StatusView> {
    let snapshot: Snapshot = read_json(&dir.join("snapshot.json"))?;
    if snapshot.schema_version != SCHEMA_VERSION {
        return Err(io::Error::other("Unsupported snapshot version"));
    }
    if snapshot.disks.len() != 2
        || snapshot.disks[0].name != "Internal"
        || snapshot.disks[1].name != "Media"
    {
        return Err(io::Error::other("Invalid snapshot disk layout"));
    }
    Ok(StatusView::new(
        snapshot,
        now_unix(),
        sysinfo::System::boot_time(),
    ))
}

pub fn run(args: &[String]) -> i32 {
    let json_output = args.iter().any(|s| s == "--json");
    match execute(args, json_output) {
        Ok(code) => code,
        Err((code, message)) => {
            if json_output {
                println!(
                    "{}",
                    json!({"schema_version":1,"error":{"code":code,"message":message}})
                );
            } else {
                eprintln!("ministat: {message}");
            }
            2
        }
    }
}

fn execute(args: &[String], json_output: bool) -> Result<i32, (&'static str, String)> {
    let invalid = |message: &str| ("invalid_arguments", message.to_owned());
    let command = args.first().map(String::as_str).unwrap_or("menu");
    if matches!(command, "--help" | "help" | "-h") {
        println!(
            "Ministat 0.1.0\n\nCommands:\n  menu [--preview] [--snapshot PNG]  Native menu-bar app; --snapshot renders the panel and exits\n  collect [--once]            Collect every 2 seconds (boot service)\n  status [--json]             Latest metrics and service health\n  health [--json]             Read-only health report; exit 1 if degraded\n  processes [--limit N] [--json]  Top CPU consumers (on demand)\n\nOptions: --state-dir PATH  --config PATH\nExit codes: 0 current/healthy, 1 degraded health, 2 unavailable/stale/error."
        );
        return Ok(0);
    }
    let mut dir = state_dir().map_err(|e| ("configuration", e.to_string()))?;
    let mut config_path = None;
    let mut once = false;
    let mut preview = false;
    let mut snapshot = None;
    let mut limit = 5;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => {}
            "--once" if command == "collect" => once = true,
            "--preview" if command == "menu" => preview = true,
            "--snapshot" if command == "menu" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| invalid("Missing path after option"))?;
                preview = true;
                snapshot = Some(PathBuf::from(value));
            }
            "--state-dir" | "--config" => {
                let flag = &args[i];
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| invalid("Missing path after option"))?;
                if flag == "--state-dir" {
                    dir = PathBuf::from(value);
                } else {
                    config_path = Some(PathBuf::from(value));
                }
            }
            "--limit" if command == "processes" => {
                i += 1;
                limit = args
                    .get(i)
                    .and_then(|s| s.parse::<usize>().ok())
                    .filter(|n| *n > 0 && *n <= 100)
                    .ok_or_else(|| invalid("Limit must be 1–100"))?;
            }
            _ => return Err(invalid("Unknown option; use ministat --help")),
        }
        i += 1;
    }
    match command {
        "collect" => {
            let config = Config::load(&config_path.unwrap_or_else(|| dir.join("config.json")))
                .map_err(|e| ("configuration", e.to_string()))?;
            crate::collector::collect(&dir, config, once)
                .map_err(|e| ("collector_error", e.to_string()))?;
            Ok(0)
        }
        "menu" => {
            crate::ui::run(dir, preview, snapshot);
            Ok(0)
        }
        "status" | "health" => {
            let view = load_view(&dir).map_err(|_| {
                (
                    "collector_unavailable",
                    "Collector unavailable. Start the boot service or run ministat collect.".into(),
                )
            })?;
            let stale = view.freshness != Freshness::Current;
            let health_stale = view.health_freshness != Freshness::Current
                || view.disks_freshness != Freshness::Current;
            let s = &view.snapshot;
            let unhealthy = s
                .health
                .services()
                .iter()
                .any(|(_, service)| service.state != State::Healthy)
                || s.disks.iter().any(|d| !d.mounted);
            if json_output {
                println!("{}", serde_json::to_string_pretty(&view).unwrap());
            } else if command == "health" {
                for (name, service) in s.health.services() {
                    println!("{name}: {}", service.summary);
                }
                println!(
                    "Collector: {:?} · {}s old",
                    view.freshness, view.age_seconds
                );
            } else {
                println!(
                    "{} · up {} · CPU {} · RAM {:.0}% · {:?}",
                    s.hostname,
                    duration_label(s.uptime_seconds),
                    s.cpu_percent
                        .map(|v| format!("{v:.0}%"))
                        .unwrap_or_else(|| "warming up".into()),
                    s.memory.used_percent,
                    view.freshness
                );
                if let Some(load) = &s.load {
                    println!(
                        "Load: {:.2} · {:.2} · {:.2} (1, 5, 15 min; {} cores)",
                        load.one_minute, load.five_minutes, load.fifteen_minutes, load.logical_cpus
                    );
                }
                for disk in &s.disks {
                    println!(
                        "{}: {}",
                        disk.name,
                        disk.used_percent
                            .map(|v| format!(
                                "{v:.0}% used · {:.1} GiB free",
                                disk.available_bytes.unwrap_or(0) as f64 / 1073741824.0
                            ))
                            .unwrap_or_else(|| "disconnected".into())
                    );
                }
            }
            Ok(if stale || (command == "health" && health_stale) {
                2
            } else if command == "health" && unhealthy {
                1
            } else {
                0
            })
        }
        "processes" => {
            let mut rows =
                crate::health::processes().map_err(|e| ("process_check_failed", e.to_string()))?;
            rows.sort_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent));
            rows.truncate(limit);
            if json_output {
                println!(
                    "{}",
                    json!({"schema_version":1,"sampled_at_unix":now_unix(),
                "cpu_percent_basis":"one logical core; OS ps estimate", "processes":rows})
                );
            } else {
                for p in rows {
                    println!(
                        "{:6} {:6.1}% {:7.1} MiB  {}",
                        p.pid,
                        p.cpu_percent,
                        p.memory_bytes as f64 / 1048576.0,
                        p.executable
                    );
                }
            }
            Ok(0)
        }
        _ => Err(invalid("Unknown command; use ministat --help")),
    }
}
