use actix_web::{get, web, App, HttpRequest, HttpResponse, HttpServer};

pub mod data;
pub mod models;
pub mod routes;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(move || App::new().service(routes::get::get_user))
        .bind("127.0.0.1:8000")?
        .run()
        .await
}
