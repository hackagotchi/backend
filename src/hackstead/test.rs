use super::get_hackstead;
use hcor::ServiceError;

#[actix_rt::test]
async fn test_get_hackstead() -> Result<(), ServiceError> {
    use crate::{data, data::to_doc};
    use actix_web::{App, HttpServer};
    use hcor::{Hackstead, UserContact};

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
            .run(),
    );

    let client = reqwest::Client::new();
    log::info!("requesting {:?}", bob_contact);
    let res = client
        .get("http://127.0.0.1:8000/hackstead/")
        .json(&bob_contact)
        .send()
        .await
        .expect("no send request");

    assert!(
        res.status().is_success(),
        "/hackstead/ Response status: {}",
        res.status()
    );

    let returned_bobstead: Hackstead = res.json().await.expect("bad json");
    assert_eq!(
        bobstead, returned_bobstead,
        "the backend returned the hackstead different than when it went in!"
    );

    Ok(())
}
