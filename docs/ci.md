# CI on the Mac mini

All GitHub Actions jobs use the repository-scoped `ministat-macmini` runner with
`[self-hosted, macOS, ARM64, ministat-ci]`. GitHub coordinates checks and retains
ordinary logs; no GitHub-hosted runner fallback, artifact uploads, or Actions
caches are allowed. An offline Mini leaves jobs queued.

Only `albovsky`-authored same-repository PRs and owner-triggered pushes/manual
runs can execute checkout code on this persistent runner. Fork and other-author
PRs need owner review before porting their changes to an eligible branch.
Runner registration is limited to this repository. The runner is a separate
service/workspace under the existing Mini account, not an OS sandbox.

Project checks run in a disposable local copy under
`/Users/albovsky/ci-data/ministat/work/`. Tool homes and caches also stay under
that project root on the Mini's internal disk. This avoids requiring interactive
macOS removable-volume permission for the service. The existing Pett SSD layout
is unchanged. No release/signing credentials are installed for this runner.
Advisory review jobs use the runner's small internal bootstrap checkout.

Before project checks, CI removes its own cache if it exceeds 3 GiB or space is
low, then requires at least 60 GiB free. It removes the disposable checkout after
the job and evicts caches over 3 GiB. The pinned tool installation is kept.
These limits protect room for Pett; low space fails the job without a hosted
fallback. Normal check output stays in GitHub logs. The existing account-wide
$0 Actions budget remains a separate paid-usage backstop.

Run `ruby scripts/verify_ci_resource_policy.rb` before publishing workflow changes.
CI also runs this policy and its regression tests. The parser rejects hosted or
dynamic runner selection, reusable workflows, artifact/cache actions, and
unapproved action pins, including nested local composite actions. Review the
storage behavior before allowing another action; shell uploads are also forbidden.

The runner service lives in `~/actions-runners/ministat-macmini` on the Mini.
Use its `./svc.sh status`, `start`, or `stop` commands for service maintenance.
Keep all project dependencies at their existing locked versions.

Rust remains pinned to 1.99.1 with rustfmt and clippy. Formatting, clippy, Rust
tests, administrator-staging Python tests, and the Release build all remain CI
gates. Cargo uses two build workers to share the Mini with other projects.
This migration targets the app's existing feature branch; it does not merge or
release the application itself.
