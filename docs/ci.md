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

Project checks run in a disposable SSD copy of the checkout; tool/cache roots are under
`/Volumes/Media/PettCI/repositories/ministat/`. Jobs fail if the SSD is unmounted. The runner uses a small internal bootstrap
checkout because macOS blocks service startup with an external work directory.
Project commands run on the SSD copy, which is removed after the job; caches stay
local. Advisory review jobs use only the internal bootstrap checkout. No release/signing credentials are installed for this
runner. Normal check output remains in GitHub logs; build outputs remain local.
The existing account-wide $0 Actions budget is a separate paid-usage backstop.

Run `ruby scripts/verify_ci_resource_policy.rb` before publishing workflow changes.
CI also runs this policy and its regression tests. The parser rejects hosted or
dynamic runner selection, reusable workflows, artifact/cache actions, and
unapproved action pins, including nested local composite actions. Review the
storage behavior before allowing another action; shell uploads are also forbidden.

The runner service lives in `~/actions-runners/ministat-macmini` on the Mini.
Use its `./svc.sh status`, `start`, or `stop` commands for service maintenance.
Keep all project dependencies at their existing locked versions.

Rust remains pinned to 1.99.1 with rustfmt and clippy. Its rustup/Cargo homes
and build target cache stay on the SSD. Formatting, clippy, Rust tests,
administrator-staging Python tests, and the Release build all remain CI gates.
This migration targets the app's existing feature branch; it does not merge or
release the application itself.
