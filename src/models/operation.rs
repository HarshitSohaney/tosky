use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Create,
    Update,
    Delete
}

#[derive(Debug, Deserialize)]
pub struct Operation {
    pub action: Action,
    pub path: String,
    // Sets a default if not present - None for Option
    #[serde(default)]
    #[serde(with = "serde_bytes")]
    pub cid: Option<Vec<u8>>,
    #[serde(default)]
    #[serde(with = "serde_bytes")]
    pub prev: Option<Vec<u8>>,
}
