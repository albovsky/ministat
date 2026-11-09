# Repository instructions

## CI

This repository is public. Run every Actions job on GitHub-hosted
`macos-latest`; never add self-hosted runners, `pull_request_target`, or write
permissions for PR code. Artifact uploads and Actions caches stay disallowed
unless deliberately reviewed. See [CI](docs/ci.md). Run
`ruby scripts/verify_ci_resource_policy.rb` before publishing workflow changes.
