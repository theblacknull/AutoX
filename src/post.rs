use std::path::Path;

use chrono::Utc;

use crate::config::{load_credentials_from_env, wants_publish};
use crate::error::AutoxError;
use crate::media::upload_media;
use crate::types::{
    CreatePostRequest, CreateTweetBody, MediaRef, PostPayload, PostResult, TweetMedia, TweetReply,
};
use crate::validate::validate_request;
use crate::x_client::XClient;

/// Create a post. **Dry-run by default** — writes draft JSON, never calls X.
/// Set `publish: true` to hit X API v2. Importing this crate never posts.
pub async fn create_post(
    client: &XClient,
    req: CreatePostRequest,
) -> Result<PostResult, AutoxError> {
    let weighted_length = validate_request(&req)?;
    let text = req.text.trim().to_string();

    let media_refs: Vec<MediaRef> = req
        .media_paths
        .iter()
        .map(|p| MediaRef {
            path: p.clone(),
            media_id: None,
            mime_type: None,
            size_bytes: None,
        })
        .collect();

    let payload = PostPayload {
        text: text.clone(),
        weighted_length,
        media: if media_refs.is_empty() {
            None
        } else {
            Some(media_refs)
        },
        reply_to_post_id: req.reply_to_post_id.clone(),
        quote_post_id: req.quote_post_id.clone(),
        created_at: Utc::now().to_rfc3339(),
    };

    if !wants_publish(req.publish) {
        let draft_path = write_draft(&req.draft_dir, &payload).await?;
        return Ok(PostResult::dry_run(payload, draft_path));
    }

    let credentials = load_credentials_from_env()?;

    // Upload media sequentially — X requires ordered segments within a file;
    // multiple images could theoretically be parallel, but sequential is correct and simple.
    let mut uploaded: Vec<MediaRef> = Vec::with_capacity(req.media_paths.len());
    for path in &req.media_paths {
        uploaded.push(upload_media(client.http(), &credentials, path).await?);
    }

    let media_ids: Vec<String> = uploaded
        .iter()
        .filter_map(|m| m.media_id.clone())
        .collect();

    let body = CreateTweetBody {
        text: &text,
        media: if media_ids.is_empty() {
            None
        } else {
            Some(TweetMedia {
                media_ids: &media_ids,
            })
        },
        reply: req
            .reply_to_post_id
            .as_deref()
            .map(|id| TweetReply {
                in_reply_to_tweet_id: id,
            }),
        quote_tweet_id: req.quote_post_id.as_deref(),
    };

    let created = client.create_tweet(&credentials, &body).await?;
    // Accuracy: only treat as published when we have a post id from X.
    let url = format!("https://x.com/i/web/status/{}", created.post_id);

    let published_payload = PostPayload {
        media: if uploaded.is_empty() {
            None
        } else {
            Some(uploaded)
        },
        ..payload
    };

    Ok(PostResult::published(
        published_payload,
        created.post_id,
        url,
        Some(created.rate_limit),
    ))
}

async fn write_draft(draft_dir: &str, payload: &PostPayload) -> Result<String, AutoxError> {
    tokio::fs::create_dir_all(draft_dir).await?;
    let stamp = payload.created_at.replace([':', '.'], "-");
    let path = Path::new(draft_dir).join(format!("draft-{stamp}.json"));
    let document = serde_json::json!({
        "mode": "dry_run",
        "note": "Not sent to X. Re-run with --publish to post.",
        "payload": payload,
    });
    let bytes = serde_json::to_vec_pretty(&document)
        .map_err(|e| AutoxError::Other(e.to_string()))?;
    tokio::fs::write(&path, bytes).await?;
    Ok(path.to_string_lossy().into_owned())
}
