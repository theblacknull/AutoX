use base64::{engine::general_purpose::STANDARD as B64, Engine};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha1::Sha1;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::XCredentials;

type HmacSha1 = Hmac<Sha1>;

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

fn normalize_params(params: &BTreeMap<&str, String>) -> String {
    params
        .iter()
        .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// Build OAuth 1.0a Authorization header. Pure — no network.
pub fn build_oauth1_header(
    credentials: &XCredentials,
    method: &str,
    url_without_query: &str,
    extra_params: &[(&str, &str)],
) -> String {
    let mut nonce_buf = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut nonce_buf);
    let nonce = hex::encode_simple(&nonce_buf);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs()
        .to_string();

    let mut params: BTreeMap<&str, String> = BTreeMap::new();
    params.insert("oauth_consumer_key", credentials.api_key.clone());
    params.insert("oauth_nonce", nonce);
    params.insert("oauth_signature_method", "HMAC-SHA1".into());
    params.insert("oauth_timestamp", timestamp);
    params.insert("oauth_token", credentials.access_token.clone());
    params.insert("oauth_version", "1.0".into());
    for (k, v) in extra_params {
        params.insert(*k, (*v).to_string());
    }

    let base = format!(
        "{}&{}&{}",
        method.to_uppercase(),
        percent_encode(url_without_query),
        percent_encode(&normalize_params(&params))
    );

    let signing_key = format!(
        "{}&{}",
        percent_encode(&credentials.api_secret),
        percent_encode(&credentials.access_token_secret)
    );

    let mut mac =
        HmacSha1::new_from_slice(signing_key.as_bytes()).expect("HMAC_SHA1 key length");
    mac.update(base.as_bytes());
    let signature = B64.encode(mac.finalize().into_bytes());
    params.insert("oauth_signature", signature);

    let mut header = String::from("OAuth ");
    let oauth_keys = [
        "oauth_consumer_key",
        "oauth_nonce",
        "oauth_signature",
        "oauth_signature_method",
        "oauth_timestamp",
        "oauth_token",
        "oauth_version",
    ];
    for (i, key) in oauth_keys.iter().enumerate() {
        if i > 0 {
            header.push_str(", ");
        }
        header.push_str(key);
        header.push_str("=\"");
        header.push_str(&percent_encode(params.get(key).unwrap()));
        header.push('"');
    }
    header
}

/// Tiny hex encoder to avoid an extra crate.
mod hex {
    pub fn encode_simple(bytes: &[u8]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut s = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            s.push(HEX[(b >> 4) as usize] as char);
            s.push(HEX[(b & 0xf) as usize] as char);
        }
        s
    }
}
