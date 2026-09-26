use std::time::Duration;

/// Opt-in retry settings for idempotent API reads.
///
/// Only GET and HEAD requests are retried, and only for connection/time-out
/// failures or HTTP 429, 502, 503, and 504 responses. Request bodies and
/// non-idempotent methods are never retried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    max_retries: u32,
    initial_backoff: Duration,
    max_backoff: Duration,
}

impl RetryPolicy {
    /// Disable retries.
    pub const fn disabled() -> Self {
        Self {
            max_retries: 0,
            initial_backoff: Duration::ZERO,
            max_backoff: Duration::ZERO,
        }
    }

    /// Retry safe reads up to `max_retries` times with exponential backoff
    /// starting at 100 ms and capped at 2 seconds.
    pub const fn for_idempotent_requests(max_retries: u32) -> Self {
        Self {
            max_retries,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(2),
        }
    }

    /// Create a policy with explicit backoff values.
    pub const fn new(max_retries: u32, initial_backoff: Duration, max_backoff: Duration) -> Self {
        Self {
            max_retries,
            initial_backoff,
            max_backoff,
        }
    }

    pub(crate) const fn max_retries(self) -> u32 {
        self.max_retries
    }

    pub(crate) fn delay(self, attempt: u32) -> Duration {
        let multiplier = 1_u32.checked_shl(attempt.min(31)).unwrap_or(u32::MAX);
        self.initial_backoff
            .saturating_mul(multiplier)
            .min(self.max_backoff)
    }

    pub(crate) fn response_delay(self, response: &reqwest::Response, attempt: u32) -> Duration {
        response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_secs)
            .map(|delay| delay.min(self.max_backoff))
            .unwrap_or_else(|| self.delay(attempt))
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::disabled()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::RetryPolicy;

    #[test]
    fn backoff_grows_and_is_capped() {
        let policy = RetryPolicy::for_idempotent_requests(5);

        assert_eq!(policy.delay(0), Duration::from_millis(100));
        assert_eq!(policy.delay(1), Duration::from_millis(200));
        assert_eq!(policy.delay(8), Duration::from_secs(2));
    }
}
