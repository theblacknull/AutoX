//! AutoX — direct X API v2 posting library.
//!
//! Importing this crate never calls X and never reads credentials.
//! Only [`create_post`] with `publish: true` (or CLI `--publish`) publishes.
//!
//! # Surface
//! - **Library** ([`XClient`], [`create_post`]) — lowest latency, most X-compatible path
//! - **CLI** (`autox-post`) — thin wrapper for automation
//!
//! No HTTP gateway: that would add a hop. Call X from the library/CLI directly.

#![forbid(unsafe_code)]

mod config;
mod error;
mod http_util;
mod media;
mod oauth1;
mod post;
mod types;
mod validate;
mod x_client;

pub use config::{load_credentials_from_env, wants_publish};
pub use error::{AutoxError, ErrorDto};
pub use http_util::build_shared_client;
pub use post::create_post;
pub use types::{
    CreatePostRequest, CreateTweetBody, MediaRef, PostMode, PostPayload, PostResult, RateLimitInfo,
    TweetMedia, TweetReply, XCredentials, X_WEIGHTED_CHAR_LIMIT,
};
pub use validate::{validate_request, weighted_length};
pub use x_client::{CreatedTweet, MeInfo, XClient};
