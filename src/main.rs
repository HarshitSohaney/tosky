mod models;
mod db;
mod ingestion;
mod parser;
mod filter;
mod server;

fn main() {
    println!("Hello, world!");
    if let Err(e) = ingestion::start_ingestion() {
        eprintln!("Error: {}", e);
    }
}
