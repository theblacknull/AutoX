use serde::{Deserialize, Serialize};

use crate::error::ErrorDto;

/// Classic post weighted-length limit (X counts most CJK as 2). Enforced before any network call.
pub const X_WEIGHTED_CHAR_LIMIT: usize = 280;

/// Absolute UTF-8 byte cap to reject absurd payloads early.
pub const X_TEXT_BYTE_CAP: usize = 8 * 1024;

/// Max images per post. Video or GIF: exactly 1, no mix with images.
pub const X_MAX_IMAGES: usize = 4;

/// Images under this size use simple upload; larger / video / GIF use chunked INIT/APPEND/FINALIZE.
pub const MEDIA_SIMPLE_MAX_BYTES: u64 = 5 * 1024 * 1024;

pub const MEDIA_IMAGE_MAX_BYTES: u64 = 5 * 1024 * 1024;
pub const MEDIA_VIDEO_MAX_BYTES: u64 = 512 * 1024 * 1024;

fn default_draft_dir() -> String {
    "drafts".into()
}

#[derive(Debug, Clone)]
pub struct CreatePostRequest {
    pub text: String,
    pub media_paths: Vec<String>,
    /// Explicit `true` required to hit X. Default = dry-run.
    pub publish: bool,
    pub draft_dir: String,
    pub reply_to_post_id: Option<String>,
    pub quote_post_id: Option<String>,
}

impl Default for CreatePostRequest {
    fn default() -> Self {
        Self {
            text: String::new(),
            media_paths: Vec::new(),
            publish: false,
            draft_dir: default_draft_dir(),
            reply_to_post_id: None,
            quote_post_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaRef {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostPayload {
    pub text: String,
    pub weighted_length: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<Vec<MediaRef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_post_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_post_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostMode {
    DryRun,
    Published,
}

/// Result of create_post. `Published` only when X returned a post id.
#[derive(Debug, Clone, Serialize)]
pub struct PostResult {
    pub ok: bool,
    pub mode: PostMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<PostPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimitInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorDto>,
}

impl PostResult {
    pub fn dry_run(payload: PostPayload, draft_path: String) -> Self {
        Self {
            ok: true,
            mode: PostMode::DryRun,
            payload: Some(payload),
            draft_path: Some(draft_path),
            post_id: None,
            url: None,
            rate_limit: None,
            error: None,
        }
    }

    pub fn published(
        payload: PostPayload,
        post_id: String,
        url: String,
        rate_limit: Option<RateLimitInfo>,
    ) -> Self {
        Self {
            ok: true,
            mode: PostMode::Published,
            payload: Some(payload),
            draft_path: None,
            post_id: Some(post_id),
            url: Some(url),
            rate_limit,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RateLimitInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_unix: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct XCredentials {
    pub api_key: String,
    pub api_secret: String,
    pub access_token: String,
    pub access_token_secret: String,
}

#[derive(Debug, Serialize)]
pub struct CreateTweetBody<'a> {
    pub text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<TweetMedia<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply: Option<TweetReply<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_tweet_id: Option<&'a str>,
}

#[derive(Debug, Serialize)]
pub struct TweetMedia<'a> {
    pub media_ids: &'a [String],
}

#[derive(Debug, Serialize)]
pub struct TweetReply<'a> {
    pub in_reply_to_tweet_id: &'a str,
}
