use serde::Deserialize;
use crate::models::Frame;

// Header tells us what type of message this is
#[derive(Debug, Deserialize)]
struct Header {
    op: i32,      // operation type (1 = message)
    t: String,    // message type ("#commit", "#identity", etc.)
}

pub fn parse_message(data: &[u8]) -> Result<Option<Frame>, Box<dyn std::error::Error>> {
    let mut iter = serde_cbor::Deserializer::from_slice(data).into_iter::<serde_cbor::Value>();

    let header_val = iter.next().ok_or("No header")??;
    let header: Header = serde_cbor::value::from_value(header_val)?;

    if header.t != "#commit" {
        return Ok(None);  // Not a commit, skip
    }

    let frame_val = iter.next().ok_or("Frame value missing")??;
    let frame = serde_cbor::value::from_value(frame_val)?;

    Ok(Some(frame))
}