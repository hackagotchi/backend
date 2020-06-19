use crate::models::{User, UserContact};
use actix_web::{get, web, HttpRequest, HttpResponse};
use bson::doc;
use fll_scoring::{data::get_mongo_database, errors::ServiceError};

#[get("/user/")]
pub async fn get_user(form: web::Form<User>) -> Result<HttpResponse, ServiceError> {
    let user_id = form.id;
    let db = get_mongo_database().await?;
    let collection = db.collection("users");
    let filter = doc! {"id": user_id.to_string()};
    let result = collection
        .find_one(filter, None)
        .await?
        .ok_or(ServiceError::NoData)?;

    Ok(HttpResponse::Ok().json(User {
        id: user_id,
        contact: form.contact.clone(),
    }))
}
