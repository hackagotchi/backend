use actix_web::{get, post, web, HttpResponse};
use log::*;
use hcor::{hackstead::TileCreationRequest, ServiceError};

/*
#[post("/hackstead/tile/new")]
pub async fn new_tile(form: web::Json<TileCreationRequest>) -> Result<HttpResponse, ServiceError> {
    debug!("servicing get_hackstead request");
    let stead = super::stead_from_db(form.steader.clone()).await?;
    trace!("got hackstead: {:#?}", stead);

    Ok(HttpResponse::Ok().json(stead))
}*/
