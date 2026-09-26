use std::{sync::Arc, time::Duration};

use progenitor_client::{ClientHooks, ClientInfo, OperationInfo};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use thiserror::Error;
use url::Url;

use crate::{AccessTokenProvider, RetryPolicy, StaticBearerToken, api, auth::ClientContext};

/// Error returned while constructing a [`TrustifyClient`].
#[derive(Debug, Error)]
pub enum BuildError {
    /// The configured base URL is not an absolute HTTP or HTTPS URL.
    #[error("invalid Trustify base URL: {0}")]
    InvalidBaseUrl(#[from] url::ParseError),
    /// The base URL must use HTTP or HTTPS.
    #[error("Trustify base URL must use http or https, got `{0}`")]
    UnsupportedScheme(String),
    /// The underlying HTTP client could not be built.
    #[error("failed to build HTTP client: {0}")]
    HttpClient(#[from] reqwest::Error),
}

/// Shared, ergonomic client configuration for Trustify.
pub struct TrustifyClientBuilder {
    base_url: String,
    http_client: Option<reqwest::Client>,
    token_provider: Option<Arc<dyn AccessTokenProvider>>,
    retry_policy: RetryPolicy,
    connect_timeout: Duration,
    request_timeout: Duration,
}

impl TrustifyClientBuilder {
    /// Configure a reusable `reqwest::Client` (for example, to set proxy or
    /// TLS configuration).
    pub fn http_client(mut self, client: reqwest::Client) -> Self {
        self.http_client = Some(client);
        self
    }

    /// Set the maximum time allowed to establish a connection.
    ///
    /// This setting is used when the builder creates its own HTTP client. A
    /// client passed through [`http_client`](Self::http_client) keeps its own
    /// timeout configuration.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set the maximum duration for one HTTP request.
    ///
    /// This setting is used when the builder creates its own HTTP client. A
    /// client passed through [`http_client`](Self::http_client) keeps its own
    /// timeout configuration.
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Attach a provider whose access token will be sent on every API request.
    pub fn token_provider<P>(mut self, provider: P) -> Self
    where
        P: AccessTokenProvider + 'static,
    {
        self.token_provider = Some(Arc::new(provider));
        self
    }

    /// Attach a shared token provider, useful when it is already managed by an
    /// application-level OIDC session.
    pub fn shared_token_provider(mut self, provider: Arc<dyn AccessTokenProvider>) -> Self {
        self.token_provider = Some(provider);
        self
    }

    /// Configure a fixed bearer token.
    pub fn bearer_token(self, token: impl Into<String>) -> Self {
        self.token_provider(StaticBearerToken::new(token))
    }

    /// Configure opt-in retries for idempotent GET and HEAD requests.
    pub fn retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// Build the client. Requests are anonymous if no token provider was set.
    pub fn build(self) -> Result<TrustifyClient, BuildError> {
        let parsed = Url::parse(&self.base_url)?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(BuildError::UnsupportedScheme(parsed.scheme().to_owned()));
        }

        let base_url = self.base_url.trim_end_matches('/').to_owned();
        let http_client = match self.http_client {
            Some(client) => client,
            None => reqwest::Client::builder()
                .connect_timeout(self.connect_timeout)
                .timeout(self.request_timeout)
                .build()?,
        };
        let context = ClientContext {
            token_provider: self.token_provider,
            retry_policy: self.retry_policy,
        };
        let api = api::Client::new_with_client(&base_url, http_client, context);

        Ok(TrustifyClient { base_url, api })
    }
}

/// Shared client for a Trustify server.
#[derive(Clone, Debug)]
pub struct TrustifyClient {
    base_url: String,
    api: api::Client,
}

impl TrustifyClient {
    /// Start configuring a client for the given Trustify base URL.
    pub fn builder(base_url: impl Into<String>) -> TrustifyClientBuilder {
        TrustifyClientBuilder {
            base_url: base_url.into(),
            http_client: None,
            token_provider: None,
            retry_policy: RetryPolicy::default(),
            connect_timeout: Duration::from_secs(15),
            request_timeout: Duration::from_secs(120),
        }
    }

    /// Return the normalized base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Access the generated endpoint builders directly.
    pub fn api(&self) -> &api::Client {
        &self.api
    }
}

/// Shared request hook referenced by generated Progenitor operations.
///
/// Keeping this as a Progenitor hook ensures both the low-level generated API
/// and wrapper methods use the same token and wire-format behavior.
pub(crate) async fn prepare_request(
    context: &ClientContext,
    request: &mut reqwest::Request,
) -> Result<(), String> {
    if let Some(provider) = &context.token_provider {
        if let Some(token) = provider.access_token().await? {
            let value = HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|error| error.to_string())?;
            request.headers_mut().insert(AUTHORIZATION, value);
        }
    }

    // Progenitor's supported request-body MIME types exclude Trustify's JSON
    // merge-patch media type. The generation pass models it as JSON, and this
    // hook restores the required Content-Type on that PATCH route.
    if request.method() == reqwest::Method::PATCH
        && request.url().path().contains("/api/v3/importer/")
    {
        request.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/merge-patch+json"),
        );
    }

    Ok(())
}

impl ClientHooks<ClientContext> for api::Client {
    async fn exec(
        &self,
        request: reqwest::Request,
        _info: &OperationInfo,
    ) -> reqwest::Result<reqwest::Response> {
        let policy = self.inner().retry_policy;
        let mut request = request;
        let mut attempt = 0;

        loop {
            let safe_method = request.method() == reqwest::Method::GET
                || request.method() == reqwest::Method::HEAD;
            let retry_request = (safe_method && attempt < policy.max_retries())
                .then(|| request.try_clone())
                .flatten();

            match self.client().execute(request).await {
                Ok(response) => {
                    if let Some(next_request) = retry_request {
                        if is_retryable_status(response.status()) {
                            let delay = policy.response_delay(&response, attempt);
                            drop(response);
                            futures_timer::Delay::new(delay).await;
                            request = next_request;
                            attempt += 1;
                            continue;
                        }
                    }
                    return Ok(response);
                }
                Err(error) => {
                    if let Some(next_request) =
                        retry_request.filter(|_| error.is_timeout() || error.is_connect())
                    {
                        futures_timer::Delay::new(policy.delay(attempt)).await;
                        request = next_request;
                        attempt += 1;
                        continue;
                    }
                    return Err(error);
                }
            }
        }
    }
}

fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    matches!(
        status,
        reqwest::StatusCode::TOO_MANY_REQUESTS
            | reqwest::StatusCode::BAD_GATEWAY
            | reqwest::StatusCode::SERVICE_UNAVAILABLE
            | reqwest::StatusCode::GATEWAY_TIMEOUT
    )
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;
    use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};

    use super::prepare_request;
    use crate::{
        AccessTokenProvider, RetryPolicy,
        auth::{ClientContext, StaticBearerToken},
    };

    #[test]
    fn shared_request_hook_applies_bearer_token() {
        let context = ClientContext {
            token_provider: Some(std::sync::Arc::new(StaticBearerToken::new("test-token"))
                as std::sync::Arc<dyn AccessTokenProvider>),
            retry_policy: RetryPolicy::default(),
        };
        let mut request = reqwest::Client::new()
            .get("https://trustify.example/api/v3/sbom")
            .build()
            .unwrap();

        block_on(prepare_request(&context, &mut request)).unwrap();

        assert_eq!(request.headers()[AUTHORIZATION], "Bearer test-token");
    }

    #[test]
    fn request_hook_restores_merge_patch_content_type() {
        let mut request = reqwest::Client::new()
            .patch("https://trustify.example/api/v3/importer/example")
            .header(CONTENT_TYPE, "application/json")
            .build()
            .unwrap();

        block_on(prepare_request(&ClientContext::default(), &mut request)).unwrap();

        assert_eq!(
            request.headers()[CONTENT_TYPE],
            "application/merge-patch+json"
        );
    }
}
