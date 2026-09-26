# Trustify Rust client

This repository contains an asynchronous Rust client for the [Trustify](https://github.com/guacsec/trustify) REST API. The `trustify-client` crate combines generated, typed endpoint bindings with shared request configuration for authentication, timeouts, pagination, and optional retries.

The crate targets Rust 1.85 or newer and is prepared for publication to crates.io.

## Install

Once the crate is available on crates.io, add it and an async runtime to your application:

```toml
[dependencies]
trustify-client = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Quick start

The generated API is available through `TrustifyClient::api()`:

```rust,no_run
use trustify_client::TrustifyClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = TrustifyClient::builder("https://trustify.example").build()?;

    let info = client.api().info().send().await?;
    println!("{:#?}", info.into_inner());
    Ok(())
}
```

The repository includes a runnable version of this example. By default it connects to `http://localhost:8080`; set `TRUSTIFY_URL` to use another server:

```sh
TRUSTIFY_URL=https://trustify.example \
  cargo run --manifest-path rust/Cargo.toml -p trustify-client --example info
```

## Authentication and request configuration

Requests are anonymous by default. Attach a fixed bearer token with:

```rust,no_run
use trustify_client::TrustifyClient;

let client = TrustifyClient::builder("https://trustify.example")
    .bearer_token("access-token")
    .build()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

For refreshed or OIDC-issued tokens, implement `AccessTokenProvider` and pass it with `.token_provider(...)`. The provider is queried for each request and returns the raw token, without the `Bearer` prefix. The application remains responsible for OIDC login, refresh policy, and token storage.

The builder accepts a reusable `reqwest::Client` and lets you set connection and request timeouts when it creates the HTTP client. Defaults are 15 seconds to connect and 120 seconds per request. A supplied `reqwest::Client` keeps its own timeout configuration.

Retries are disabled by default. To retry safe reads up to two times:

```rust,no_run
use trustify_client::{RetryPolicy, TrustifyClient};

let client = TrustifyClient::builder("https://trustify.example")
    .retry_policy(RetryPolicy::for_idempotent_requests(2))
    .build()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Only GET and HEAD requests are retried, for connection/time-out failures and HTTP 429, 502, 503, or 504 responses. Backoff starts at 100 ms and is capped at 2 seconds; a numeric `Retry-After` value is honored up to that cap. `RetryPolicy::new` allows custom retry and backoff values.

## Pagination

`collect_offset_pages` is a generic helper for offset/limit endpoints. Supply a callback that fetches a page and returns `OffsetPage { items, total }`; the helper advances the offset until the known total is collected, or—when the total is unknown—until a short or empty page is returned. Request errors are passed back to the caller, and the helper does not perform retries itself.

## Repository layout

- `rust/trustify-client/` — library crate, example, shared client, and generated API.
- `rust/trustify-client/src/api_generated.rs` — checked-in Progenitor output.
- `rust/xtask/` — OpenAPI normalization and API generation.
- `openapi/` — pinned upstream Trustify OpenAPI source and version/hash metadata.
- `scripts/` — OpenAPI sync and Rust API generation scripts.
- `.github/workflows/` — CI and crates.io release workflows.

The checked-in API is generated from the Trustify `v0.6.2` specification at commit `b9d2627f83d189f0e7447b6bc0820f95bd061749`. The source spec is OpenAPI 3.1.0; `xtask` normalizes it for the pinned Progenitor 0.15.0 generator without changing the canonical spec file.

## Build and test

Run these commands from the repository root:

```sh
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets --all-features -- -D warnings
cargo test --manifest-path rust/Cargo.toml --workspace --all-features
```

Regenerate the checked-in client after changing the OpenAPI source or generator:

```sh
scripts/generate-rust.sh
```

To sync the OpenAPI source to a different Trustify Git ref, run `scripts/sync-openapi.sh <git-ref>`, update the commit and SHA-256 recorded in [`openapi/README.md`](openapi/README.md) from the script output, then regenerate the client.

## Release

Update the crate version in `rust/trustify-client/Cargo.toml`, then push a matching `vX.Y.Z` tag. The release workflow verifies the version, runs tests, and dry-runs packaging. Stable tags publish to crates.io; prerelease tags run validation without publishing. Configure `CARGO_REGISTRY_TOKEN` as a secret for the GitHub Actions `crates-io` environment.

## License

Licensed under Apache-2.0. See [`LICENSE`](LICENSE).
