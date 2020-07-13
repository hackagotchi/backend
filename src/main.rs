use actix_web::{get, web, App, HttpRequest, HttpResponse, HttpServer};

pub mod data;
pub mod models;
pub mod routes;
pub mod middlewares;

#[get("/user/{id}")]
async fn get_user(_req: HttpRequest) -> HttpResponse {
    /*!
     * Gets a user from the API
     */

    HttpResponse::Ok().body("success")
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(move || App::new().service(get_user))
        .bind("127.0.0.1:8000")?
        .run()
        .await
}
