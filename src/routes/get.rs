use crate::data::get_mongo_database;
use crate::models::{User, UserContact, UserRequest};
use actix_web::{get, web, HttpRequest, HttpResponse};
use bson::{doc, Bson};
use hcor::errors::ServiceError;

#[get("/user/")]
pub async fn get_user(form: web::Form<UserRequest>) -> Result<HttpResponse, ServiceError> {
    let user_id = form.id;
    let db = get_mongo_database("hackagotchi").await?;
    let collection = db.collection("users");
    let filter = doc! {"id": user_id.to_string()};
    let result = collection
        .find_one(filter, None)
        .await?
        .ok_or(ServiceError::NoData)?;

    let res_contact = match result.get("contact") {
        Some(bson) => bson,
        None => {
            return Err(ServiceError::NoData);
        }
    };

    let uc: UserContact = match bson::from_bson(res_contact.clone()) {
        Ok(uc) => uc,
        Err(_) => {
            return Err(ServiceError::InternalServerError);
        }
    };

    Ok(HttpResponse::Ok().json(User {
        id: user_id,
        contact: uc,
    }))
}
