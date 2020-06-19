use actix_web::{get, App, HttpServer, HttpResponse, HttpRequest, web};

pub mod routes;
pub mod models;

#[get("/user/{id}")]
async fn get_user(_req: HttpRequest) -> HttpResponse {
    /*!
     * Gets a user from the API
     */

    HttpResponse::Ok().body("success")
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    
    HttpServer::new(move || {
        App::new()
            .service(get_user)
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await
}
