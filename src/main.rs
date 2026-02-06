mod models;
mod db;
mod ingestion;
mod parser;
mod filter;
mod server;

use crate::db::Database;
use crate::filter::Filter;

fn main() {
    let db = Database::new("../db/posts.db");
    let mut filter: Filter = Filter::new(db);

    if let Err(e) = ingestion::start_ingestion(&mut filter) {
        eprintln!("Error: {}", e);
    }
}
