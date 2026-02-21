use sqlite::{Connection, State};
use crate::models::{Post, TorontoPost};
use std::time::{SystemTime, UNIX_EPOCH};

// Ranking parameters
const BASE_SCORE: f64 = 5.0;      // Minimum score for new posts with no engagement
const DECAY_RATE: f64 = 0.05;     // Quadratic decay factor (age^2 * this)
const SHUFFLE_MOD: i32 = 5;      // Range of hourly shuffle (0 to N-1)
const SHUFFLE_MULT: i32 = 7;      // Multiplier for URI-based variance

pub struct Database {
    conn: Connection,
    counter: i32
}

pub enum Column {
    Likes,
    Reposts,
}

pub struct Metadata {
    pub seq: i64,
    pub last_updated: i64,
}

impl Database {
    pub fn new(path: &str) -> Self {
        let conn = sqlite::open(path).unwrap();
        conn.execute("PRAGMA journal_mode=WAL;").ok();
        conn.execute("PRAGMA busy_timeout=5000;").ok();

        let q = "
            CREATE TABLE IF NOT EXISTS posts (
                uri TEXT PRIMARY KEY,
                cid TEXT NOT NULL,
                did TEXT NOT NULL,
                indexed_at INTEGER NOT NULL,
                likes INTEGER DEFAULT 0,
                reposts INTEGER DEFAULT 0,
                quotes INTEGER DEFAULT 0,
                replies INTEGER DEFAULT 0,
                bookmarks INTEGER DEFAULT 0,
                score INTEGER DEFAULT 0,
                last_enriched INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
        ";

        if let Err(e) = conn.execute(q) {
            eprintln!("There was an creating the table {}", e);
        }

        // Migration: add created_at column if it doesn't exist
        conn.execute("ALTER TABLE posts ADD COLUMN created_at INTEGER DEFAULT 0").ok();

        Database { conn, counter: 0 }
    }

    pub fn insert_post(&mut self, post: &TorontoPost) -> Result<(), Box<dyn std::error::Error>> {
        if self.counter >= 1000 {
            self.pop_posts();
        }

        let mut stmt = self.conn.prepare("INSERT INTO posts (uri, cid, did, indexed_at, created_at) VALUES (?, ?, ?, ?, ?)")?;
        stmt.bind((1, post.uri.as_str()))?;
        stmt.bind((2, post.cid.as_str()))?;
        stmt.bind((3, post.did.as_str()))?;
        stmt.bind((4, post.indexed_at))?;
        stmt.bind((5, post.created_at))?;
        stmt.next()?;

        self.counter += 1;
        Ok(())
    }

    pub fn pop_posts(&mut self) {
        let q = "
            DELETE FROM posts WHERE indexed_at < (
            SELECT indexed_at FROM posts
            ORDER BY indexed_at DESC
            LIMIT 1 OFFSET 99999
        )";

        if let Err(e) = self.conn.execute(q) {
            eprintln!("There was an deleting the table {}", e);
        }

        self.counter = 0;
    }

    pub fn delete_post(&self, uri: &str) {
        let q = "DELETE FROM posts WHERE uri = ?";
        if let Ok(mut stmt) = self.conn.prepare(q) {
            stmt.bind((1, uri)).ok();
            stmt.next().ok();
        }
    }

    /// cursor is the indexed_at timestamp to paginate from
    pub fn read_posts(&self, limit: i64, cursor: Option<i64>, seed: u32) -> (Vec<String>, Option<String>) {
        let mut posts: Vec<String> = Vec::new();
        let mut last_indexed_at: Option<i64> = None;

        let age_hours = "(strftime('%s', 'now') - CASE WHEN created_at > 0 THEN created_at ELSE indexed_at END) / 3600.0";
        let ranking_formula = format!(
            "(score + {}) / (1.0 + ({} * {} * {})) + (({} + LENGTH(uri) * {}) % {})",
            BASE_SCORE, age_hours, age_hours, DECAY_RATE, seed, SHUFFLE_MULT, SHUFFLE_MOD
        );

        let (q, needs_cursor_bind) = match cursor {
            Some(_) => (
                format!(
                    "SELECT uri, indexed_at FROM posts WHERE indexed_at < ? ORDER BY ({}) DESC LIMIT ?",
                    ranking_formula
                ),
                true
            ),
            None => (
                format!(
                    "SELECT uri, indexed_at FROM posts ORDER BY ({}) DESC LIMIT ?",
                    ranking_formula
                ),
                false
            ),
        };

        let mut stmt = match self.conn.prepare(&q) {
            Ok(s) => s,
            Err(_) => return (posts, None)
        };

        if needs_cursor_bind {
            stmt.bind((1, cursor.unwrap())).ok();
            stmt.bind((2, limit)).ok();
        } else {
            stmt.bind((1, limit)).ok();
        }

        while let Ok(sqlite::State::Row) = stmt.next() {
            if let Ok(uri) = stmt.read::<String, _>(0) {
                posts.push(uri);
            }
            if let Ok(indexed_at) = stmt.read::<i64, _>(1) {
                last_indexed_at = Some(indexed_at);
            }
        }

        // Only return cursor if we got a full page (more results likely)
        let next_cursor = if posts.len() == limit as usize {
            last_indexed_at.map(|ts| ts.to_string())
        } else {
            None
        };

        (posts, next_cursor)
    }

    pub fn increment_col(&self, uri: &str, column: Column) -> Result<(), Box<dyn std::error::Error>> {
        let q = match column {
            Column::Likes => "UPDATE posts SET likes = likes + 1, score = (likes + 1) + reposts * 2 WHERE uri = ?",
            Column::Reposts => "UPDATE posts SET reposts = reposts + 1, score = likes + (reposts + 1) * 2 WHERE uri = ?",
        };
        
        let mut stmt = self.conn.prepare(q)?;
        stmt.bind((1, uri))?;
        stmt.next()?;

        Ok(())
    }

    pub fn has_unenriched_posts(&self) -> bool {
        let q = "SELECT 1 FROM posts WHERE last_enriched = 0 LIMIT 1";
        if let Ok(mut stmt) = self.conn.prepare(q) {
            if let Ok(State::Row) = stmt.next() {
                return true;
            }
        }
        false
    }

    pub fn get_posts_to_enrich(&self, limit: i64) -> Vec<String> {
        let mut posts: Vec<String> = Vec::new();

        let q = "
            SELECT uri
            FROM posts
            ORDER BY (created_at = 0) DESC, last_enriched ASC
            LIMIT ?
        ";

        let mut stmt = match self.conn.prepare(&q) {
            Ok(s) => s,
            Err(_) => return posts,
        };

        stmt.bind((1, limit));

        while let Ok(sqlite::State::Row) = stmt.next() {
            if let Ok(uri) = stmt.read::<String, _>(0) {
                posts.push(uri);
            }
        }

        posts
    }

    pub fn backfill_created_at(&self, uri: &str, created_at: i64) {
        let q = "UPDATE posts SET created_at = ? WHERE uri = ? AND created_at = 0";
        if let Ok(mut stmt) = self.conn.prepare(q) {
            stmt.bind((1, created_at)).ok();
            stmt.bind((2, uri)).ok();
            stmt.next().ok();
        }
    }

    pub fn update_engagement(&self,
        uri: &str,
        likes: i64,
        reposts: i64,
        quotes: i64,
        replies: i64,
        bookmarks: i64) {

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let score = likes + reposts*2 + quotes*3 + replies + bookmarks;

        let q = "UPDATE posts SET likes=?, reposts=?, quotes=?, replies=?, bookmarks=?, score=?, last_enriched=? WHERE
    uri=?";

        if let Ok(mut stmt) = self.conn.prepare(q) {
            stmt.bind((1, likes)).ok();
            stmt.bind((2, reposts)).ok();
            stmt.bind((3, quotes)).ok();
            stmt.bind((4, replies)).ok();
            stmt.bind((5, bookmarks)).ok();
            stmt.bind((6, score)).ok();
            stmt.bind((7, now)).ok();
            stmt.bind((8, uri)).ok();
            stmt.next().ok();
        }
    }

    pub fn insert_post_if_not_exists(
        &mut self,
        uri: &str,
        cid: &str,
        did: &str,
        created_at: i64,
        likes: i64,
        reposts: i64,
        quotes: i64,
        replies: i64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.counter >= 1000 {
            self.pop_posts();
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let score = likes + reposts * 2 + quotes * 3 + replies;

        let mut stmt = self.conn.prepare(
            "INSERT OR IGNORE INTO posts (uri, cid, did, indexed_at, created_at, likes, reposts, quotes, replies, score, last_enriched)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )?;
        stmt.bind((1, uri))?;
        stmt.bind((2, cid))?;
        stmt.bind((3, did))?;
        stmt.bind((4, now))?;
        stmt.bind((5, created_at))?;
        stmt.bind((6, likes))?;
        stmt.bind((7, reposts))?;
        stmt.bind((8, quotes))?;
        stmt.bind((9, replies))?;
        stmt.bind((10, score))?;
        stmt.bind((11, now))?;
        stmt.next()?;

        self.counter += 1;
        Ok(())
    }

    pub fn set_metadata(&self, metadata: &Metadata) {
        let q = "INSERT OR REPLACE INTO metadata (key, value) VALUES ('cursor', ?)";
        if let Ok(mut stmt) = self.conn.prepare(q) {
            stmt.bind((1, metadata.seq.to_string().as_str())).ok();
            stmt.next().ok();
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let q2 = "INSERT OR REPLACE INTO metadata (key, value) VALUES ('cursor_updated', ?)";
        if let Ok(mut stmt) = self.conn.prepare(q2) {
            stmt.bind((1, now.to_string().as_str())).ok();
            stmt.next().ok();
        }
    }

    pub fn get_metadata(&self) -> Option<Metadata> {
        let mut seq: Option<i64> = None;
        let mut last_updated: i64 = 0;

        let q = "SELECT value FROM metadata WHERE key = 'cursor'";
        if let Ok(mut stmt) = self.conn.prepare(q) {
            if let Ok(State::Row) = stmt.next() {
                if let Ok(seq_str) = stmt.read::<String, _>(0) {
                    seq = seq_str.parse::<i64>().ok();
                }
            }
        }

        let q2 = "SELECT value FROM metadata WHERE key = 'cursor_updated'";
        if let Ok(mut stmt) = self.conn.prepare(q2) {
            if let Ok(State::Row) = stmt.next() {
                if let Ok(ts_str) = stmt.read::<String, _>(0) {
                    last_updated = ts_str.parse::<i64>().unwrap_or(0);
                }
            }
        }

        seq.map(|s| Metadata { seq: s, last_updated })
    }
}
