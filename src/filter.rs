use crate::models::{Feature, Frame, Operation, Post, TorontoPost, Embed, StrongRef, InteractionType};
use crate::db::{Column, Database};
use lru::LruCache;
use std::num::NonZeroUsize;

use std::time::{SystemTime, UNIX_EPOCH};

pub struct Filter {
    pub db: Database,
    toronto_uris: LruCache<String, ()>,
    lax_keywords: Vec<&'static str>,
    strict_keywords: Vec<&'static str>
}

impl Filter {
    pub fn new(db: Database) -> Self {
        Filter {
            db, 
            toronto_uris: LruCache::new(NonZeroUsize::new(100_000).unwrap()),
            lax_keywords: vec![
                "toronto",
                "torono",
            ],
            strict_keywords: vec![
                "ttc",
                "cn tower",
                "6ix",
                "Danforth Music Hall",
                "bluejays",
                "Scotiabank arena",
                "air canada centre",
                "Rogers centre",
                "Rogers Stadium",
                "Trillium Park",
                "Olivia Chow",
                "Kensington Market",
                "Yonge",
                "Roncesvalles",
                "YYZ",
                "metrolinx"
            ]
        }
    }

    fn is_6ix_post(&self, post: &Post) -> bool {
        if let Some(Embed::Record { record }) = &post.embed {
            if self.toronto_uris.contains(&record.uri) {
                return true;
            }
        }

        let mut text = post.text.to_lowercase();

        // We're just going to concat all the text in the post to text 
        // (facets included) and then just do one check

        for facet in post.facets.iter().flatten() {
            for feat in &facet.features {
                match feat {
                    Feature::Tag { tag } => {
                        text = text + " TAG: " + &tag.to_lowercase()
                    },
                    Feature::Link { uri } => {
                        text = text + " LINK: " + &uri.to_lowercase()
                    },
                    _ => {}
                }
            }
        }

        if let Some(Embed::Video { alt: Some(alt_text)}) = &post.embed {
            text = text + " ALT: " + &alt_text.to_lowercase()
        }

        if self.lax_keywords.iter().any(|k| text.contains(k)) 
            || self.strict_keywords.iter().any(|k| self.contains_word_strict(&text, k)) {
            return true;
        }

        false
    }

    fn contains_word_strict(&self, text: &str, word: &str) -> bool {
        text.split_whitespace()
          .any(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase() == word)
    }

    fn bytes_to_hex(&self, bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    pub fn on_interaction(&mut self, subject: &StrongRef, interaction_type: InteractionType) {
        if interaction_type == InteractionType::LIKE {
            if let Err(res) = self.db.increment_col(&subject.uri, Column::Likes) {
                eprintln!("Ran into an error {}", res);
            }
        } else if interaction_type == InteractionType::REPOST {
            if let Err(res) = self.db.increment_col(&subject.uri, Column::Reposts) {
                eprintln!("Ran into an error {}", res);
            }
        }
    }

    pub fn callback(&mut self, frame: &Frame, op: &Operation, post: &Post) {
        if !self.is_6ix_post(post) {
            return;
        }

        println!("---POST--- \n {}\n ------- \n", post.text);

        let toronto_post = TorontoPost {
            uri: format!("at://{}/{}", frame.repo, op.path),
            cid: self.bytes_to_hex(&op.cid.as_ref().unwrap()[1..]),
            did: frame.repo.clone(),
            indexed_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
        };

        if let Err(e) = self.db.insert_post(&toronto_post) {
            eprintln!("Failed to insert post: {}", e);
        }

        self.toronto_uris.put(toronto_post.uri.clone(), ());
    }
}
