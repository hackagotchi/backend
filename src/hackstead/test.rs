use crate::ServiceError;
const PORT: usize = 8000;

#[actix_rt::test]
async fn test_get_hackstead() -> Result<(), ServiceError> {
    use actix_web::{App, HttpServer};
    use hcor::{Hackstead, UserId};

    // attempt to establish logging, do nothing if it fails
    // (it probably fails because it's already been established in another test)
    drop(pretty_env_logger::try_init());

    tokio::spawn(
        HttpServer::new(move || {
            App::new()
                .service(super::new_hackstead)
                .service(super::get_hackstead)
                .service(super::remove_hackstead)
        })
        .bind(&format!("127.0.0.1:{}", PORT))
        .expect(&format!("couldn't bind port {}", PORT))
        .run(),
    );

    // create bob's stead!
    let bob_steader_id = {
        let res = reqwest::Client::new()
            .post(&format!("http://127.0.0.1:{}/hackstead/new", PORT))
            .json(&hcor::hackstead::NewHacksteadRequest {
                slack_id: None,
            })
            .send()
            .await
            .expect("no send request");

        assert!(
            res.status().is_success(),
            "/hackstead/new Response status: {}",
            res.status()
        );

        res
            .json::<Hackstead>()
            .await
            .expect("new hackstead bad json")
            .profile
            .steader_id
    };
    let bob_id = UserId::Uuid(bob_steader_id);

    let get_bob = || async {
        reqwest::Client::new()
            .get(&format!("http://127.0.0.1:{}/hackstead/", PORT))
            .json(&bob_id)
            .send()
            .await
            .expect("no send request")
    };
    let kill_bob = || async {
        reqwest::Client::new()
            .post(&format!("http://127.0.0.1:{}/hackstead/remove", PORT))
            .json(&bob_id)
            .send()
            .await
            .expect("no send request")
    };

    // fetch bob
    let bobstead: Hackstead = {
        log::info!("requesting {:?}", bob_id);
        let res = get_bob().await;

        assert!(
            res.status().is_success(),
            "/hackstead/ Response status: {}",
            res.status()
        );
        res.json().await.expect("bad get hackstead json")
    };

    // now kill bob
    {
        let res = kill_bob().await;
        assert!(
            res.status().is_success(),
            "/hackstead/remove Response status: {}",
            res.status()
        );
        let returned_bobstead: Hackstead = res.json().await.expect("bad kill hackstead json");
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
