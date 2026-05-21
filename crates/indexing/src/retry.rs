//! Exponential-backoff retry helper for transport-bound operations.
//!
//! Used by the storage client (Codex sometimes 503s under load) and the
//! batch CLI's nwaku subscribe (the relay rejects subscribe-before-ready
//! for a few seconds after container boot).

use std::future::Future;
use std::time::Duration;

/// Configuration for [`with_retry`]. Construct via [`RetryConfig::default`]
/// for "5 attempts, 100 ms → 1.6 s exponential backoff".
#[derive(Clone, Copy, Debug)]
pub struct RetryConfig {
    /// Maximum number of attempts (including the first).
    pub attempts: u32,
    /// Backoff before the second attempt.
    pub base_delay: Duration,
    /// Multiplicative growth between attempts.
    pub growth: f64,
    /// Cap on any single delay.
    pub max_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            attempts: 5,
            base_delay: Duration::from_millis(100),
            growth: 2.0,
            max_delay: Duration::from_secs(10),
        }
    }
}

impl RetryConfig {
    /// Compute the delay before attempt N (1-indexed). N=1 returns zero.
    pub fn delay_for(&self, attempt: u32) -> Duration {
        if attempt <= 1 {
            return Duration::ZERO;
        }
        let raw_ms = self.base_delay.as_millis() as f64
            * self.growth.powi((attempt - 1) as i32 - 1);
        Duration::from_millis(raw_ms.min(self.max_delay.as_millis() as f64) as u64)
    }
}

/// Run a fallible async operation with exponential backoff. Stops on
/// first success; returns the last error if every attempt fails.
pub async fn with_retry<T, E, F, Fut>(cfg: RetryConfig, mut op: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let mut last_err = None;
    for attempt in 1..=cfg.attempts {
        let delay = cfg.delay_for(attempt);
        if !delay.is_zero() {
            tokio_sleep(delay).await;
        }
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.expect("attempts > 0 by construction"))
}

// Tiny shim so unit tests don't need a tokio runtime feature flag.
#[cfg(feature = "std")]
async fn tokio_sleep(_d: Duration) {}
#[cfg(not(feature = "std"))]
async fn tokio_sleep(_d: Duration) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn delay_for_attempt_1_is_zero() {
        let c = RetryConfig::default();
        assert_eq!(c.delay_for(1), Duration::ZERO);
    }

    #[test]
    fn delay_grows_exponentially() {
        let c = RetryConfig {
            attempts: 5,
            base_delay: Duration::from_millis(100),
            growth: 2.0,
            max_delay: Duration::from_secs(60),
        };
        assert_eq!(c.delay_for(2), Duration::from_millis(100));
        assert_eq!(c.delay_for(3), Duration::from_millis(200));
        assert_eq!(c.delay_for(4), Duration::from_millis(400));
        assert_eq!(c.delay_for(5), Duration::from_millis(800));
    }

    #[test]
    fn delay_capped_by_max_delay() {
        let c = RetryConfig {
            attempts: 10,
            base_delay: Duration::from_millis(100),
            growth: 10.0,
            max_delay: Duration::from_secs(1),
        };
        // 5th attempt would be 100 * 10^3 = 100_000 ms; capped to 1000 ms.
        assert_eq!(c.delay_for(5), Duration::from_secs(1));
    }

    #[tokio::test]
    async fn retries_until_success() {
        let calls = AtomicU32::new(0);
        let result: Result<u32, &str> = with_retry(
            RetryConfig {
                attempts: 5,
                base_delay: Duration::ZERO,
                growth: 1.0,
                max_delay: Duration::ZERO,
            },
            || async {
                let n = calls.fetch_add(1, Ordering::Relaxed) + 1;
                if n >= 3 {
                    Ok(n)
                } else {
                    Err("not yet")
                }
            },
        )
        .await;
        assert_eq!(result, Ok(3));
        assert_eq!(calls.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn returns_last_error_after_exhaustion() {
        let calls = AtomicU32::new(0);
        let result: Result<u32, &str> = with_retry(
            RetryConfig {
                attempts: 3,
                base_delay: Duration::ZERO,
                growth: 1.0,
                max_delay: Duration::ZERO,
            },
            || async {
                calls.fetch_add(1, Ordering::Relaxed);
                Err("nope")
            },
        )
        .await;
        assert_eq!(result, Err("nope"));
        assert_eq!(calls.load(Ordering::Relaxed), 3);
    }
}
