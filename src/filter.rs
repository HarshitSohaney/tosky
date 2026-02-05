use crate::models::{Post, Frame, Operation, TorontoPost};
use crate::db::Database;
use lru::LruCache;
use std::num::NonZeroUsize;

use std::time::{SystemTime};

pub struct Filter {
    db: Database,
    toronto_uris: LruCache<String, ()>,
    keywords: Vec<&'static str>,
}

impl Filter {
    pub fn new(db: Database) -> Self {
        Filter {
            db, 
            toronto_uris: LruCache::new(NonZeroUsize::new(100_000).unwrap()),
            keywords: vec![
                "toronto",
                "ttc",
                "cn tower",
                "torono",
                "6ix"
            ],
        }
    }

    fn is_6ix_post(&self, post: &Post) -> bool {
        let text = post.text.to_lowercase();
        self.keywords.iter().any(|k| text.contains(k))
    }

    pub fn bytes_to_hex(&self, bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    pub fn callback(&self, frame: &Frame, op: &Operation, post: &Post) {
        if !self.is_6ix_post(post) {
            return;
        }

        println!("---POST--- \n {}\n ------- \n", post.text);

        let toronto_post = TorontoPost {
            uri: format!("at://{}/{}", frame.repo, op.path),
            cid: self.bytes_to_hex(op.cid.as_ref().unwrap()),
            did: frame.repo.clone(),
            indexed_at: SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
        };

        self.db.insert_post(&toronto_post);
    }
}
