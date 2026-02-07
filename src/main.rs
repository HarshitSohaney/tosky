mod models;
mod db;
mod ingestion;
mod parser;
mod filter;
mod server;
use std::thread;

use crate::db::Database;
use crate::filter::Filter;

fn main() {
    let ingestion_handle = thread::spawn(|| {
        let db = Database::new("../db/posts.db");
        let mut filter: Filter = Filter::new(db);

        if let Err(e) = ingestion::start_ingestion(&mut filter) {
            eprintln!("Error: {}", e);
        }
    });

    let server_handle = thread::spawn(|| {
        server::start_server();
    });

    ingestion_handle.join().unwrap();
    server_handle.join().unwrap();
}
