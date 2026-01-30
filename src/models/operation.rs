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
    pub cid: Option<String>,
    pub path: String,
    pub prev: Option<String>
}
