mod models;
mod db;
mod ingestion;
mod parser;
mod filter;
mod server;
mod enrichment;
mod backfill;
use std::thread;

use crate::db::Database;
use crate::enrichment::EnrichThread;
use crate::filter::Filter;

fn db_path() -> String {
    std::env::var("TOSKY_DB_PATH").unwrap_or_else(|_| "../db/posts.db".to_string())
}

fn main() {
    let db_path = db_path();

    {
        let _ = Database::new(&db_path);
    }

    let enrichment_db_path = db_path.clone();
    let _enrichment_handle = thread::spawn(move || {
        let mut enrich = EnrichThread::new(&enrichment_db_path);

        loop {
            if let Err(e) = enrich.enrich_what_we_missed() {
                eprintln!("Error when enriching: {}", e);
            }

            let sleep_secs = enrich.sleep_duration_secs();
            std::thread::sleep(std::time::Duration::from_secs(sleep_secs));
        }

    });

    let server_db_path = db_path.clone();
    let _server_handle = thread::spawn(move || {
        server::start_server(&server_db_path);
    });

    // Run backfill synchronously on main thread before starting ingestion
    {
        let mut db = Database::new(&db_path);
        backfill::run_backfill(&mut db);
    }

    // Monitor ingestion thread and respawn if it panics
    loop {
        let ingestion_db_path = db_path.clone();
        let ingestion_handle = thread::spawn(move || {
            let db = Database::new(&ingestion_db_path);
            let mut filter: Filter = Filter::new(db);

            ingestion::start_ingestion(&mut filter);
        });

        match ingestion_handle.join() {
            Ok(_) => {
                eprintln!("Ingestion thread exited cleanly, restarting...");
            }
            Err(e) => {
                eprintln!("Ingestion thread panicked: {:?}, restarting in 5s...", e);
                thread::sleep(std::time::Duration::from_secs(5));
            }
        }
    }
}
