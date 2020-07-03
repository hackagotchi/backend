use hcor::{Hackstead, UserContact};
use actix_web::{get, web, HttpResponse};
use bson::doc;
use hcor::errors::ServiceError;

#[get("/hackstead/")]
pub async fn get_hackstead(form: web::Json<UserContact>) -> Result<HttpResponse, ServiceError> {
    log::debug!("servicing get_hackstead request");

    let slack_id = form
        .slack()
        .ok_or(ServiceError::bad_request("no slack"))?;
    log::debug!("looking for {}", slack_id);

    let res = crate::data::hacksteads()
        .await?
        .find_one(doc! { "id": slack_id }, None)
        .await?
        .ok_or(ServiceError::NoData)?;
    log::debug!("hackstead bson: {}", res);

    let stead: Hackstead = bson::from_bson(res.into())
        .map_err(|_| ServiceError::InternalServerError)?;

    log::debug!("hackstead: {:?}", stead);

    Ok(HttpResponse::Ok().json(stead))
}

#[actix_rt::test]
async fn test_get_hackstead() -> Result<(), ServiceError> {
    use hcor::{Hackstead, UserContact};
    use crate::{data, to_doc};
    use actix_web::{App, HttpServer};

    pretty_env_logger::init();

    const BOB_ID: &'static str = "U14MB0B";
    let bob_contact = UserContact::Slack(BOB_ID.to_string());
    let bobstead = Hackstead::new(BOB_ID);

    log::debug!("bobstead doc: {}", to_doc(&bobstead)?);
    let hacksteads = data::hacksteads().await?;
    hacksteads.insert_one(to_doc(&bobstead)?, None).await?;

    tokio::spawn(
        HttpServer::new(move || App::new().service(get_hackstead))
            .bind("127.0.0.1:8000")
            .expect("couldn't bind port 8000")
            .run()
    );

    let client = reqwest::Client::new();
    log::info!("requesting {:?}", bob_contact);
    let res = client
        .get("http://127.0.0.1:8000/hackstead/")
        .json(&bob_contact)
        .send()
        .await
        .expect("no send request");
    assert!(res.status().is_success(), "/hackstead/ Response status: {}", res.status());

    Ok(())
}
