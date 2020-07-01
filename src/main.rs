use actix_web::{App, HttpServer};
use hcor::errors::{ServiceError, RequestError};

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

fn to_doc<S: serde::Serialize>(s: &S) -> Result<bson::Document, RequestError> {
    match bson::to_bson(s)? {
        bson::Bson::Document(d) => Ok(d),
        not_doc => Err(RequestError::NotDocument(not_doc))
    }
}

#[actix_rt::test]
async fn test_get_user() -> Result<(), ServiceError> {
    use models::{User, UserContact};
    use data::get_mongo_database;

    let bob = User::new(UserContact::Email("bob@bob.com".to_string()));

    let db = get_mongo_database("hackagotchi").await?;
    let users = db.collection("users");
    users.insert_one(to_doc(&bob)?, None).await?;

    tokio::spawn(async move { main() });

    let client = reqwest::Client::new();
    assert!(
        client
            .get("http://localhost:8000")
            .form(&bob)
            .send()
            .await
            .unwrap()
            .status()
            .is_success()
    );

    Ok(())
}
