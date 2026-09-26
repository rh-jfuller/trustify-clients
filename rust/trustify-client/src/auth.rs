use std::sync::Arc;

use async_trait::async_trait;

use crate::RetryPolicy;

/// Asynchronously provide an access token for a Trustify API request.
///
/// Implementations may refresh or obtain tokens from an OIDC client. The
/// provider is asked for a token on every request, so it can return a cached
/// token and refresh it when appropriate.
#[async_trait]
pub trait AccessTokenProvider: Send + Sync {
    /// Return the raw access token, without the `Bearer` prefix.
    async fn access_token(&self) -> Result<Option<String>, String>;
}

/// A fixed bearer token provider.
#[derive(Clone)]
pub struct StaticBearerToken(Arc<str>);

impl StaticBearerToken {
    /// Create a provider for a raw access token.
    pub fn new(token: impl Into<String>) -> Self {
        Self(Arc::from(token.into()))
    }
}

impl std::fmt::Debug for StaticBearerToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("StaticBearerToken")
            .field(&"[REDACTED]")
            .finish()
    }
}

#[async_trait]
impl AccessTokenProvider for StaticBearerToken {
    async fn access_token(&self) -> Result<Option<String>, String> {
        Ok(Some(self.0.to_string()))
    }
}

#[doc(hidden)]
#[derive(Clone, Default)]
pub struct ClientContext {
    pub(crate) token_provider: Option<Arc<dyn AccessTokenProvider>>,
    pub(crate) retry_policy: RetryPolicy,
}

impl std::fmt::Debug for ClientContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientContext")
            .field(
                "token_provider",
                &self.token_provider.as_ref().map(|_| "[configured]"),
            )
            .field("retry_policy", &self.retry_policy)
            .finish()
    }
}
