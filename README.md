# AutoX

Fast, dry-run-safe **X (Twitter) API v2** posting for **@TheBlackNull** / BlackNull.

Cursor’s official X MCP plugin cannot create posts. This crate talks to X **directly** — the most compatible and lowest-latency write path (no gateway hop).

## What we shipped

| Surface | Role |
| --- | --- |
| **Rust library** (`autox`) | Core: OAuth 1.0a user-context → `POST /2/tweets` + v1.1 media upload |
| **CLI** (`autox-post`) | Thin wrapper for automation |
| HTTP gateway | **Skipped** — adds a hop; library/CLI call X directly |

**Why this is fastest + most X-compatible:** one process, one pooled `reqwest` (rustls) client, release LTO binary, validate before any network, then the same endpoints X documents for posting.

## Safety

- Importing the library never posts and never reads secrets.
- **Dry-run is default** — writes `drafts/draft-*.json`.
- Real posts require explicit `--publish`.
- Credentials loaded from env **only on the publish path**.
- `Published` is returned **only** when X responds with `data.id`.
- Rate limits → clear `RATE_LIMITED` errors.

## Text limits (enforced before network)

We enforce the **classic post** weighted length of **≤ 280** (CJK/fullwidth/emoji graphemes count as 2; `http(s)://…` URLs count as 23). Long-form / Premium higher limits are **not** enabled here — keep posts within 280 weighted units. Absolute UTF-8 byte cap: 8 KiB.

## Setup

```bash
cp .env.example .env   # fill locally — never commit
cargo build --release  # supported binary (LTO, codegen-units=1, opt-level=3)
```

### Env (publish only)

| Variable | Meaning |
| --- | --- |
| `X_API_KEY` | App API Key (consumer key) |
| `X_API_SECRET` | App API Secret |
| `X_ACCESS_TOKEN` | User access token (**Read and Write**) |
| `X_ACCESS_TOKEN_SECRET` | User access token secret |

Aliases `TWITTER_*` accepted. Create an app at [console.x.com](https://console.x.com) with OAuth 1.0a **Read and Write**.

## CLI

```bash
# Dry-run (default) — no X network
./target/release/autox-post --text "Hello from AutoX dry-run"

# Record media paths in the draft (upload only with --publish)
./target/release/autox-post --text "Brag clip" --media ./clips/demo.mp4

# Publish (requires env credentials) — do this only when you intend to post
./target/release/autox-post --text "Hello" --publish

# Verify credentials without posting
./target/release/autox-post --verify
```

Dev (slower): `cargo run --release -- --text "…"`.

## Library

```rust
use autox::{create_post, CreatePostRequest, XClient};

#[tokio::main]
async fn main() -> Result<(), autox::AutoxError> {
    let client = XClient::shared()?; // one pooled client
    let result = create_post(&client, CreatePostRequest {
        text: "Feature update".into(),
        publish: false, // dry-run
        ..Default::default()
    }).await?;
    println!("{result:?}");
    Ok(())
}
```

## Latency notes

| Path | What dominates |
| --- | --- |
| **Dry-run** (release) | Local validate + write JSON — typically low single-digit ms on a warm disk |
| **Publish text-only** | Network RTT to `api.x.com` (TLS + OAuth + JSON) — usually tens–hundreds of ms |
| **Publish + media** | Upload bandwidth + (for video) X processing poll — seconds |

Measure dry-run:

```bash
cargo build --release
time ./target/release/autox-post --text "bench dry-run" --json >/dev/null
```

## Layout

```
src/
  lib.rs          Public API (no side effects on import)
  post.rs         create_post — dry-run vs publish
  x_client.rs     POST /2/tweets, GET /2/users/me
  media.rs        v1.1 simple + chunked (streamed APPEND) upload
  oauth1.rs       OAuth 1.0a signing
  validate.rs     Weighted length + media checks
  bin/autox_post.rs
drafts/           Dry-run output (gitignored)
examples/         Sample dry-run payload
```

## Out of scope

- News / brag schedulers
- Buffer / Typefully adapters
- Cursor X MCP for writes
- HTTP API gateway (use the library directly)
