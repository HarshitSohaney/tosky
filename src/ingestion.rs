use crate::parser::{parse_car_blocks, parse_message};
use tungstenite::{connect, Message};
use crate::models::{Post, Action, Like, Repost, InteractionType};
use crate::db::Metadata;
use crate::filter::Filter;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::thread;

const MAX_CURSOR_AGE_SECS: i64 = 259200; // 3 days

pub fn start_ingestion(filter: &mut Filter) {
    loop {
         // Let's see if we have a cursor we need to use!
        let uri = match filter.db.get_metadata() {
            Some(meta) => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;
                let age = now - meta.last_updated;

                if meta.last_updated > 0 && age > MAX_CURSOR_AGE_SECS {
                    println!("[Ingestion] Cursor is {}s old (>{} max), discarding stale cursor and starting fresh",
                        age, MAX_CURSOR_AGE_SECS);
                    String::from("wss://bsky.network/xrpc/com.atproto.sync.subscribeRepos")
                } else {
                    println!("[Ingestion] Resuming from cursor: {}", meta.seq);
                    format!("wss://bsky.network/xrpc/com.atproto.sync.subscribeRepos?cursor={}", meta.seq)
                }
            },
            None => {
                println!("[Ingestion] No cursor found, starting fresh");
                String::from("wss://bsky.network/xrpc/com.atproto.sync.subscribeRepos")
            },
        };

        match connect(&uri) {
            Ok((mut socket, _)) => {
                println!("[Ingestion] Connected to the firehose");

                let mut count = 0;

                // when we receive a message from the url, call a provided callback
                // that passes the posts to our filter
                loop {
                    let msg = socket.read().unwrap();

                    match msg {
                        Message::Binary(data) => {
                            if let Ok(Some(frame)) = parse_message(&data) {
                                if count > 500 {
                                    count = 0;
                                    filter.db.set_metadata(&Metadata { seq: frame.seq, last_updated: 0 });
                                } else {
                                    count += 1;
                                }

                                for op in &frame.ops {
                                    if !matches!(op.action, Action::Create) {
                                        continue;
                                    }
                                    let blocks = parse_car_blocks(&frame.blocks);

                                    if let Some(target_cid) = &op.cid {
                                        for (block_cid, block_data) in &blocks {
                                            if block_cid != &target_cid[1..] {
                                                continue;
                                            }

                                            if op.path.starts_with("app.bsky.feed.post/") {
                                                match serde_cbor::from_slice::<Post>(&block_data) {
                                                    Ok(post) => filter.callback(&frame, &op, &post),
                                                    Err(e) => println!("Failed to parse post: {}", e),
                                                }
                                            } else if op.path.starts_with("app.bsky.feed.like/") {
                                                match serde_cbor::from_slice::<Like>(&block_data) {
                                                    Ok(like) => filter.on_interaction(&like.subject, InteractionType::LIKE),
                                                    Err(e) => println!("Failed to parse like: {}", e),
                                                }
                                            } else if op.path.starts_with("app.bsky.feed.repost/") {
                                                match serde_cbor::from_slice::<Repost>(&block_data) {
                                                    Ok(repost) => filter.on_interaction(&repost.subject, InteractionType::REPOST),
                                                    Err(e) => println!("Failed to parse repost: {}", e),
                                                }
                                            }
                                        
                                        }
                                    }
                                }
                            }
                        }
                        Message::Close(_) => {
                            // Server closed connection
                            break;
                        }
                        Message::Ping(_) | Message::Pong(_) => {
                            // Heartbeat - ignore
                        }
                        Message::Text(_) => {
                            // Shouldn't happen for firehose
                        }
                        _ => {}
                    }
                }
            },
            Err(e) => {
                eprintln!("[Ingestion] Connection failed: {}", e);
            }
        }        

        println!("[Ingestion] Disconnected, reconnecting in 5s...");
        thread::sleep(Duration::from_secs(5));
    }
}