# trustify-client

Async Rust bindings for the Trustify REST API. The low-level generated API is
available as `trustify_client::api`; `TrustifyClient` configures shared request
behavior such as bearer-token acquisition for both generated and ergonomic
calls.

API date-time fields use `jiff::Timestamp` (UTC instants), and date-only fields
use `jiff::civil::Date`. Both serialize as strings in the API's date formats.

```rust,no_run
use trustify_client::{RetryPolicy, TrustifyClient};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let client = TrustifyClient::builder("https://trustify.example")
    .retry_policy(RetryPolicy::for_idempotent_requests(2))
    .build()?;

let info = client.api().info().send().await?;
println!("Trustify: {:#?}", info.into_inner());
# Ok(())
# }
```

Authentication is anonymous by default. To attach a fixed token, use
`bearer_token`; for refreshed or OIDC-issued tokens, implement
`AccessTokenProvider` and pass it with `token_provider`. Login flows and token
storage remain the responsibility of the calling application. Retries are
disabled by default; opt into bounded retries for GET/HEAD requests with
`RetryPolicy::for_idempotent_requests`.

HTTP response compression is enabled for gzip, Brotli, Zstandard, and deflate.
`reqwest` advertises supported encodings on requests and transparently
decompresses compressed responses. Request bodies are not compressed.

The OpenAPI source is pinned under `openapi/`. Run `scripts/generate-rust.sh`
after updating it; generated bindings are checked in at
`rust/trustify-client/src/api_generated.rs`.
