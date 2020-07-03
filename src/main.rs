use actix_web::{App, HttpServer};
use hcor::errors::RequestError;

pub mod data;
pub mod hackstead;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    HttpServer::new(move || App::new().service(hackstead::get_hackstead))
        .bind("127.0.0.1:8000")?
        .run()
        .await
}

fn to_doc<S: serde::Serialize>(s: &S) -> Result<bson::Document, RequestError> {
    match bson::to_bson(s)? {
        bson::Bson::Document(d) => Ok(d),
        not_doc => Err(RequestError::NotDocument(not_doc))
    }
}
