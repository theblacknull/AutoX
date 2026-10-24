use serde::Serialize;
use thiserror::Error;

use crate::types::RateLimitInfo;

#[derive(Debug, Error)]
pub enum AutoxError {
    #[error("{message}")]
    InvalidInput { message: String },

    #[error("missing env credentials: {missing}")]
    MissingCredentials { missing: String },

    #[error("media file not found: {path}")]
    MediaNotFound { path: String },

    #[error("media error: {message}")]
    Media { message: String },

    #[error("X API rate limited{hint}")]
    RateLimited {
        hint: String,
        rate_limit: RateLimitInfo,
        details: Option<serde_json::Value>,
    },

    #[error("X API error (HTTP {status}): {message}")]
    XApi {
        status: u16,
        message: String,
        rate_limit: RateLimitInfo,
        details: Option<serde_json::Value>,
    },

    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

impl AutoxError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "INVALID_INPUT",
            Self::MissingCredentials { .. } => "MISSING_CREDENTIALS",
            Self::MediaNotFound { .. } => "MEDIA_NOT_FOUND",
            Self::Media { .. } => "MEDIA_ERROR",
            Self::RateLimited { .. } => "RATE_LIMITED",
            Self::XApi { .. } => "X_API_ERROR",
            Self::Network(_) => "NETWORK_ERROR",
            Self::Io(_) => "IO_ERROR",
            Self::Other(_) => "OTHER",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorDto {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimitInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl From<&AutoxError> for ErrorDto {
    fn from(err: &AutoxError) -> Self {
        let (rate_limit, details) = match err {
            AutoxError::RateLimited {
                rate_limit,
                details,
                ..
            } => (Some(rate_limit.clone()), details.clone()),
            AutoxError::XApi {
                rate_limit,
                details,
                ..
            } => (Some(rate_limit.clone()), details.clone()),
            _ => (None, None),
        };
        Self {
            code: err.code().to_string(),
            message: err.to_string(),
            rate_limit,
            details,
        }
    }
}
