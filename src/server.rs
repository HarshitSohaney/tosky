use tiny_http::{Server, Response};
use std::sync::{Arc};
use crate::db::Database;
use std::thread;

pub fn start_server() {
    let server = Arc::new(Server::http("0.0.0.0:3000").unwrap());
    let num_guards = 4;
    let mut guards = Vec::with_capacity(num_guards);

    for _ in 0 .. num_guards {
        // ARC is a smart pointer (Reference counting) - .clone() creates another pointer to server
        let server = server.clone();

        let guard = thread::spawn(move || {
            let db = Database::new("../db/posts.db");

            loop {
                let req = match server.recv() {
                    Ok(rq) => {
                        let url = rq.url();

                        if url.starts_with("/xrpc/app.bsky.feed.getFeedSkeleton") {
                            // Construct posts to send
                            let posts = db.read_last_n_posts_from(0, 15);

                            let feed: Vec<String> = posts.iter().map(|uri| format!(r#"{{"post":"{}"}}"#, uri)).collect();
                            let json = format!(r#"{{"feed":[{}]}}"#, feed.join(","));

                            let response = Response::from_string(json)
                                .with_header("Content-Type: application/json".parse::<tiny_http::Header>().unwrap());
                            rq.respond(response).ok();
                        } else {
                            // Return 404
                            let response = Response::from_string("Not Found").with_status_code(404);
                            rq.respond(response).ok();
                        }
                    },
                    Err(e) => {
                        eprintln!("Err: {}", e);
                        break
                    }
                };
            };
        });

        guards.push(guard);
    }
}