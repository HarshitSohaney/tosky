use sqlite::{Connection, State};
use crate::models::{Post, TorontoPost};

pub struct Database {
    pub conn: Connection
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

        Database { conn }
    }

    pub fn insert_post(&self, post: &TorontoPost) -> Result<(), Box<dyn std::error::Error>> {
        let mut stmt = self.conn.prepare("INSERT INTO posts (uri, cid, did, indexed_at) VALUES (?, ?, ?, ?)")?;
        stmt.bind((1, post.uri.as_str()))?;
        stmt.bind((2, post.cid.as_str()))?;
        stmt.bind((3, post.did.as_str()))?;
        stmt.bind((4, post.indexed_at))?;
        stmt.next()?;
        
        Ok(())
    }

}



