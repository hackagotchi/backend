use actix_web::{get, web, HttpResponse};
use hcor::{Hackstead, ServiceError, UserContact};

#[get("/hackstead/")]
pub async fn get_hackstead(form: web::Json<UserContact>) -> Result<HttpResponse, ServiceError> {
    log::debug!("servicing get_hackstead request");

    let slack_id = form.slack().ok_or(ServiceError::bad_request("no slack"))?;
    log::debug!("looking for {}", slack_id);

    let res = crate::data::hacksteads()
        .await?
        .find_one(bson::doc! { "id": slack_id }, None)
        .await?
        .ok_or(ServiceError::NoData)?;
    log::debug!("hackstead bson: {}", res);

    let stead: Hackstead =
        bson::from_bson(res.into()).map_err(|_| ServiceError::InternalServerError)?;

    log::debug!("hackstead: {:?}", stead);

    Ok(HttpResponse::Ok().json(stead))
}

#[cfg(test)]
mod test;
