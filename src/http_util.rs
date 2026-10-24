use reqwest::header::HeaderMap;
use serde_json::Value;

use crate::error::AutoxError;
use crate::types::RateLimitInfo;

/// Shared reqwest client factory — call once, reuse forever (keep-alive pool).
pub fn build_shared_client() -> Result<reqwest::Client, AutoxError> {
    reqwest::Client::builder()
        .use_rustls_tls()
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .pool_max_idle_per_host(4)
        .tcp_keepalive(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(120))
        .user_agent(concat!("autox/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| AutoxError::Other(format!("failed to build HTTP client: {e}")))
}

pub fn parse_rate_limit(headers: &HeaderMap) -> RateLimitInfo {
    RateLimitInfo {
        limit: header_u32(headers, "x-rate-limit-limit"),
        remaining: header_u32(headers, "x-rate-limit-remaining"),
        reset_unix: header_u64(headers, "x-rate-limit-reset"),
        retry_after_seconds: header_u64(headers, "retry-after"),
    }
}

fn header_u32(headers: &HeaderMap, name: &str) -> Option<u32> {
    headers.get(name)?.to_str().ok()?.parse().ok()
}

fn header_u64(headers: &HeaderMap, name: &str) -> Option<u64> {
    headers.get(name)?.to_str().ok()?.parse().ok()
}

pub async fn ensure_success(
    response: reqwest::Response,
    context: &str,
) -> Result<(Value, RateLimitInfo), AutoxError> {
    let status = response.status();
    let rate_limit = parse_rate_limit(response.headers());
    let body_text = response.text().await.unwrap_or_default();
    let details: Option<Value> = if body_text.is_empty() {
        None
    } else {
        serde_json::from_str(&body_text).ok().or_else(|| {
            Some(Value::Object({
                let mut m = serde_json::Map::new();
                m.insert("raw".into(), Value::String(body_text.clone()));
                m
            }))
        })
    };

    if status.is_success() {
        let value = details.unwrap_or(Value::Null);
        return Ok((value, rate_limit));
    }

    let code = status.as_u16();
    if code == 429 {
        let hint = if let Some(reset) = rate_limit.reset_unix {
            format!("; resets at unix {reset}")
        } else if let Some(secs) = rate_limit.retry_after_seconds {
            format!("; retry after {secs}s")
        } else {
            String::new()
        };
        return Err(AutoxError::RateLimited {
            hint,
            rate_limit,
            details,
        });
    }

    let message = details
        .as_ref()
        .map(|d| d.to_string())
        .unwrap_or_else(|| status.canonical_reason().unwrap_or("error").to_string());

    Err(AutoxError::XApi {
        status: code,
        message: format!("{context}: {message}"),
        rate_limit,
        details,
    })
}
