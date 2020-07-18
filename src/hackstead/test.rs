use hcor::ServiceError;

#[actix_rt::test]
async fn hackstead_routes() -> Result<(), ServiceError> {
    use actix_web::{App, HttpServer};
    use hcor::{Hackstead, UserContact};

    pretty_env_logger::init();

    tokio::spawn(
        HttpServer::new(move || {
            App::new()
                .service(super::new_hackstead)
                .service(super::get_hackstead)
                .service(super::remove_hackstead)
        })
        .bind("127.0.0.1:8000")
        .expect("couldn't bind port 8000")
        .run(),
    );

    // create bob's stead!
    const BOB_ID: &'static str = "U14MB0B";
    let bob_contact = UserContact::Slack(BOB_ID.to_string());
    {
        let res = reqwest::Client::new()
            .post("http://127.0.0.1:8000/hackstead/new")
            .json(&bob_contact)
            .send()
            .await
            .expect("no send request");

        assert!(
            res.status().is_success(),
            "/hackstead/new Response status: {}",
            res.status()
        );
    }

    let get_bob = || async {
        reqwest::Client::new()
            .get("http://127.0.0.1:8000/hackstead/")
            .json(&bob_contact)
            .send()
            .await
            .expect("no send request")
    };
    let kill_bob = || async {
        reqwest::Client::new()
            .post("http://127.0.0.1:8000/hackstead/remove")
            .json(&bob_contact)
            .send()
            .await
            .expect("no send request")
    };

    // fetch bob
    let bobstead: Hackstead = {
        log::info!("requesting {:?}", bob_contact);
        let res = get_bob().await;

        assert!(
            res.status().is_success(),
            "/hackstead/ Response status: {}",
            res.status()
        );
        res.json().await.expect("bad json")
    };

    // now kill bob
    {
        let res = kill_bob().await;
        assert!(
            res.status().is_success(),
            "/hackstead/remove Response status: {}",
            res.status()
        );
        let returned_bobstead: Hackstead = res.json().await.expect("bad json");
        assert_eq!(
            bobstead, returned_bobstead,
            "the hackstead returned from /hackstead/remove is different than the one from /hackstead/!"
        );
    }

    // make sure we can't get ded bob
    {
        let res = get_bob().await;
        assert!(
            res.status().is_client_error(),
            concat!(
                "the backend didn't successfully kill bob, ",
                "/hackstead/ Response status: {}",
            ),
            res.status()
        );
    }

    Ok(())
}
