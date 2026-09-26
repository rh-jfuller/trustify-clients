# Releasing `trustify-client`

This guide covers releasing the Rust library crate in `rust/trustify-client/` to crates.io. The release workflow does not build platform binaries or create a GitHub Release.

## Prerequisites

- Push access to `rh-jfuller/trustify-clients`
- Permission to publish `trustify-client` on crates.io
- A GitHub Actions environment named `crates-io` with `CARGO_REGISTRY_TOKEN` configured as an environment secret

## Release process

### 1. Prepare a release PR

Update the package version in `rust/trustify-client/Cargo.toml`:

```toml
[package]
name = "trustify-client"
version = "0.1.0"
```

Run the workspace checks and package dry-run from the repository root:

```sh
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets --all-features -- -D warnings
cargo test --manifest-path rust/Cargo.toml --workspace --all-features
cargo publish --manifest-path rust/trustify-client/Cargo.toml --dry-run
```

Open and merge the release PR before creating the tag.

### 2. Tag and push

```sh
git tag v0.1.0
git push origin v0.1.0
```

The tag version must exactly match the crate version. Stable tags use `vMAJOR.MINOR.PATCH` (for example, `v0.1.0`). Tags with a prerelease suffix, such as `v0.2.0-rc.1`, are also accepted for validation.

### 3. What happens automatically

Pushing a matching tag triggers `.github/workflows/release.yml`:

| Job | What it does |
|-----|-------------|
| `init` | Extracts the tag version and identifies whether it is stable or prerelease |
| `check` | Verifies the package version, runs workspace tests, and dry-runs publishing the crate |
| `publish` | Publishes `trustify-client` to crates.io for stable tags only |

Prerelease tags run the version check, tests, and publish dry-run, but skip the crates.io publish. A stable tag publishes the library crate; it does not create a GitHub Release or attach binary artifacts.

### 4. Verify

- Check the [GitHub Actions workflow](https://github.com/rh-jfuller/trustify-clients/actions/workflows/release.yml) for status.
- After publication, check [`trustify-client` on crates.io](https://crates.io/crates/trustify-client).

## Pre-release

Use a prerelease version in `rust/trustify-client/Cargo.toml`, merge it, and push a matching tag:

```sh
git tag v0.2.0-rc.1
git push origin v0.2.0-rc.1
```

The workflow validates the matching prerelease version and runs tests and the publish dry-run, but does not publish it to crates.io.

## Troubleshooting

**Version mismatch error:**
The `check` job fails unless the tag (without its leading `v`) matches the version in `rust/trustify-client/Cargo.toml`. Correct the package version in a commit, then create and push a new matching tag.

**crates.io publish fails:**
Verify that `CARGO_REGISTRY_TOKEN` is present in the GitHub Actions `crates-io` environment and has permission to publish `trustify-client`. Run the package dry-run locally before tagging.

**Publish job does not run:**
Only stable tags matching `vMAJOR.MINOR.PATCH` publish. Prerelease tags intentionally stop after validation.
