use std::path::Path;

use bytes::Bytes;
use futures_util::stream;
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

use crate::error::AutoxError;
use crate::http_util::ensure_success;
use crate::oauth1::build_oauth1_header;
use crate::types::{
    MediaRef, XCredentials, MEDIA_SIMPLE_MAX_BYTES,
};

const UPLOAD_URL: &str = "https://upload.twitter.com/1.1/media/upload.json";
const CHUNK_SIZE: usize = 4 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct MediaUploadResponse {
    media_id_string: Option<String>,
    media_id: Option<u64>,
    processing_info: Option<ProcessingInfo>,
}

#[derive(Debug, Deserialize)]
struct ProcessingInfo {
    state: String,
    check_after_secs: Option<u64>,
    error: Option<ProcessingError>,
}

#[derive(Debug, Deserialize)]
struct ProcessingError {
    message: Option<String>,
}

fn media_id_from(resp: &MediaUploadResponse) -> Option<String> {
    resp.media_id_string
        .clone()
        .or_else(|| resp.media_id.map(|id| id.to_string()))
}

fn mime_and_category(path: &Path) -> Result<(String, String), AutoxError> {
    let mime = mime_guess::from_path(path)
        .first_or_octet_stream()
        .essence_str()
        .to_string();
    let category = if mime.starts_with("video/") {
        "tweet_video"
    } else if mime == "image/gif" {
        "tweet_gif"
    } else {
        "tweet_image"
    };
    Ok((mime, category.into()))
}

/// Upload one local file. Streams chunked APPEND from disk (no full-file buffer for large video).
pub async fn upload_media(
    client: &reqwest::Client,
    credentials: &XCredentials,
    path: &str,
) -> Result<MediaRef, AutoxError> {
    let p = Path::new(path);
    let meta = tokio::fs::metadata(p).await.map_err(|_| AutoxError::MediaNotFound {
        path: path.to_string(),
    })?;
    if !meta.is_file() {
        return Err(AutoxError::MediaNotFound {
            path: path.to_string(),
        });
    }

    let size = meta.len();
    let (mime, category) = mime_and_category(p)?;
    let needs_chunked = size > MEDIA_SIMPLE_MAX_BYTES
        || category == "tweet_video"
        || category == "tweet_gif";

    let media_id = if needs_chunked {
        chunked_upload(client, credentials, path, &mime, size, &category).await?
    } else {
        simple_upload(client, credentials, path, &mime).await?
    };

    Ok(MediaRef {
        path: path.to_string(),
        media_id: Some(media_id),
        mime_type: Some(mime),
        size_bytes: Some(size),
    })
}

async fn simple_upload(
    client: &reqwest::Client,
    credentials: &XCredentials,
    path: &str,
    mime: &str,
) -> Result<String, AutoxError> {
    // Small images: one multipart POST. Read file once (≤5MB).
    let bytes = tokio::fs::read(path).await?;
    let file_name = Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("media")
        .to_string();

    let auth = build_oauth1_header(credentials, "POST", UPLOAD_URL, &[]);
    let part = Part::bytes(bytes)
        .file_name(file_name)
        .mime_str(mime)
        .map_err(|e| AutoxError::Media {
            message: e.to_string(),
        })?;
    let form = Form::new().part("media", part);

    let response = client
        .post(UPLOAD_URL)
        .header("Authorization", auth)
        .multipart(form)
        .send()
        .await?;

    let (value, _) = ensure_success(response, "media simple upload").await?;
    let parsed: MediaUploadResponse =
        serde_json::from_value(value).map_err(|e| AutoxError::Media {
            message: format!("invalid upload response: {e}"),
        })?;
    media_id_from(&parsed).ok_or_else(|| AutoxError::Media {
        message: "upload returned no media_id".into(),
    })
}

async fn chunked_upload(
    client: &reqwest::Client,
    credentials: &XCredentials,
    path: &str,
    mime: &str,
    total_bytes: u64,
    media_category: &str,
) -> Result<String, AutoxError> {
    // INIT
    let init_params = [
        ("command", "INIT"),
        ("total_bytes", &total_bytes.to_string()),
        ("media_type", mime),
        ("media_category", media_category),
    ];
    // Need owned strings for form + oauth param refs
    let total_s = total_bytes.to_string();
    let init_oauth = [
        ("command", "INIT"),
        ("total_bytes", total_s.as_str()),
        ("media_type", mime),
        ("media_category", media_category),
    ];
    let auth = build_oauth1_header(credentials, "POST", UPLOAD_URL, &init_oauth);
    let body = form_urlencoded::Serializer::new(String::new())
        .extend_pairs(init_params)
        .finish();

    let response = client
        .post(UPLOAD_URL)
        .header("Authorization", auth)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await?;

    let (value, _) = ensure_success(response, "media INIT").await?;
    let parsed: MediaUploadResponse =
        serde_json::from_value(value).map_err(|e| AutoxError::Media {
            message: format!("invalid INIT response: {e}"),
        })?;
    let media_id = media_id_from(&parsed).ok_or_else(|| AutoxError::Media {
        message: "INIT returned no media_id".into(),
    })?;

    // APPEND — sequential (X requires ordered segment_index). Stream each chunk from disk.
    let mut file = File::open(path).await?;
    let mut segment: u32 = 0;
    let mut buf = vec![0u8; CHUNK_SIZE];

    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        let chunk = Bytes::copy_from_slice(&buf[..n]);
        let segment_s = segment.to_string();

        // Multipart body is not part of OAuth base string.
        let auth = build_oauth1_header(credentials, "POST", UPLOAD_URL, &[]);
        let stream = stream::once(async move { Ok::<Bytes, std::io::Error>(chunk) });
        let part = Part::stream(reqwest::Body::wrap_stream(stream))
            .file_name(format!("chunk{segment}"))
            .mime_str("application/octet-stream")
            .map_err(|e| AutoxError::Media {
                message: e.to_string(),
            })?;

        let form = Form::new()
            .text("command", "APPEND")
            .text("media_id", media_id.clone())
            .text("segment_index", segment_s)
            .part("media", part);

        let response = client
            .post(UPLOAD_URL)
            .header("Authorization", auth)
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            ensure_success(response, &format!("media APPEND segment {segment}")).await?;
        }

        segment += 1;
    }

    // FINALIZE
    let finalize_oauth = [("command", "FINALIZE"), ("media_id", media_id.as_str())];
    let auth = build_oauth1_header(credentials, "POST", UPLOAD_URL, &finalize_oauth);
    let body = form_urlencoded::Serializer::new(String::new())
        .append_pair("command", "FINALIZE")
        .append_pair("media_id", &media_id)
        .finish();

    let response = client
        .post(UPLOAD_URL)
        .header("Authorization", auth)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await?;

    let (value, _) = ensure_success(response, "media FINALIZE").await?;
    let mut parsed: MediaUploadResponse =
        serde_json::from_value(value).map_err(|e| AutoxError::Media {
            message: format!("invalid FINALIZE response: {e}"),
        })?;

    // STATUS poll when processing_info present (video)
    while let Some(info) = parsed.processing_info.as_ref() {
        match info.state.as_str() {
            "succeeded" => break,
            "failed" => {
                return Err(AutoxError::Media {
                    message: format!(
                        "media processing failed: {}",
                        info.error
                            .as_ref()
                            .and_then(|e| e.message.as_deref())
                            .unwrap_or("unknown")
                    ),
                });
            }
            _ => {
                let wait = info.check_after_secs.unwrap_or(2);
                tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                let status_oauth = [("command", "STATUS"), ("media_id", media_id.as_str())];
                let auth = build_oauth1_header(credentials, "GET", UPLOAD_URL, &status_oauth);
                let url = format!("{UPLOAD_URL}?command=STATUS&media_id={media_id}");
                let response = client
                    .get(&url)
                    .header("Authorization", auth)
                    .send()
                    .await?;
                let (value, _) = ensure_success(response, "media STATUS").await?;
                parsed = serde_json::from_value(value).map_err(|e| AutoxError::Media {
                    message: format!("invalid STATUS response: {e}"),
                })?;
            }
        }
    }

    Ok(media_id)
}
