# Ministat

A quiet native Mac server monitor, written in Rust. One executable provides a
boot collector, menu-bar app and read-only JSON CLI for Hermes or any terminal agent.
No browser runtime, database, cloud account or listening network API.

## On the Mac mini

The app is `/Users/albovsky/Applications/Ministat.app`. Click its two-line
**CPU / RAM / INT / EXT / UP** readout to open the detail panel. Uptime sits at
the far right in compact forms such as `1h11m`; the panel keeps its full spacing.
Click it again
or switch to another app to dismiss. **Quit** exits the menu UI; collection
continues independently. The menu app returns at login, and the collector starts
at boot before login, supervised by launchd under the normal `albovsky` account.

```sh
~/.local/bin/ministat status
~/.local/bin/ministat status --json
~/.local/bin/ministat health --json
~/.local/bin/ministat processes --limit 5 --json

# From the administration Mac:
ssh pett-macmini '~/.local/bin/ministat status --json'
```

`status` and `health` return the same versioned snapshot, with different exit
semantics: **0** means current (`health`: also healthy), **1** means degraded
health, and **2** means missing, stale, incompatible or invalid collector data.
JSON errors are written to stdout as `{"schema_version":1,"error":{...}}`.
Check both the exit code and freshness fields. Stale service states are replaced
with `unknown`; old green results never masquerade as current health.

## What the numbers mean

- **Up:** time since boot. **Awake:** cumulative awake clock, excluding sleep;
  this is not time since the last wake event. Raw OS clocks can differ slightly
  at startup.
- **CPU:** all-core system utilization, sampled every two seconds. The first
  sample is warming up. The panel shows about two minutes of recent CPU history.
- **RAM:** app memory excluding purgeable pages, plus wired and compressed memory,
  using sysinfo's macOS definition. Available memory includes reclaimable pages
  and is not simply `total - used`. Pressure and swap give the useful context.
- **Internal:** APFS shared capacity at `/System/Volumes/Data`, using container
  capacity minus free blocks. This avoids counting only the sealed system volume
  or summing APFS sibling volumes. **Media:** the actual `/Volumes/Media` mount.
  Values use GiB. An empty mount directory is explicitly disconnected.
- **HomeKit:** authenticated loopback device API, available/expected lights and
  a reachable HAP listener. The private bridge token never enters output.
- **Jellyfin:** HTTP 200 and `Healthy` from loopback `/health`.
- **CI:** the configured local Runner.Listener processes and descendant workers.
  Local readiness does **not** verify GitHub connectivity, runner registration,
  queue state or workflow success. JSON explicitly says `unverified`.

Metrics update every 2 seconds. Disk and health workers update every 30 seconds
without blocking CPU/RAM collection. The private atomic snapshot is considered
stale after 15 seconds; disk and health results after 90 seconds.
Snapshots are disposable, and writes do not force an SSD fsync.

## Configuration and logs

The private state directory is `~/Library/Application Support/Ministat` (mode 700),
with `snapshot.json`, `collector.log`, `collector.lock`, and optional `config.json`.
The defaults match the current Mac mini. To customize, write an absolute-path
configuration and restart the collector:

```json
{
  "external_mount": "/Volumes/Media",
  "homekit_data": "/Users/albovsky/Projects/govee-lights/data",
  "jellyfin_port": 8096,
  "runner_roots": [
    "/Users/albovsky/actions-runners/pett-macmini-simple",
    "/Users/albovsky/actions-runners/pett-review"
  ]
}
```

Health HTTP requests go only to loopback with proxies and redirects disabled,
2-second deadlines and a 256 KiB response cap. The collector has no restart,
light-control, workflow-dispatch or other service mutation command.

## Build and run

Requires an Apple silicon Mac, macOS 14+, Xcode command-line tools and Rust 1.95+. The initial build
used Rust 1.99.1 in project-local `.tools/`; that toolchain is not deployed to the
Mac mini. The Cargo lockfile pins dependencies.

```sh
./scripts/cargo.sh test
./scripts/cargo.sh clippy --all-targets -- -D warnings
./scripts/cargo.sh fmt --check
./scripts/build.sh
./script/build_and_run.sh --preview
```

The signed development bundle is `dist/Ministat.app` (ad-hoc signed, no Developer
ID/notarization). Production is menu-only. `--preview` adds a Dock presence and
keeps the native panel visible for UI inspection. An isolated local collector
can run with `ministat collect --state-dir /tmp/ministat-preview`; pass the same
directory to `ministat menu --preview --state-dir /tmp/ministat-preview`.

## Install, update and disable

Run `./scripts/install.sh` as the target account. It stages verified app/binary
copies, preserves previous versions in the private `rollback/` directory, installs
the JSON CLI link and menu login job, and starts a temporary login collector if
the system service has not been installed yet. Run the printed administrator
command once to enable collection before login:

```sh
sudo "$HOME/Library/Application Support/Ministat/install-boot-service.sh"
```

The installer replaces paths atomically and restarts owned processes on update.
The system job is `com.albovsky.ministat.collector`; the login job is
`com.albovsky.ministat.menu`. The installer does not uninstall other monitors.

To disable boot and menu startup while retaining all files:

```sh
sudo "$HOME/Library/Application Support/Ministat/install-boot-service.sh" --remove
```

To roll back an update, use `./scripts/rollback.sh` with the absolute path to a
chosen `rollback/<timestamp>/` directory. It verifies the candidate, preserves
the current release, replaces paths atomically and restarts the owned processes.
Never overwrite the contents of an executable that is running. The prior monitor
apps can be retained separately for rollback.
