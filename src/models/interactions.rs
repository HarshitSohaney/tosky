// src/models/interactions.rs

use serde::Deserialize;
use crate::models::StrongRef;

#[derive(PartialEq)]
pub enum InteractionType {
    LIKE,
    REPOST
}

#[derive(Debug, Deserialize)]
pub struct Like {
    pub subject: StrongRef,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    pub via: Option<StrongRef>
}

#[derive(Debug, Deserialize)]
pub struct Repost {
    pub subject: StrongRef,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    pub via: Option<StrongRef>
}
