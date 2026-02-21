use crate::db::Database;
use crate::filter::{LAX_KEYWORDS, STRICT_KEYWORDS, NSFW_LABELS};
use serde_json::Value;
use std::process::Command;
use std::thread;
use std::time::Duration;

struct TimeWindow {
    label: &'static str,
    hours_ago_start: i64,
    hours_ago_end: i64,
}

const WINDOWS: &[TimeWindow] = &[
    TimeWindow { label: "0-1h",   hours_ago_start: 0,  hours_ago_end: 1 },
    TimeWindow { label: "1-4h",   hours_ago_start: 1,  hours_ago_end: 4 },
    TimeWindow { label: "4-12h",  hours_ago_start: 4,  hours_ago_end: 12 },
    TimeWindow { label: "12-24h", hours_ago_start: 12, hours_ago_end: 24 },
    TimeWindow { label: "24-48h", hours_ago_start: 24, hours_ago_end: 48 },
];

const MAX_RETRIES: u32 = 5;
const BASE_SLEEP_MS: u64 = 1000;

// Note: lowercase "searchposts" is intentional — the CDN blocks the cursor
// parameter on the canonical "searchPosts" endpoint (known issue:
// https://github.com/bluesky-social/atproto/issues/3583)
const SEARCH_URL: &str = "https://api.bsky.app/xrpc/app.bsky.feed.searchposts";

pub fn run_backfill(db: &mut Database) {
    println!("[Backfill] Starting reverse hydration via search API");

    let now = chrono::Utc::now();
    let mut total_inserted = 0;
    let mut total_queries = 0;

    for window in WINDOWS {
        let window_end = now - chrono::Duration::hours(window.hours_ago_start);
        let window_start = now - chrono::Duration::hours(window.hours_ago_end);

        println!("[Backfill] Window {} ({} to {})",
            window.label,
            window_start.format("%H:%M"),
            window_end.format("%H:%M"),
        );

        let mut window_inserted = 0;

        // Lax keywords: subdivide into 1-hour chunks (high volume)
        for keyword in LAX_KEYWORDS {
            let window_hours = window.hours_ago_end - window.hours_ago_start;
            for chunk in 0..window_hours {
                let chunk_start = window_start + chrono::Duration::hours(chunk);
                let chunk_end = window_start + chrono::Duration::hours(chunk + 1);

                let since = chunk_start.format("%Y-%m-%dT%H:%M:%SZ").to_string();
                let until = chunk_end.format("%Y-%m-%dT%H:%M:%SZ").to_string();

                let (inserted, queries) = search_all_pages(db, keyword, &since, &until);
                window_inserted += inserted;
                total_queries += queries;

                thread::sleep(Duration::from_millis(BASE_SLEEP_MS));
            }
        }

        // Strict keywords: use full window (low volume)
        for keyword in STRICT_KEYWORDS {
            let since = window_start.format("%Y-%m-%dT%H:%M:%SZ").to_string();
            let until = window_end.format("%Y-%m-%dT%H:%M:%SZ").to_string();

            let (inserted, queries) = search_all_pages(db, keyword, &since, &until);
            window_inserted += inserted;
            total_queries += queries;

            thread::sleep(Duration::from_millis(BASE_SLEEP_MS));
        }

        total_inserted += window_inserted;
        println!("[Backfill] Window {} complete: {} posts inserted", window.label, window_inserted);
    }

    println!("[Backfill] Done. {} total queries, {} total posts inserted", total_queries, total_inserted);
}

fn search_all_pages(db: &mut Database, keyword: &str, since: &str, until: &str) -> (i64, usize) {
    let mut total_inserted = 0i64;
    let mut total_results = 0usize;
    let mut cursor: Option<String> = None;
    let mut queries = 0usize;

    loop {
        let json = match fetch_page_with_retry(keyword, since, until, cursor.as_deref()) {
            Some(j) => j,
            None => break,
        };
        queries += 1;

        let posts = match json["posts"].as_array() {
            Some(p) => p,
            None => break,
        };

        let page_count = posts.len();
        total_results += page_count;
        total_inserted += insert_posts(db, posts);

        cursor = json["cursor"].as_str()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        if cursor.is_none() || page_count < 100 {
            break;
        }

        println!("[Backfill] Paginating '{}' ({} results so far)", keyword, total_results);
        thread::sleep(Duration::from_millis(BASE_SLEEP_MS));
    }

    (total_inserted, queries)
}

fn fetch_page_with_retry(keyword: &str, since: &str, until: &str, cursor: Option<&str>) -> Option<Value> {
    let mut url = format!(
        "{}?q={}&limit=100&sort=top&since={}&until={}",
        SEARCH_URL,
        urlencoding::encode(keyword),
        since,
        until,
    );
    if let Some(c) = cursor {
        url.push_str("&cursor=");
        url.push_str(&urlencoding::encode(c));
    }

    for attempt in 0..MAX_RETRIES {
        let output = match Command::new("curl")
            .arg("-s")
            .arg("-w")
            .arg("\n%{http_code}")
            .arg(&url)
            .output()
        {
            Ok(o) => o,
            Err(e) => {
                eprintln!("[Backfill] curl failed: {}", e);
                return None;
            }
        };

        let raw = match String::from_utf8(output.stdout) {
            Ok(b) => b,
            Err(_) => return None,
        };

        let (body, status_line) = match raw.rfind('\n') {
            Some(pos) => (&raw[..pos], raw[pos+1..].trim()),
            None => (raw.as_str(), ""),
        };

        if status_line == "403" || status_line == "429" {
            let wait = BASE_SLEEP_MS * 2u64.pow(attempt + 1);
            println!("[Backfill] {} for '{}' (attempt {}), retrying in {}s...",
                status_line, keyword, attempt + 1, wait / 1000);
            thread::sleep(Duration::from_millis(wait));
            continue;
        }

        match serde_json::from_str(body) {
            Ok(j) => return Some(j),
            Err(e) => {
                eprintln!("[Backfill] JSON parse error: {} body: {}", e, &body[..200.min(body.len())]);
                return None;
            }
        }
    }

    eprintln!("[Backfill] Giving up on '{}' after {} retries", keyword, MAX_RETRIES);
    None
}

fn insert_posts(db: &mut Database, posts: &[Value]) -> i64 {
    let mut inserted = 0i64;

    for post in posts {
        if let Some(labels) = post["labels"].as_array() {
            let is_nsfw = labels.iter().any(|l| {
                let val = l["val"].as_str().unwrap_or("");
                NSFW_LABELS.contains(&val)
            });
            if is_nsfw {
                continue;
            }
        }

        let uri = match post["uri"].as_str() {
            Some(u) => u,
            None => continue,
        };
        let cid = match post["cid"].as_str() {
            Some(c) => c,
            None => continue,
        };
        let did = match post["author"]["did"].as_str() {
            Some(d) => d,
            None => continue,
        };

        let created_at = post["record"]["createdAt"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.timestamp())
            .unwrap_or(0);

        let likes = post["likeCount"].as_i64().unwrap_or(0);
        let reposts = post["repostCount"].as_i64().unwrap_or(0);
        let quotes = post["quoteCount"].as_i64().unwrap_or(0);
        let replies = post["replyCount"].as_i64().unwrap_or(0);

        if let Err(e) = db.insert_post_if_not_exists(uri, cid, did, created_at, likes, reposts, quotes, replies) {
            eprintln!("[Backfill] Insert error: {}", e);
        } else {
            inserted += 1;
        }
    }

    inserted
}
