use tiny_http::{Server, Response};
use std::sync::Arc;
use crate::db::Database;
use std::thread;

/// Parse query string into key-value pairs
fn parse_query_params(url: &str) -> std::collections::HashMap<String, String> {
    let mut params = std::collections::HashMap::new();

    if let Some(query_start) = url.find('?') {
        let query = &url[query_start + 1..];
        for pair in query.split('&') {
            if let Some(eq_pos) = pair.find('=') {
                let key = pair[..eq_pos].to_string();
                let value = pair[eq_pos + 1..].to_string();
                params.insert(key, value);
            }
        }
    }

    params
}

pub fn start_server() {
    let server = Arc::new(Server::http("0.0.0.0:3000").unwrap());
    println!("Server running on http://localhost:3000");

    let num_guards = 4;
    let mut guards = Vec::with_capacity(num_guards);

    for _ in 0..num_guards {
        let server = server.clone();

        let guard = thread::spawn(move || {
            let db = Database::new("../db/posts.db");

            loop {
                match server.recv() {
                    Ok(rq) => {
                        let url = rq.url();

                        if url.starts_with("/xrpc/app.bsky.feed.getFeedSkeleton") {
                            let params = parse_query_params(url);

                            // Parse limit (default 50, max 100)
                            let limit: i64 = params.get("limit")
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(50)
                                .min(100);

                            // Parse cursor (optional)
                            let cursor: Option<i64> = params.get("cursor")
                                .and_then(|s| s.parse().ok());

                            let (posts, next_cursor) = db.read_posts(limit, cursor);

                            // Build feed array
                            let feed: Vec<String> = posts
                                .iter()
                                .map(|uri| format!(r#"{{"post":"{}"}}"#, uri))
                                .collect();

                            // Build response with optional cursor
                            let json = match next_cursor {
                                Some(c) => format!(r#"{{"feed":[{}],"cursor":"{}"}}"#, feed.join(","), c),
                                None => format!(r#"{{"feed":[{}]}}"#, feed.join(",")),
                            };

                            let response = Response::from_string(json)
                                .with_header("Content-Type: application/json".parse::<tiny_http::Header>().unwrap());
                            rq.respond(response).ok();
                        } else {
                            let response = Response::from_string("Not Found").with_status_code(404);
                            rq.respond(response).ok();
                        }
                    },
                    Err(e) => {
                        eprintln!("Server error: {}", e);
                        break
                    }
                }
            }
        });

        guards.push(guard);
    }
}