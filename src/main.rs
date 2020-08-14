use actix::Actor;
use actix_web::{web, App, HttpServer};

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    let wormhole = backend::WormholeServer::new().start();

    HttpServer::new(move || {
        App::new().service(
            web::scope("/api")
                .data(wormhole.clone())
                // wormhole
                .service(web::resource("/wormhole").to(backend::establish_wormhole))
                // hackstead
                .service(backend::hackstead_summon)
                .service(backend::hackstead_spy)
                .service(backend::hackstead_slaughter)
                // beg
                .service(backend::beg),
        )
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await
}
