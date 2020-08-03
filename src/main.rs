use actix::Actor;
use actix_web::{web, App, HttpServer};

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    let db = backend::db_pool().await.expect("couldn't make db pool");
    let wormhole = backend::WormholeServer::new(db.clone()).start();
    let dbd = web::Data::new(db);

    HttpServer::new(move || {
        App::new()
            .app_data(dbd.clone())
            .data(wormhole.clone())
            // wormhole
            .service(web::resource("/wormhole").to(backend::establish_wormhole))
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
            .service(backend::rub_plant)
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await
}
