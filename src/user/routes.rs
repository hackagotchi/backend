use crate::data::get_mongo_database;
use super::{User, UserContact, UserRequest};
use actix_web::{get, web, HttpResponse};
use bson::doc;
use hcor::errors::ServiceError;

#[get("/user/")]
pub async fn get_user(form: web::Form<UserRequest>) -> Result<HttpResponse, ServiceError> {
    log::debug!("servicing get_user request");

    let user_id = form.id;
    let db = get_mongo_database("hackagotchi").await?;
    let users = db.collection("users");
    let filter = doc! {"id": user_id.to_string()};
    let user = users
        .find_one(filter, None)
        .await?
        .ok_or(ServiceError::NoData)?;
    log::info!("found user: {:?}", user);

    let res_contact = user.get("contact").ok_or(ServiceError::NoData)?;

    let uc: UserContact = bson::from_bson(res_contact.clone())
        .map_err(|_| ServiceError::InternalServerError)?;

    Ok(HttpResponse::Ok().json(User {
        id: user_id,
        contact: uc,
    }))
}

#[actix_rt::test]
async fn test_get_user() -> Result<(), ServiceError> {
    use reqwest::header::{CONTENT_TYPE, HeaderValue};
    use super::{User, UserContact};
    use crate::data::get_mongo_database;
    use actix_web::{App, HttpServer};

    pretty_env_logger::init();

    let bob = User::new(UserContact::Email("bob@bob.com".to_string()));
    let bob_request = bob.request();
    let bob_request_form = serde_qs::to_string(&bob_request)
        .expect("couldn't format user request");

    let db = get_mongo_database("hackagotchi").await.expect("no db");
    let users = db.collection("users");
    users.insert_one(crate::to_doc(&bob)?, None).await?;

    tokio::spawn(
        HttpServer::new(move || App::new().service(get_user))
            .bind("127.0.0.1:8000")
            .expect("couldn't bind port 8000")
            .run()
    );

    let client = reqwest::Client::new();
    log::info!("requesting {}", bob_request_form);
    let res = client
        .get("http://127.0.0.1:8000/user/")
        .header(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-www-form-urlencoded"),
        )
        .body(bob_request_form)
        .send()
        .await
        .expect("no send request");
    assert!(res.status().is_success(), "/user/ Response status: {}", res.status());

    Ok(())
}
