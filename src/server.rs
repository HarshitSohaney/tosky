use tiny_http::{Server, Response};
use std::sync::Arc;
use crate::db::Database;
use std::thread;
use urlencoding::decode;

// Your ngrok hostname - update this each time you restart ngrok
const HOSTNAME: &str = "unobscenely-keyed-tatiana.ngrok-free.dev";

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

/// DID document for did:web resolution
fn did_json() -> String {
    let json = r##"{"@context":["https://www.w3.org/ns/did/v1"],"id":"did:web:HOSTNAME","service":[{"id":"#bsky_fg","type":"BskyFeedGenerator","serviceEndpoint":"https://HOSTNAME"}]}"##;
    json.replace("HOSTNAME", HOSTNAME)
}

/// Describe feed generator endpoint
fn describe_feed_generator() -> String {
    let json = r#"{"did":"did:web:HOSTNAME","feeds":[{"uri":"at://did:web:HOSTNAME/app.bsky.feed.generator/toronto"}]}"#;
    json.replace("HOSTNAME", HOSTNAME)
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

                        if url == "/.well-known/did.json" {
                            let response = Response::from_string(did_json())
                                .with_header("Content-Type: application/json".parse::<tiny_http::Header>().unwrap());
                            rq.respond(response).ok();
                        } else if url.starts_with("/xrpc/app.bsky.feed.describeFeedGenerator") {
                            let response = Response::from_string(describe_feed_generator())
                                .with_header("Content-Type: application/json".parse::<tiny_http::Header>().unwrap());
                            rq.respond(response).ok();
                        } else if url.starts_with("/xrpc/app.bsky.feed.getFeedSkeleton") {
                            let params = parse_query_params(url);

                            // Parse limit (default 50, max 100)
                            let limit: i64 = params.get("limit")
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(50)
                                .min(100);

                            // Parse cursor (format: "timestamp:seed" or none)
                            let (cursor, seed): (Option<i64>, u32) = match params.get("cursor") {
                                Some(c) => {
                                    let decoded = decode(c).unwrap_or(std::borrow::Cow::Borrowed(c));
                                    println!("[Server] Raw cursor: {}, decoded: {}", c, decoded);
                                    let parts: Vec<&str> = decoded.split(':').collect();
                                    let ts = parts.get(0).and_then(|x| x.parse().ok());
                                    let s = parts.get(1).and_then(|x| x.parse().ok()).unwrap_or_else(|| rand::random::<u32>());

                                    (ts, s)
                                },
                                None => (None, rand::random::<u32>())
                            };

                            println!("[Server] getFeedSkeleton request - limit:{} cursor:{:?}", limit, cursor);

                            let (posts, next_cursor) = db.read_posts(limit, cursor, seed);

                            println!("[Server] Returning {} posts, next_cursor:{:?}", posts.len(), next_cursor);
                            for (i, uri) in posts.iter().enumerate() {
                                println!("[Server]   {}. {}", i + 1, uri);
                            }

                            // Build feed array
                            let feed: Vec<String> = posts
                                .iter()
                                .map(|uri| format!(r#"{{"post":"{}"}}"#, uri))
                                .collect();

                            // Build response with optional cursor
                            let json = match next_cursor {
                                Some(c) => {
                                    let cursor_str = format!("{}:{}", c, seed);
                                    println!("[Server] Returning cursor: {}", cursor_str);
                                    format!(r#"{{"feed":[{}],"cursor":"{}"}}"#, feed.join(","), cursor_str)
                                },
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