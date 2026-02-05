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

fn split_cid_and_data(block: &[u8]) -> (Vec<u8>, Vec<u8>) {
    // CID structure:
    // [version varint][codec varint][hash_type varint][hash_len varint][hash_bytes]
    let mut pos = 0;

    let (_, size) = read_varint(&block[pos..]);
    pos += size;

    let (_, size) = read_varint(&block[pos..]);
    pos += size;

    // 3. Read hash type (varint, 0x12 for sha256)
    let (_, size) = read_varint(&block[pos..]);
    pos += size;

    // 4. Read hash length (varint, 0x20 = 32)
    let (hash_len, size) = read_varint(&block[pos..]);
    pos += size;

    // 5. Skip hash bytes
    pos += hash_len as usize;

    // Now pos points to where DATA begins
    let cid = block[..pos].to_vec();
    let data = block[pos..].to_vec();

    (cid, data)
}

pub fn parse_car_blocks(car_data: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {  // Vec<(CID, Data)>
    let mut blocks = Vec::new();
    let mut pos = 0;

    // 1. Skip header
    let (header_len, varint_size) = read_varint(&car_data[pos..]);
    pos += varint_size + header_len as usize;

    // 2. Read blocks until end
    while pos < car_data.len() {
        // Read block length
        let (block_len, varint_size) = read_varint(&car_data[pos..]);
        pos += varint_size;

        // Block contains: [CID][DATA]
        // We need to parse CID to know where DATA starts
        let block_bytes = &car_data[pos..pos + block_len as usize];

        // Parse CID, get remaining as data
        let (cid, data) = split_cid_and_data(block_bytes);
        blocks.push((cid, data));

        pos += block_len as usize;
    }

    blocks
}

fn read_varint(data: &[u8]) -> (u64, usize) {
    // Each byte:
    // - Lower 7 bits: part of the number
    // - High bit (0x80): "more bytes follow" flag

    // Example: 300 = 0b100101100
    // Encoded as: [0xAC, 0x02]
    //   0xAC = 0b10101100 → lower 7 bits = 0101100, high bit set = more coming
    //   0x02 = 0b00000010 → lower 7 bits = 0000010, high bit clear = done
    //   Result: 0000010_0101100 = 300

    let mut result = 0u64;
    let mut shift = 0;

    // TODO: implement
    for (i, &byte) in data.iter().enumerate() {
        let data_bits = (0x7F & byte) as u64;
        result |= data_bits << shift;

        if (0x80 & byte) == 0 {
            return (result, i + 1);
        }

        shift += 7;
    };

    (result, data.len())
}
