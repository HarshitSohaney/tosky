use sqlite::{Connection, State};
use crate::models::{Post, TorontoPost};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Database {
    conn: Connection,
    counter: i32
}

pub enum Column {
    Likes,
    Reposts,
}

pub struct Metadata {
    pub seq: i64
}

impl Database {
    pub fn new(path: &str) -> Self {
        let conn = sqlite::open(path).unwrap();

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

        Database { conn, counter: 0 }
    }

    pub fn insert_post(&mut self, post: &TorontoPost) -> Result<(), Box<dyn std::error::Error>> {
        if self.counter >= 1000 {
            self.pop_posts();
        }

        let mut stmt = self.conn.prepare("INSERT INTO posts (uri, cid, did, indexed_at) VALUES (?, ?, ?, ?)")?;
        stmt.bind((1, post.uri.as_str()))?;
        stmt.bind((2, post.cid.as_str()))?;
        stmt.bind((3, post.did.as_str()))?;
        stmt.bind((4, post.indexed_at))?;
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

    /// cursor is the indexed_at timestamp to paginate from
    pub fn read_posts(&self, limit: i64, cursor: Option<i64>) -> (Vec<String>, Option<String>) {
        let mut posts: Vec<String> = Vec::new();
        let mut last_indexed_at: Option<i64> = None;

        let (q, needs_cursor_bind) = match cursor {
            Some(_) => (
                "SELECT uri, indexed_at FROM posts WHERE indexed_at < ?
                ORDER BY ((score + 5) / (1.0 + ((strftime('%s', 'now') - indexed_at) / 3600.0) * 0.5) + ((strftime('%s', 'now') / 3600 + LENGTH(uri) * 7) % 10))
                DESC LIMIT ?",
                true
            ),
            None => (
                "SELECT uri, indexed_at
                FROM posts
                ORDER BY ((score + 5) / (1.0 + ((strftime('%s', 'now') - indexed_at) / 3600.0) * 0.5) + ((strftime('%s', 'now') / 3600 + LENGTH(uri) * 7) % 10))
                DESC LIMIT ?",
                false
            ),
        };

        let mut stmt = match self.conn.prepare(q) {
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

    pub fn get_posts_to_enrich(&self, limit: i64) -> Vec<String> {
        let mut posts: Vec<String> = Vec::new();

        let q = "
            SELECT uri
            FROM posts
            ORDER BY last_enriched
            ASC 
            LIMIT ?
        ";

        let mut stmt = match self.conn.prepare(q) {
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

    pub fn set_metadata(&self, metadata: &Metadata) {
        let q = "INSERT OR REPLACE INTO metadata (key, value) VALUES ('cursor', ?)";
        if let Ok(mut stmt) = self.conn.prepare(q) {
            stmt.bind((1, metadata.seq.to_string().as_str())).ok();
            stmt.next().ok();
        }
    }

    pub fn get_metadata(&self) -> Option<Metadata> {
        let q = "SELECT value FROM metadata WHERE key = 'cursor'";
        if let Ok(mut stmt) = self.conn.prepare(q) {
            if let Ok(State::Row) = stmt.next() {
                if let Ok(seq_str) = stmt.read::<String, _>(0) {
                    if let Ok(seq) = seq_str.parse::<i64>() {
                        return Some(Metadata { seq });
                    }
                }
            }
        }
        None
    }
}
