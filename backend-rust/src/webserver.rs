use actix_files as fs;
use actix_web::{App, HttpServer};
use std::io::Result;

pub async fn run_server(address: &str) -> Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(fs::Files::new("/", "./src/static").index_file("index.html"))
            // Add your routes and handlers here
    })
    .bind(address)?
    .run()
    .await
}
