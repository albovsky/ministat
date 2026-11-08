# Read-only Mac mini monitoring

Run `/Users/albovsky/.local/bin/ministat status --json` locally, or
`ssh pett-macmini '~/.local/bin/ministat status --json'` remotely.

- Treat `schema_version: 1` as the supported schema.
- Check `freshness`, `health_freshness`, `disks_freshness`, and sample ages before
  interpreting readings. Missing/stale state exits 2; do not call it healthy.
- `health --json` exits 0 only for fresh healthy services and mounted storage,
  1 for degraded health, and 2 for unavailable/stale data.
- `processes --limit 5 --json` collects process usage on demand. CPU percentages
  are OS estimates per logical core, so a process can exceed 100%.
- CI local readiness is separate from GitHub availability, job success and queues;
  `github_connectivity` is deliberately `unverified`.
- These commands only observe. They do not authorize service restarts, light
  changes, deployments or CI workflow execution.

The HomeKit token must stay in the bridge's private data directory. Do not read
it for routine monitoring; Ministat reports only aggregate health.
