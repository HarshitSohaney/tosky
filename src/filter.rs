use crate::models::{Feature, Frame, Operation, Post, TorontoPost, Embed, StrongRef, InteractionType};
use crate::db::{Column, Database};
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use std::time::{SystemTime, UNIX_EPOCH};

pub const NSFW_LABELS: &[&str] = &["porn", "nudity", "sexual", "graphic-media", "nsfw"];
const CAUGHT_UP_THRESHOLD_SECS: i64 = 3600; // 1 hour

pub struct Filter {
    pub db: Database,
    toronto_uris: LruCache<String, ()>,
    lax_keywords: Vec<&'static str>,
    strict_keywords: Vec<&'static str>,
    caught_up: Arc<AtomicBool>,
    logged_caught_up: bool,
}

impl Filter {
    pub fn new(db: Database, caught_up: Arc<AtomicBool>) -> Self {
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
            ],
            caught_up,
            logged_caught_up: false,
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

    fn is_nsfw(&self, post: &Post) -> bool {
        if let Some(labels) = &post.labels {
            return labels.values.iter().any(|l| NSFW_LABELS.contains(&l.val.as_str()));
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
        if !self.logged_caught_up && !self.caught_up.load(Ordering::Relaxed) {
            if let Ok(created) = chrono::DateTime::parse_from_rfc3339(&post.created_at) {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;
                let post_ts = created.timestamp();
                if (now - post_ts).abs() < CAUGHT_UP_THRESHOLD_SECS {
                    println!("[Ingestion] Caught up to live firehose (post age {}s)", now - post_ts);
                    self.caught_up.store(true, Ordering::Relaxed);
                    self.logged_caught_up = true;
                }
            }
        }

        if self.is_nsfw(post) || !self.is_6ix_post(post) {
            return;
        }

        println!("---POST [{}]--- \n {}\n ------- \n", post.created_at, post.text);

        let created_at = chrono::DateTime::parse_from_rfc3339(&post.created_at)
            .map(|dt| dt.timestamp())
            .unwrap_or_else(|_| SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64);

        let toronto_post = TorontoPost {
            uri: format!("at://{}/{}", frame.repo, op.path),
            cid: self.bytes_to_hex(&op.cid.as_ref().unwrap()[1..]),
            did: frame.repo.clone(),
            indexed_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64,
            created_at,
        };

        if let Err(e) = self.db.insert_post(&toronto_post) {
            eprintln!("Failed to insert post: {}", e);
        }

        self.toronto_uris.put(toronto_post.uri.clone(), ());
    }
}
