use actix_web::{App, HttpServer};

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    HttpServer::new(move || App::new().service(backend::get_hackstead))
        .bind("127.0.0.1:8000")?
        .run()
        .await
}
