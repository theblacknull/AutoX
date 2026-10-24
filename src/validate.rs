use std::path::Path;

use unicode_segmentation::UnicodeSegmentation;

use crate::error::AutoxError;
use crate::types::{
    CreatePostRequest, MEDIA_IMAGE_MAX_BYTES, MEDIA_VIDEO_MAX_BYTES, X_MAX_IMAGES,
    X_TEXT_BYTE_CAP, X_WEIGHTED_CHAR_LIMIT,
};

/// X weighted length: most BMP chars = 1; many CJK / fullwidth = 2.
/// Approximation of Twitter's official counter (URLs counted as 23 when present as http(s)).
pub fn weighted_length(text: &str) -> usize {
    // URL t.co weighting: replace http(s) URLs with 23-weight placeholders for counting.
    let mut count = 0usize;
    let mut rest = text;
    while let Some(idx) = find_url_start(rest) {
        let before = &rest[..idx];
        count += grapheme_weight(before);
        let after_scheme = &rest[idx..];
        let url_end = after_scheme
            .char_indices()
            .find(|(_, c)| c.is_whitespace())
            .map(|(i, _)| i)
            .unwrap_or(after_scheme.len());
        count += 23; // t.co length
        rest = &after_scheme[url_end..];
    }
    count + grapheme_weight(rest)
}

fn find_url_start(s: &str) -> Option<usize> {
    s.find("https://").or_else(|| s.find("http://"))
}

fn grapheme_weight(s: &str) -> usize {
    s.graphemes(true).map(char_weight).sum()
}

fn char_weight(g: &str) -> usize {
    let mut chars = g.chars();
    let Some(c) = chars.next() else {
        return 0;
    };
    // If it's a multi-codepoint grapheme (emoji ZWJ etc.), count as 2 (safe for classic limit).
    if chars.next().is_some() {
        return 2;
    }
    let cp = c as u32;
    // CJK Unified Ideographs + common fullwidth / Hangul / Hiragana / Katakana ranges → 2
    if (0x1100..=0x11FF).contains(&cp)
        || (0x2E80..=0x9FFF).contains(&cp)
        || (0xA960..=0xA97F).contains(&cp)
        || (0xAC00..=0xD7AF).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0xFE10..=0xFE1F).contains(&cp)
        || (0xFE30..=0xFE4F).contains(&cp)
        || (0xFF00..=0xFFEF).contains(&cp)
        || (0x1F300..=0x1FAFF).contains(&cp)
    // emoji blocks
    {
        2
    } else {
        1
    }
}

/// Validate text + media paths before any X network call.
pub fn validate_request(req: &CreatePostRequest) -> Result<usize, AutoxError> {
    if !req.text.is_empty() && std::str::from_utf8(req.text.as_bytes()).is_err() {
        // String in Rust is always UTF-8; this is defensive documentation.
        return Err(AutoxError::invalid("text must be valid UTF-8"));
    }

    let trimmed = req.text.trim();
    if trimmed.is_empty() {
        return Err(AutoxError::invalid("post text is required (empty after trim)"));
    }

    if req.text.len() > X_TEXT_BYTE_CAP {
        return Err(AutoxError::invalid(format!(
            "text exceeds absolute byte cap of {X_TEXT_BYTE_CAP}"
        )));
    }

    let wlen = weighted_length(trimmed);
    if wlen > X_WEIGHTED_CHAR_LIMIT {
        return Err(AutoxError::invalid(format!(
            "text weighted length {wlen} exceeds X classic limit {X_WEIGHTED_CHAR_LIMIT}"
        )));
    }

    if req.media_paths.len() > X_MAX_IMAGES {
        return Err(AutoxError::invalid(format!(
            "at most {X_MAX_IMAGES} media attachments allowed"
        )));
    }

    let mut has_video_or_gif = false;
    let mut image_count = 0usize;

    for path in &req.media_paths {
        let p = Path::new(path);
        if !p.is_file() {
            return Err(AutoxError::MediaNotFound {
                path: path.clone(),
            });
        }
        let meta = std::fs::metadata(p)?;
        let size = meta.len();
        let mime = mime_guess::from_path(p)
            .first_or_octet_stream()
            .essence_str()
            .to_string();

        if mime.starts_with("video/") || mime == "image/gif" {
            if has_video_or_gif || image_count > 0 || req.media_paths.len() > 1 {
                return Err(AutoxError::invalid(
                    "video/GIF must be the only media attachment",
                ));
            }
            has_video_or_gif = true;
            if size > MEDIA_VIDEO_MAX_BYTES {
                return Err(AutoxError::invalid(format!(
                    "video/GIF exceeds max size {MEDIA_VIDEO_MAX_BYTES} bytes"
                )));
            }
        } else if mime.starts_with("image/") {
            image_count += 1;
            if has_video_or_gif {
                return Err(AutoxError::invalid(
                    "cannot mix images with video/GIF",
                ));
            }
            if size > MEDIA_IMAGE_MAX_BYTES {
                return Err(AutoxError::invalid(format!(
                    "image exceeds max size {MEDIA_IMAGE_MAX_BYTES} bytes: {path}"
                )));
            }
        } else {
            return Err(AutoxError::invalid(format!(
                "unsupported media type {mime} for {path}"
            )));
        }
    }

    Ok(wlen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_weight() {
        assert_eq!(weighted_length("hello"), 5);
    }

    #[test]
    fn cjk_weight() {
        assert_eq!(weighted_length("你好"), 4);
    }

    #[test]
    fn url_counts_as_23() {
        assert_eq!(weighted_length("see https://example.com/foo"), 4 + 23);
    }
}
