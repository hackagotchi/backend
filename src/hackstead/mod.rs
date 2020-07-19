use crate::data;
use actix_web::{get, post, web, HttpResponse};
use hcor::{Hackstead, ServiceError, UserContact};
use log::*;

#[cfg(test)]
mod test;

#[get("/hackstead/")]
pub async fn get_hackstead(form: web::Json<UserContact>) -> Result<HttpResponse, ServiceError> {
    debug!("servicing get_hackstead request");

    let slack_id = form.slack().ok_or(ServiceError::bad_request("no slack"))?;
    let stead_bson = data::hacksteads()
        .await?
        .find_one(bson::doc! { "id": slack_id }, None)
        .await?
        .ok_or(ServiceError::NoData)?;

    let stead: Hackstead =
        bson::from_bson(stead_bson.into()).map_err(|_| ServiceError::InternalServerError)?;

    trace!("got hackstead: {:#?}", stead);

    Ok(HttpResponse::Ok().json(stead))
}

#[post("/hackstead/new")]
pub async fn new_hackstead(form: web::Json<UserContact>) -> Result<HttpResponse, ServiceError> {
    debug!("servicing new_hackstead request");

    let slack_id = form.slack().ok_or(ServiceError::bad_request("no slack"))?;
    let hs = Hackstead::new(slack_id);
    data::hacksteads()
        .await?
        .insert_one(data::to_doc(&hs)?, None)
        .await?;

    Ok(HttpResponse::Created().finish())
}

#[post("/hackstead/remove")]
pub async fn remove_hackstead(form: web::Json<UserContact>) -> Result<HttpResponse, ServiceError> {
    debug!("servicing remove_hackstead request");

    let slack_id = form.slack().ok_or(ServiceError::bad_request("no slack"))?;
    let stead_bson = data::hacksteads()
        .await?
        .find_one_and_delete(bson::doc! { "id": slack_id }, None)
        .await?
        .ok_or(ServiceError::NoData)?;

    let stead: Hackstead =
        bson::from_bson(stead_bson.into()).map_err(|_| ServiceError::InternalServerError)?;

    debug!(":( removed hackstead: {:#?}", stead);

    Ok(HttpResponse::Ok().json(stead))
}
