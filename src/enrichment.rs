use crate::db::Database;
use std::process::Command;
use serde_json::Value;

pub struct EnrichThread {
    db: Database,
}

impl EnrichThread {
    pub fn new(path: &str) -> Self {
        EnrichThread { db: Database::new(path) }
    }

    pub fn enrich_what_we_missed(&self) -> Result<(), Box<dyn std::error::Error>> {
        let uris = self.db.get_posts_to_enrich(25);
        println!("[Enrichment] Fetching engagement for {} posts", uris.len());

        if uris.is_empty() {
            println!("[Enrichment] No posts to enrich");
            return Ok(());
        }

        let url = format!("https://public.api.bsky.app/xrpc/app.bsky.feed.getPosts?uris={}",
            uris.join("&uris=")
        );

        let output = Command::new("curl")
            .arg("-s")
            .arg(url)
            .output()
            .expect("Failed to execute curl");

        let body = String::from_utf8(output.stdout).unwrap();

        let json: Value = serde_json::from_str(&body)?;

        if let Some(posts) = json["posts"].as_array() {
            println!("[Enrichment] Got {} posts from API", posts.len());

            for post in posts {
                let uri = post["uri"].as_str().unwrap_or("");
                let likes = post["likeCount"].as_i64().unwrap_or(0);
                let reposts = post["repostCount"].as_i64().unwrap_or(0);
                let quotes = post["quoteCount"].as_i64().unwrap_or(0);
                let replies = post["replyCount"].as_i64().unwrap_or(0);
                let bookmarks = post["bookmarkCount"].as_i64().unwrap_or(0);

                println!("[Enrichment] {} - L:{} R:{} Q:{}", uri, likes, reposts, quotes);

                self.db.update_engagement(uri, likes, reposts, quotes, replies, bookmarks);
            }
        } else {
            println!("[Enrichment] No posts in response. Body: {}", &body[..200.min(body.len())]);
        }

        Ok(())
    }
}

