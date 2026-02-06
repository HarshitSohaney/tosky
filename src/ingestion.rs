use crate::parser::{parse_car_blocks, parse_message};
use tungstenite::{connect, Message};
use crate::models::{Post, Action};
use crate::filter::Filter;

pub fn start_ingestion(filter: &mut Filter) -> Result<(), Box<dyn std::error::Error>> {
    // Open a websocket to the url
    let (mut socket, response) = connect("wss://bsky.network/xrpc/com.atproto.sync.subscribeRepos")?;

    println!("Connected to the server");

    // when we receive a message from the url, call a provided callback
    // that passes the posts to our filter
    loop {
        let msg = socket.read()?;

        match msg {
            Message::Binary(data) => {
                if let Ok(Some(frame)) = parse_message(&data) {
                    for op in &frame.ops {
                        if !matches!(op.action, Action::Create)
                            || !op.path.starts_with("app.bsky.feed.post/") {
                            continue;
                        }

                        // Parse blocks
                        let blocks = parse_car_blocks(&frame.blocks);
                        if let Some(target_cid) = &op.cid {
                            for (block_cid, block_data) in &blocks {
                                if block_cid == &target_cid[1..] {
                                    // Found the right block!
                                    match serde_cbor::from_slice::<Post>(&block_data) {
                                        Ok(post) => filter.callback(&frame, &op, &post),
                                        Err(e) => println!("Failed to parse post: {}", e),
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Message::Close(_) => {
                // Server closed connection
                return Ok(());
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
}