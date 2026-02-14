mod models;
mod db;
mod ingestion;
mod parser;
mod filter;
mod server;
mod enrichment;
use std::thread;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::db::Database;
use crate::enrichment::EnrichThread;
use crate::filter::Filter;

fn main() {
    {
        let _ = Database::new("../db/posts.db");
    }

    let caught_up = Arc::new(AtomicBool::new(false));

    let enrichment_handle = thread::spawn(move || {
        let mut enrich = EnrichThread::new("../db/posts.db");

        loop {
            if let Err(e) = enrich.enrich_what_we_missed() {
                eprintln!("Error when enriching: {}", e);
            }

            let sleep_secs = enrich.sleep_duration_secs();
            std::thread::sleep(std::time::Duration::from_secs(sleep_secs));
        }

    });

    let caught_up_ingest = caught_up.clone();
    let ingestion_handle = thread::spawn(move || {
        let db = Database::new("../db/posts.db");
        let mut filter: Filter = Filter::new(db, caught_up_ingest);

        ingestion::start_ingestion(&mut filter);
    });

    let server_handle = thread::spawn(|| {
        server::start_server();
    });

    enrichment_handle.join().unwrap();
    ingestion_handle.join().unwrap();
    server_handle.join().unwrap();
}
