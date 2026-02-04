// src/models/frame.rs

use serde::Deserialize;
use super::Operation;

#[derive(Debug, Deserialize)]
pub struct Frame {
    pub repo: String,
    pub ops: Vec<Operation>,
    #[serde(with = "serde_bytes")]
    pub blocks: Vec<u8>,
}