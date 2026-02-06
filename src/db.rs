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

}



