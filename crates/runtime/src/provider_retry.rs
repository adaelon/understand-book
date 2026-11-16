//! Bounded recovery before a provider response stream has started.
use crate::{run_context::CancellationToken, AdapterError};
use serde_json::Value;
use std::time::{Duration, Instant, SystemTime};

pub(crate) const MAX_RETRIES: u32 = 3;

pub(crate) fn retryable_status(status: u16, body: Option<&Value>) -> bool {
    if !matches!(status, 408 | 429 | 500 | 502 | 503 | 504 | 529) {
        return false;
    }
    // Compatible providers also use 429 for exhausted credit. Structured
    // permanent codes override the status; free-form error prose does not.
    !body.is_some_and(|body| {
        let error = body.get("error").unwrap_or(body);
        ["code", "type"].into_iter().any(|field| {
            matches!(
                error[field].as_str(),
                Some(
                    "insufficient_quota"
                        | "insufficient_balance"
                        | "billing_hard_limit_reached"
                        | "credit_balance_too_low"
                        | "invalid_api_key"
                        | "authentication_error"
                        | "permission_error"
                )
            )
        })
    })
}

pub(crate) fn retry_after(response: &ureq::Response) -> Option<Duration> {
    response
        .header("Retry-After")
        .and_then(|value| parse_retry_after(value, SystemTime::now()))
        .or_else(|| {
            response
                .header("retry-after-ms")?
                .trim()
                .parse::<u64>()
                .ok()
                .map(Duration::from_millis)
        })
}

fn parse_retry_after(value: &str, now: SystemTime) -> Option<Duration> {
    let value = value.trim();
    value
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
        .or_else(|| {
            httpdate::parse_http_date(value)
                .ok()
                .map(|at| at.duration_since(now).unwrap_or_default())
        })
}

pub(crate) fn delay(retries: u32, retry_after: Option<Duration>) -> Duration {
    let base_ms = 1_000_u64 << retries.min(MAX_RETRIES - 1);
    // UUID v7's random tail disperses callers that hit the same shared quota.
    let jitter = u64::from(uuid::Uuid::now_v7().as_bytes()[15]) * base_ms / 1_024;
    Duration::from_millis(base_ms + jitter).max(retry_after.unwrap_or_default())
}

pub(crate) fn remaining(deadline: Instant) -> Result<Duration, AdapterError> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
        .ok_or_else(|| AdapterError {
            spend_stop: None,
            message: "模型调用超时（包含重试等待）".into(),
        })
}

pub(crate) fn wait(
    delay: Duration,
    deadline: Instant,
    cancellation: &CancellationToken,
) -> Result<(), AdapterError> {
    cancellation.check().map_err(|error| AdapterError {
        spend_stop: None,
        message: error.message,
    })?;
    if delay >= remaining(deadline)? {
        return Err(AdapterError {
            spend_stop: None,
            message: "模型服务要求的重试等待超过本次调用剩余时限".into(),
        });
    }
    let wake = Instant::now() + delay;
    loop {
        cancellation.check().map_err(|error| AdapterError {
            spend_stop: None,
            message: error.message,
        })?;
        remaining(deadline)?;
        let left = wake.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Ok(());
        }
        std::thread::sleep(left.min(Duration::from_millis(50)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_after_accepts_seconds_and_http_dates_without_shortening_them() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
        assert_eq!(
            parse_retry_after(" 60 ", now),
            Some(Duration::from_secs(60))
        );
        assert_eq!(
            parse_retry_after(&httpdate::fmt_http_date(now + Duration::from_secs(90)), now),
            Some(Duration::from_secs(90))
        );
        assert_eq!(
            parse_retry_after(&httpdate::fmt_http_date(now - Duration::from_secs(90)), now),
            Some(Duration::ZERO)
        );
        assert_eq!(parse_retry_after("invalid", now), None);
        assert_eq!(
            delay(0, Some(Duration::from_secs(60))),
            Duration::from_secs(60)
        );
        for retry in 0..MAX_RETRIES {
            let wait = delay(retry, None);
            let base = Duration::from_secs(1 << retry);
            assert!(wait >= base && wait < base.mul_f64(1.25));
        }
    }
}
