use actix_web::{get, HttpRequest, HttpResponse, web};
use fll_scoring::{data::get_mongo_database, errors::ServiceError};
use bson::doc;
use crate::models::{User, UserContact};

#[get("/user/")]
pub async fn get_user(form: web::Form<User>) -> Result<HttpResponse, ServiceError> {

    let user_id = form.id;
    let db = get_mongo_database().await?;
    let collection = db.collection("users");
    let filter = doc! {"id": user_id.to_string()};
    let result = match collection.find_one(filter, None).await {
        Ok(opt) => match opt {
            Some(doc) => doc,
            None => {
                return Err(ServiceError::NoData);
            }
        },
        Err(err) => {
            return Err(err.into());
        }
    };
    let res_user_id = user_id;
    let res_contact = form.contact.clone();

    let json = web::Json(User {
        id: res_user_id,
        contact: res_contact,
    });


    Ok(HttpResponse::Ok().json(json.0))
}