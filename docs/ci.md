# CI

This repository is public, so every GitHub Actions job runs on a GitHub-hosted
`macos-latest` runner. Hosted minutes are free for public repositories.
Self-hosted runners are not allowed: they would let pull-request code, including
code from forks, execute on a persistent machine.

The workflow uses a read-only `GITHUB_TOKEN`, `pull_request` (never
`pull_request_target`), and checkout without persisted credentials. Fork PRs run
with GitHub's normal restrictions; first-time contributors need approval.

Rust is pinned to 1.99.1 with rustfmt and clippy. Formatting, clippy, Rust tests,
administrator-staging Python tests and the release build are all CI gates.

Run `ruby scripts/verify_ci_resource_policy.rb` before publishing workflow changes.
CI also runs this policy and its regression tests. The parser rejects
self-hosted or dynamic runner selection, reusable workflows, artifact and cache
actions, and unapproved action pins, including nested local composite actions.
Review an action's behavior before adding it to the allowlist.
