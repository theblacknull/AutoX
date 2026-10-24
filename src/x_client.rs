use serde::Deserialize;

use crate::error::AutoxError;
use crate::http_util::ensure_success;
use crate::oauth1::build_oauth1_header;
use crate::types::{CreateTweetBody, RateLimitInfo, XCredentials};

const CREATE_TWEET_URL: &str = "https://api.x.com/2/tweets";
const ME_URL: &str = "https://api.x.com/2/users/me";

#[derive(Debug, Deserialize)]
struct TweetCreateResponse {
    data: Option<TweetData>,
}

#[derive(Debug, Deserialize)]
struct TweetData {
    id: Option<String>,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MeResponse {
    data: Option<MeData>,
}

#[derive(Debug, Deserialize)]
struct MeData {
    id: String,
    name: String,
    username: String,
}

#[derive(Debug)]
pub struct CreatedTweet {
    pub post_id: String,
    pub text: Option<String>,
    pub rate_limit: RateLimitInfo,
}

#[derive(Debug)]
pub struct MeInfo {
    pub id: String,
    pub name: String,
    pub username: String,
}

/// Direct X API v2 client. Holds one shared `reqwest::Client` for keep-alive.
#[derive(Clone)]
pub struct XClient {
    http: reqwest::Client,
}

impl XClient {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    pub fn shared() -> Result<Self, AutoxError> {
        Ok(Self::new(crate::http_util::build_shared_client()?))
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// `POST /2/tweets` — returns post id only when X includes `data.id`.
    pub async fn create_tweet(
        &self,
        credentials: &XCredentials,
        body: &CreateTweetBody<'_>,
    ) -> Result<CreatedTweet, AutoxError> {
        let auth = build_oauth1_header(credentials, "POST", CREATE_TWEET_URL, &[]);
        // Serialize once — no clone of the logical payload.
        let json = serde_json::to_vec(body)
            .map_err(|e| AutoxError::Other(format!("serialize tweet body: {e}")))?;

        let response = self
            .http
            .post(CREATE_TWEET_URL)
            .header("Authorization", auth)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(json)
            .send()
            .await?;

        let (value, rate_limit) = ensure_success(response, "create tweet").await?;

        let parsed: TweetCreateResponse = serde_json::from_value(value).map_err(|e| {
            AutoxError::XApi {
                status: 502,
                message: format!("invalid create tweet response: {e}"),
                rate_limit: rate_limit.clone(),
                details: None,
            }
        })?;

        let post_id = parsed
            .data
            .as_ref()
            .and_then(|d| d.id.clone())
            .ok_or_else(|| AutoxError::XApi {
                status: 502,
                message: "X response missing data.id — not treating as published".into(),
                rate_limit: rate_limit.clone(),
                details: None,
            })?;

        Ok(CreatedTweet {
            post_id,
            text: parsed.data.and_then(|d| d.text),
            rate_limit,
        })
    }

    /// `GET /2/users/me` — verify credentials without posting.
    pub async fn verify_me(&self, credentials: &XCredentials) -> Result<MeInfo, AutoxError> {
        let auth = build_oauth1_header(credentials, "GET", ME_URL, &[]);
        let response = self
            .http
            .get(ME_URL)
            .header("Authorization", auth)
            .header("Accept", "application/json")
            .query(&[("user.fields", "id,name,username")])
            .send()
            .await?;

        let (value, _) = ensure_success(response, "users/me").await?;
        let parsed: MeResponse =
            serde_json::from_value(value).map_err(|e| AutoxError::Other(e.to_string()))?;
        let data = parsed.data.ok_or_else(|| AutoxError::Other(
            "users/me response missing data".into(),
        ))?;
        Ok(MeInfo {
            id: data.id,
            name: data.name,
            username: data.username,
        })
    }
}
