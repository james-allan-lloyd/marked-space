//! Retry policy for requests to Confluence.
//!
//! Confluence Cloud rate limits aggressively and answers with `429 Too Many Requests` (usually
//! with a `Retry-After` header) when a sync makes requests faster than the allowance. It also
//! occasionally returns transient 5xx errors. Both are retried with an exponential backoff.

use std::env;
use std::fmt;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::header::{HeaderMap, RETRY_AFTER};
use reqwest::{Method, StatusCode};

pub const MAX_RETRIES_ENV: &str = "MARKED_SPACE_MAX_RETRIES";
pub const INITIAL_BACKOFF_MS_ENV: &str = "MARKED_SPACE_RETRY_INITIAL_BACKOFF_MS";
pub const MAX_BACKOFF_SECS_ENV: &str = "MARKED_SPACE_RETRY_MAX_BACKOFF_SECS";

const DEFAULT_MAX_RETRIES: u32 = 8;
const DEFAULT_INITIAL_BACKOFF_MS: u64 = 500;
const DEFAULT_MAX_BACKOFF_SECS: u64 = 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryConfig {
    /// Number of retries *after* the initial attempt. Zero disables retrying.
    pub max_retries: u32,
    /// Backoff before the first retry. Doubles on every subsequent retry.
    pub initial_backoff: Duration,
    /// Upper bound for any single wait, including waits requested by `Retry-After`.
    pub max_backoff: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        RetryConfig {
            max_retries: DEFAULT_MAX_RETRIES,
            initial_backoff: Duration::from_millis(DEFAULT_INITIAL_BACKOFF_MS),
            max_backoff: Duration::from_secs(DEFAULT_MAX_BACKOFF_SECS),
        }
    }
}

impl RetryConfig {
    pub fn from_env() -> Self {
        let defaults = RetryConfig::default();
        RetryConfig {
            max_retries: env_parsed(MAX_RETRIES_ENV).unwrap_or(defaults.max_retries),
            initial_backoff: env_parsed(INITIAL_BACKOFF_MS_ENV)
                .map(Duration::from_millis)
                .unwrap_or(defaults.initial_backoff),
            max_backoff: env_parsed(MAX_BACKOFF_SECS_ENV)
                .map(Duration::from_secs)
                .unwrap_or(defaults.max_backoff),
        }
    }

    /// Wait before the retry following `attempt` (zero based), preferring the delay the server
    /// asked for over our own exponential backoff. Never waits longer than `max_backoff`, so a
    /// server asking us to come back in an hour still gets retried (and, if it's still rate
    /// limiting us, asked again) within a sensible time.
    pub fn delay_for(&self, attempt: u32, server_suggested: Option<Duration>) -> Duration {
        let delay = match server_suggested {
            Some(suggested) => suggested,
            None => self.backoff(attempt),
        };
        delay.min(self.max_backoff)
    }

    /// Exponential backoff with jitter: a random point in `[half, full]` of the current window,
    /// so that repeated collisions with the rate limiter don't line up.
    fn backoff(&self, attempt: u32) -> Duration {
        let window = self
            .initial_backoff
            .saturating_mul(1u32.checked_shl(attempt).unwrap_or(u32::MAX))
            .min(self.max_backoff);

        window.mul_f64(jitter_fraction())
    }
}

fn env_parsed<T: FromStr>(name: &str) -> Option<T> {
    env::var(name).ok()?.trim().parse().ok()
}

/// A pseudo random fraction in `[0.5, 1.0)`. The clock is a good enough entropy source for
/// spreading out retries, and saves pulling in a random number generator.
fn jitter_fraction() -> f64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    0.5 + 0.5 * f64::from(nanos % 1_000_000) / 1_000_000.0
}

#[derive(Debug, PartialEq, Eq)]
pub enum RetryReason {
    RateLimited,
    ServerError(StatusCode),
    Transport(String),
}

impl fmt::Display for RetryReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RetryReason::RateLimited => write!(f, "rate limited by Confluence (429)"),
            RetryReason::ServerError(status) => write!(f, "server error ({})", status),
            RetryReason::Transport(message) => write!(f, "{}", message),
        }
    }
}

/// Requests that can safely be repeated when we don't know whether the server processed them.
/// A rate limited request is never processed, so 429s are retried whatever the method is; the
/// remaining cases are only retried when repeating them can't create duplicates.
fn is_idempotent(method: &Method) -> bool {
    matches!(
        *method,
        Method::GET | Method::HEAD | Method::PUT | Method::DELETE | Method::OPTIONS | Method::TRACE
    )
}

pub fn classify_status(status: StatusCode, method: &Method) -> Option<RetryReason> {
    if status == StatusCode::TOO_MANY_REQUESTS {
        Some(RetryReason::RateLimited)
    } else if status.is_server_error() && is_idempotent(method) {
        Some(RetryReason::ServerError(status))
    } else {
        None
    }
}

pub fn classify_error(err: &reqwest::Error, method: &Method) -> Option<RetryReason> {
    // A connection that was never established can always be retried. Anything else may have
    // reached the server, so only repeat it for idempotent methods.
    let retryable =
        err.is_connect() || (is_idempotent(method) && (err.is_timeout() || err.is_request()));

    if retryable {
        Some(RetryReason::Transport(err.to_string()))
    } else {
        None
    }
}

/// The `Retry-After` delay in seconds. HTTP dates are valid in this header too, but Confluence
/// Cloud sends seconds, and falling back to our own backoff for anything else is harmless.
pub fn retry_after(headers: &HeaderMap) -> Option<Duration> {
    let value = headers.get(RETRY_AFTER)?.to_str().ok()?;
    value.trim().parse::<u64>().ok().map(Duration::from_secs)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::error::TestResult;
    use reqwest::header::HeaderValue;

    fn headers_with_retry_after(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, HeaderValue::from_str(value).unwrap());
        headers
    }

    #[test]
    fn it_parses_retry_after_seconds() -> TestResult {
        let headers = headers_with_retry_after("12");

        assert_eq!(retry_after(&headers), Some(Duration::from_secs(12)));

        Ok(())
    }

    #[test]
    fn it_ignores_unparseable_retry_after() -> TestResult {
        assert_eq!(retry_after(&HeaderMap::new()), None);
        assert_eq!(
            retry_after(&headers_with_retry_after("Wed, 21 Oct 2015 07:28:00 GMT")),
            None
        );

        Ok(())
    }

    #[test]
    fn it_backs_off_exponentially_up_to_the_maximum() -> TestResult {
        let config = RetryConfig {
            max_retries: 10,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(10),
        };

        for (attempt, window) in [(0, 1), (1, 2), (2, 4), (3, 8), (4, 10), (30, 10)] {
            let delay = config.delay_for(attempt, None);
            let window = Duration::from_secs(window);
            assert!(
                delay >= window / 2 && delay <= window,
                "attempt {} gave {:?}, expected within [{:?}, {:?}]",
                attempt,
                delay,
                window / 2,
                window
            );
        }

        Ok(())
    }

    #[test]
    fn it_prefers_the_server_suggested_delay() -> TestResult {
        let config = RetryConfig {
            max_retries: 10,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(30),
        };

        assert_eq!(
            config.delay_for(0, Some(Duration::from_secs(17))),
            Duration::from_secs(17)
        );

        Ok(())
    }

    #[test]
    fn it_caps_the_server_suggested_delay() -> TestResult {
        let config = RetryConfig {
            max_retries: 10,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(30),
        };

        assert_eq!(
            config.delay_for(0, Some(Duration::from_secs(3600))),
            Duration::from_secs(30)
        );

        Ok(())
    }

    #[test]
    fn it_retries_rate_limits_for_any_method() -> TestResult {
        for method in [Method::GET, Method::POST, Method::PUT, Method::DELETE] {
            assert_eq!(
                classify_status(StatusCode::TOO_MANY_REQUESTS, &method),
                Some(RetryReason::RateLimited),
                "{} should be retried when rate limited",
                method
            );
        }

        Ok(())
    }

    #[test]
    fn it_only_retries_server_errors_for_idempotent_methods() -> TestResult {
        assert_eq!(
            classify_status(StatusCode::SERVICE_UNAVAILABLE, &Method::GET),
            Some(RetryReason::ServerError(StatusCode::SERVICE_UNAVAILABLE))
        );
        assert_eq!(
            classify_status(StatusCode::SERVICE_UNAVAILABLE, &Method::POST),
            None
        );

        Ok(())
    }

    #[test]
    fn it_does_not_retry_client_errors() -> TestResult {
        assert_eq!(classify_status(StatusCode::NOT_FOUND, &Method::GET), None);
        assert_eq!(
            classify_status(StatusCode::UNAUTHORIZED, &Method::GET),
            None
        );
        assert_eq!(classify_status(StatusCode::CONFLICT, &Method::PUT), None);

        Ok(())
    }

    #[test]
    fn it_reads_config_from_env() -> TestResult {
        // Note: env vars are process wide, so this test sets and restores them itself.
        env::set_var(MAX_RETRIES_ENV, "3");
        env::set_var(INITIAL_BACKOFF_MS_ENV, "50");
        env::set_var(MAX_BACKOFF_SECS_ENV, "5");

        let config = RetryConfig::from_env();

        env::remove_var(MAX_RETRIES_ENV);
        env::remove_var(INITIAL_BACKOFF_MS_ENV);
        env::remove_var(MAX_BACKOFF_SECS_ENV);

        assert_eq!(
            config,
            RetryConfig {
                max_retries: 3,
                initial_backoff: Duration::from_millis(50),
                max_backoff: Duration::from_secs(5),
            }
        );

        Ok(())
    }
}
