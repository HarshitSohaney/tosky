use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Post {
    pub text: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    pub facets: Option<Vec<Facet>>,
    pub reply: Option<Reply>,
    pub embed: Option<Embed>,
    pub langs: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub labels: Option<SelfLabels>
}

#[derive(Debug, Deserialize)]
pub struct Facet {
    pub features: Vec<Feature>,
    pub index: ByteSlice
}

#[derive(Debug, Deserialize)]
pub struct ByteSlice {
    #[serde(rename = "byteStart")]
    pub byte_start: i32,
    #[serde(rename = "byteEnd")]
    pub byte_end: i32
}

#[derive(Debug, Deserialize)]
#[serde(tag = "$type")]
pub enum Feature {
    #[serde(rename = "app.bsky.richtext.facet#mention")]
    Mention { did: String },
    #[serde(rename = "app.bsky.richtext.facet#link")]
    Link { uri: String },
    #[serde(rename = "app.bsky.richtext.facet#tag", alias = "app.bsky.richtext.facet#hashtag")]
    Tag { tag: String },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
pub struct StrongRef {
    pub uri: String,
    pub cid: String,
}

#[derive(Debug, Deserialize)]
pub struct Reply {
    pub parent: StrongRef,
    pub root: StrongRef,
}

#[derive(Debug, Deserialize)]
pub struct Caption {
    lang: String,
    file: Vec<u8>
}

#[derive(Debug, Deserialize)]
#[serde(tag = "$type")]
pub enum Embed {
    #[serde(rename = "app.bsky.embed.record")]
    Record { 
        record: StrongRef 
    },
    #[serde(rename = "app.bsky.embed.images")]
    Images {
        images: Vec<Image>
    },
    #[serde(rename = "app.bsky.embed.video")]
    Video {
        // captions: Option<Vec<Captions>>,
        alt: Option<String>,
    },
    #[serde(other)] 
    Unknown
}

#[derive(Debug, Deserialize)]
pub struct AspectRatio {
    pub width: i32,
    pub height: i32
}

#[derive(Debug, Deserialize)]
pub struct Image {
    // pub image: Vec<u8>,
    pub alt: Option<String>,
}

#[derive(Debug)]
pub struct TorontoPost {
    pub uri: String,
    pub cid: String,
    pub did: String,
    pub indexed_at: i64,
    pub created_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct SelfLabels {
    pub values: Vec<LabelValue>
}

#[derive(Debug, Deserialize)]
pub struct LabelValue {
    pub val: String,
}
