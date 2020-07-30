use actix_web::{web, App, HttpServer};

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    let db = web::Data::new(backend::db_pool().await.expect("couldn't make db pool"));

    HttpServer::new(move || {
        App::new()
            .app_data(db.clone())
            .service(backend::get_hackstead)
            .service(backend::new_hackstead)
            .service(backend::remove_hackstead)
            .service(backend::spawn_items)
            .service(backend::transfer_items)
            .service(backend::new_tile)
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await
}
