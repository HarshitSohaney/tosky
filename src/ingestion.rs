use crate::parser::{parse_message};
use tungstenite::{connect, Message};

pub fn start_ingestion() -> Result<(), Box<dyn std::error::Error>> {
    // Open a websocket to the url
    let (mut socket, response) = connect("wss://bsky.network/xrpc/com.atproto.sync.subscribeRepos")?;

    println!("Connected to the server");
    println!("Response HTTP code: {}", response.status());
    println!("Response contains the following headers:");

    // when we receive a message from the url, call a provided callback
    // that passes the posts to our filter
    loop {
        let msg = socket.read()?;

        match msg {
            Message::Binary(data) => {
                let msg = parse_message(&data);
                println!("Received {} bytes, msg is {:?}", data.len(), msg);
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