use crate::error::AutoxError;
use crate::types::XCredentials;

fn first_env(keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Ok(v) = std::env::var(key) {
            let t = v.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

/// Load OAuth 1.0a credentials from env. Call only on the publish path.
pub fn load_credentials_from_env() -> Result<XCredentials, AutoxError> {
    let api_key = first_env(&["X_API_KEY", "TWITTER_API_KEY"]);
    let api_secret = first_env(&["X_API_SECRET", "TWITTER_API_SECRET", "X_API_KEY_SECRET"]);
    let access_token = first_env(&["X_ACCESS_TOKEN", "TWITTER_ACCESS_TOKEN"]);
    let access_token_secret =
        first_env(&["X_ACCESS_TOKEN_SECRET", "TWITTER_ACCESS_TOKEN_SECRET"]);

    let mut missing = Vec::new();
    if api_key.is_none() {
        missing.push("X_API_KEY");
    }
    if api_secret.is_none() {
        missing.push("X_API_SECRET");
    }
    if access_token.is_none() {
        missing.push("X_ACCESS_TOKEN");
    }
    if access_token_secret.is_none() {
        missing.push("X_ACCESS_TOKEN_SECRET");
    }
    if !missing.is_empty() {
        return Err(AutoxError::MissingCredentials {
            missing: missing.join(", "),
        });
    }

    Ok(XCredentials {
        api_key: api_key.unwrap(),
        api_secret: api_secret.unwrap(),
        access_token: access_token.unwrap(),
        access_token_secret: access_token_secret.unwrap(),
    })
}

#[inline]
pub fn wants_publish(flag: bool) -> bool {
    flag
}
