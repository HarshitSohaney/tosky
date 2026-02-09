# Toronto Feed (to-sky)

A custom Bluesky feed generator that surfaces posts about Toronto.

## What it does

- Connects to the Bluesky firehose (WebSocket)
- Filters posts containing Toronto-related keywords, hashtags, and URLs
- Stores matching posts in SQLite
- Serves a feed via HTTP that Bluesky can consume

## Filter criteria

Posts match if they contain:
- **Keywords**: toronto, ttc, cn tower, 6ix (whole word matching for short terms)
- **Hashtags**: #toronto
- **URLs**: Links containing "toronto"
- **Quotes**: Quote posts of other Toronto posts

## Running locally

```bash
cargo run
```

This starts:
- Firehose ingestion (writes to `db/posts.db`)
- HTTP server on port 3000

## Testing the feed

```bash
curl "http://localhost:3000/xrpc/app.bsky.feed.getFeedSkeleton?limit=10"
```

## Exposing via ngrok

```bash
ngrok http 3000
```

Copy the ngrok URL (e.g., `https://abc123.ngrok-free.app`)

Update `HOSTNAME` in `src/server.rs`:
```rust
const HOSTNAME: &str = "abc123.ngrok-free.app";
```

Rebuild and run:
```bash
cargo run
```

## Registering the feed (first time)

```bash
# 1. Login
curl -s -X POST "https://bsky.social/xrpc/com.atproto.server.createSession" \
  -H "Content-Type: application/json" \
  -d '{"identifier":"<YOUR_HANDLE>","password":"<YOUR_APP_PASSWORD>"}' > /tmp/session.json

# 2. Extract credentials
TOKEN=$(cat /tmp/session.json | grep -o '"accessJwt":"[^"]*"' | cut -d'"' -f4)
DID=$(cat /tmp/session.json | grep -o '"did":"[^"]*"' | cut -d'"' -f4)

# 3. Create feed record
curl -X POST "https://bsky.social/xrpc/com.atproto.repo.putRecord" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"repo\": \"$DID\",
    \"collection\": \"app.bsky.feed.generator\",
    \"rkey\": \"toronto\",
    \"record\": {
      \"\$type\": \"app.bsky.feed.generator\",
      \"did\": \"did:web:<YOUR_NGROK_HOSTNAME>\",
      \"displayName\": \"Toronto Feed\",
      \"description\": \"Posts about Toronto - keywords, hashtags, and links\",
      \"createdAt\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"
    }
  }"
```

## Updating the feed URL (new ngrok session)

When you restart ngrok, you get a new URL. Update the feed record:

```bash
# 1. Login
TOKEN=$(curl -s -X POST "https://bsky.social/xrpc/com.atproto.server.createSession" \
  -H "Content-Type: application/json" \
  -d '{"identifier":"<YOUR_HANDLE>","password":"<YOUR_APP_PASSWORD>"}' | grep -o '"accessJwt":"[^"]*"' | cut -d'"' -f4)

# 2. Update feed record with new URL
curl -X POST "https://bsky.social/xrpc/com.atproto.repo.putRecord" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "repo": "harsheet.bsky.social",
    "collection": "app.bsky.feed.generator",
    "rkey": "toronto",
    "record": {
      "$type": "app.bsky.feed.generator",
      "did": "did:web:unobscenely-keyed-tatiana.ngrok-free.dev",
      "displayName": "Toronto Feed",
      "description": "Posts about Toronto - keywords, hashtags, and links",
      "createdAt": "2026-02-07T00:00:00Z"
    }
  }'
```

Don't forget to also update `HOSTNAME` in `src/server.rs`!

## Deleting the feed

```bash
TOKEN=$(curl -s -X POST "https://bsky.social/xrpc/com.atproto.server.createSession" \
  -H "Content-Type: application/json" \
  -d '{"identifier":"<YOUR_HANDLE>","password":"<YOUR_APP_PASSWORD>"}' | grep -o '"accessJwt":"[^"]*"' | cut -d'"' -f4)

curl -X POST "https://bsky.social/xrpc/com.atproto.repo.deleteRecord" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "repo": "<YOUR_HANDLE>",
    "collection": "app.bsky.feed.generator",
    "rkey": "toronto"
  }'
```

## Placeholders

Replace these in the commands above:
- `<YOUR_HANDLE>` - Your Bluesky handle (e.g., `yourname.bsky.social`)
- `<YOUR_APP_PASSWORD>` - App password from Bluesky Settings → App Passwords
- `<YOUR_NGROK_HOSTNAME>` - Just the hostname (e.g., `abc123.ngrok-free.app`, no `https://`)

## Project structure

```
src/
├── main.rs       - Entry point, spawns ingestion + server threads
├── ingestion.rs  - WebSocket firehose connection
├── parser.rs     - CBOR/CAR parsing
├── filter.rs     - Toronto keyword matching
├── db.rs         - SQLite operations
├── server.rs     - HTTP server (getFeedSkeleton)
└── models/       - Data structures (Post, Frame, etc.)
```

## View your feed

Once registered: `https://bsky.app/profile/<YOUR_HANDLE>/feed/toronto`
