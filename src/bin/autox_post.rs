//! Thin CLI around the AutoX X API v2 library.
//! Dry-run by default; `--publish` required to hit X.

use anyhow::Context;
use clap::Parser;
use std::process::ExitCode;

use autox::{create_post, CreatePostRequest, PostMode, XClient};

#[derive(Debug, Parser)]
#[command(
    name = "autox-post",
    about = "Post to X via API v2 (dry-run by default; --publish to go live)"
)]
struct Args {
    /// Post text (required unless --verify)
    #[arg(short, long)]
    text: Option<String>,

    /// Local media path (repeatable, max 4 images or 1 video/GIF)
    #[arg(short, long = "media")]
    media: Vec<String>,

    /// Actually call X API (requires env credentials)
    #[arg(long, default_value_t = false)]
    publish: bool,

    /// Draft directory for dry-run output
    #[arg(short, long, default_value = "drafts")]
    out: String,

    /// Reply to post id
    #[arg(long = "reply-to")]
    reply_to: Option<String>,

    /// Quote post id
    #[arg(long)]
    quote: Option<String>,

    /// Print full JSON result
    #[arg(long, default_value_t = false)]
    json: bool,

    /// Verify credentials via GET /2/users/me (no post)
    #[arg(long, default_value_t = false)]
    verify: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    if let Err(err) = run().await {
        eprintln!("Error: {err:#}");
        // Try to surface machine-readable code when it's AutoxError
        if let Some(ae) = err.downcast_ref::<autox::AutoxError>() {
            eprintln!("code={}", ae.code());
        }
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

async fn run() -> anyhow::Result<()> {
    let args = Args::parse();
    let client = XClient::shared().context("build HTTP client")?;

    if args.verify {
        let creds = autox::load_credentials_from_env()?;
        let me = client.verify_me(&creds).await?;
        println!(
            "ok id={} username=@{} name={}",
            me.id, me.username, me.name
        );
        return Ok(());
    }

    let text = args
        .text
        .ok_or_else(|| anyhow::anyhow!("--text is required unless --verify"))?;

    if args.publish {
        eprintln!("WARNING: --publish set; this will call the X API and may create a real post.");
    }

    let req = CreatePostRequest {
        text,
        media_paths: args.media,
        publish: args.publish,
        draft_dir: args.out,
        reply_to_post_id: args.reply_to,
        quote_post_id: args.quote,
    };

    let result = create_post(&client, req).await?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    match result.mode {
        PostMode::DryRun => {
            println!(
                "Dry-run OK — draft written to {}",
                result.draft_path.as_deref().unwrap_or("(none)")
            );
            if let Some(p) = &result.payload {
                println!("Text (weighted {}): {}", p.weighted_length, p.text);
                if let Some(media) = &p.media {
                    let paths: Vec<_> = media.iter().map(|m| m.path.as_str()).collect();
                    println!("Media (not uploaded): {}", paths.join(", "));
                }
            }
            println!("Re-run with --publish to post to X.");
        }
        PostMode::Published => {
            let id = result
                .post_id
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("published mode missing post_id"))?;
            println!("Published post id: {id}");
            if let Some(url) = &result.url {
                println!("URL: {url}");
            }
        }
    }

    Ok(())
}
