# Repository instructions

## CI resources

Run all Actions jobs on the repository-scoped Mac mini pool. Never add
GitHub-hosted runners or hosted fallbacks, GitHub artifact uploads, or Actions
caches. Keep tools/build caches on the Mini SSD and preserve the owner/source
gates. See [CI operations](docs/ci.md). Run
`ruby scripts/verify_ci_resource_policy.rb` before publishing workflow changes.
