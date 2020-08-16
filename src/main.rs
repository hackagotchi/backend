use actix::Actor;
use actix_web::{web, App, HttpServer};

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    let wormhole = backend::WormholeServer::new().start();

    // create fs dirs where we dump the data, if they don't already exist
    let mkdir = |name| {
        std::fs::create_dir(name).unwrap_or_else(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => {}
            _ => panic!("couldn't make {} folder: {}", name, e),
        })
    };
    mkdir("stead");
    mkdir("slack");

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
