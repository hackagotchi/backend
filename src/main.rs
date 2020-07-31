use actix_web::{web, App, HttpServer};

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    let db = web::Data::new(backend::db_pool().await.expect("couldn't make db pool"));

    HttpServer::new(move || {
        App::new()
            .app_data(db.clone())
            // hackstead
            .service(backend::get_hackstead)
            .service(backend::new_hackstead)
            .service(backend::remove_hackstead)
            // item
            .service(backend::spawn_items)
            .service(backend::transfer_items)
            .service(backend::hatch_item)
            // tile
            .service(backend::new_tile)
            // plant
            .service(backend::new_plant)
            .service(backend::remove_plant)
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await
}
