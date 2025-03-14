mod webserver;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    webserver::run_server("0.0.0.0:80").await
}
