use actix::Actor;
use actix_web::{web, App, HttpServer};

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    let wormhole = backend::WormholeServer::new().start();

    HttpServer::new(move || {
        App::new()
            .data(wormhole.clone())
            // wormhole
            .service(web::resource("/wormhole").to(backend::establish_wormhole))
            // hackstead
            .service(backend::get_hackstead)
            .service(backend::new_hackstead)
            .service(backend::remove_hackstead)
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await
}
