use crate::data::get_mongo_database;
use crate::models::{User, UserContact, UserRequest};
use actix_web::{get, web, HttpResponse};
use bson::doc;
use hcor::errors::ServiceError;

#[get("/user/")]
pub async fn get_user(form: web::Form<UserRequest>) -> Result<HttpResponse, ServiceError> {
    let user_id = form.id;
    let db = get_mongo_database("hackagotchi").await?;
    let users = db.collection("users");
    let filter = doc! {"id": user_id.to_string()};
    let result = users
        .find_one(filter, None)
        .await?
        .ok_or(ServiceError::NoData)?;

    let res_contact = result.get("contact").ok_or(ServiceError::NoData)?;

    let uc: UserContact = bson::from_bson(res_contact.clone())
        .map_err(|_| ServiceError::InternalServerError)?;

    Ok(HttpResponse::Ok().json(User {
        id: user_id,
        contact: uc,
    }))
}
