use sqlite::{Connection, State};
use crate::models::{Post, TorontoPost};

pub struct Database {
    conn: Connection,
    counter: i32
}

impl Database {
    pub fn new(path: &str) -> Self {
        let conn = sqlite::open(path).unwrap();

        let q = "
            CREATE TABLE IF NOT EXISTS posts (
                uri TEXT PRIMARY KEY,
                cid TEXT NOT NULL,
                did TEXT NOT NULL,
                indexed_at INTEGER NOT NULL
            )
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
                "SELECT uri, indexed_at FROM posts WHERE indexed_at < ? ORDER BY indexed_at DESC LIMIT ?",
                true
            ),
            None => (
                "SELECT uri, indexed_at FROM posts ORDER BY indexed_at DESC LIMIT ?",
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
}



