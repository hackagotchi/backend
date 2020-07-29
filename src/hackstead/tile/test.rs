const PORT: usize = 9001;

#[actix_rt::test]
/// NOTE: relies on item/spawn!
async fn test_new_tile() -> Result<(), crate::ServiceError> {
    use actix_web::{App, HttpServer};
    use hcor::{Hackstead, UserId};

    // attempt to establish logging, do nothing if it fails
    // (it probably fails because it's already been established in another test)
    drop(pretty_env_logger::try_init());

    tokio::spawn(
        HttpServer::new(move || {
            App::new()
                .service(crate::hackstead::new_hackstead)
                .service(crate::hackstead::get_hackstead)
                .service(crate::hackstead::remove_hackstead)
                .service(crate::hackstead::item::spawn_items)
                .service(super::new_tile)
        })
        .bind(&format!("127.0.0.1:{}", PORT))
        .expect(&format!("couldn't bind port {}", PORT))
        .run(),
    );

    let no_requires_xp_arch = hcor::CONFIG
        .possession_archetypes
        .iter()
        .find(|i| {
            i.unlocks_land
                .as_ref()
                .filter(|ul| !ul.requires_xp)
                .is_some()
        })
        .expect("no items in config that unlock land and don't require xp to do so?");
    let requires_xp_arch = hcor::CONFIG
        .possession_archetypes
        .iter()
        .find(|i| {
            i.unlocks_land
                .as_ref()
                .filter(|ul| ul.requires_xp)
                .is_some()
        })
        .expect("no items in config that unlock land and require xp to do so?");

    // create bob's stead!
    let new_bobstead = {
        let res = reqwest::Client::new()
            .post(&format!("http://127.0.0.1:{}/hackstead/new", PORT))
            .json(&hcor::hackstead::NewHacksteadRequest { slack_id: None })
            .send()
            .await
            .expect("bad hackstead/new request");

        assert!(
            res.status().is_success(),
            "/hackstead/new Response status: {}",
            res.status()
        );

        res.json::<Hackstead>()
            .await
            .expect("bad new stead json")
    };
    let starting_tile_count = new_bobstead.land.len();
    let bob_steader_id = new_bobstead.profile.steader_id;
    let bob_id = UserId::Uuid(bob_steader_id);

    let get_bob = || async {
        let res = reqwest::Client::new()
            .get(&format!("http://127.0.0.1:{}/hackstead/", PORT))
            .json(&bob_id.clone())
            .send()
            .await
            .expect("bad get /hackstead/ request");

        assert!(
            res.status().is_success(),
            "/tile/new Response status: {}",
            res.status()
        );

        res.json::<Hackstead>().await.expect("bad get stead json")
    };

    let spawn_bob = |arch| async move {
        let res = reqwest::Client::new()
            .post(&format!("http://127.0.0.1:{}/item/spawn", PORT))
            .json(&vec![hcor::Item::from_archetype(
                arch,
                bob_steader_id,
                hcor::item::Acquisition::Trade,
            )])
            .send()
            .await
            .expect("bad send item/spawn request");

        assert!(
            res.status().is_success(),
            "/item/spawn Response status: {}",
            res.status()
        );

        get_bob()
            .await
            .inventory
            .into_iter()
            .find(|i| i.name == arch.name)
            .unwrap_or_else(|| panic!(
                "item/spawn didn't put {} in bob's inventory",
                arch.name
            ))
            .base
            .item_id
    };

    let new_tile_bob = |item| async move {
        reqwest::Client::new()
            .post(&format!("http://127.0.0.1:{}/tile/new", PORT))
            .json(&hcor::hackstead::TileCreationRequest {
                tile_consumable_item_id: item,
                steader: UserId::Uuid(bob_steader_id),
            })
            .send()
            .await
            .expect("bad send tile/new request")
    };

    let new_tile_bob_and_check = |item_id, expected_extra| async move {
        let res = new_tile_bob(item_id).await;
        assert!(
            res.status().is_success(),
            "/tile/new Response Status (expected success): {}\n{:#?}",
            res.status(),
            res.text().await
        );

        let bobstead = get_bob().await;

        assert_eq!(
            bobstead.land.len(),
            (starting_tile_count + expected_extra),
            "bob didn't get that extra tile?",
        );

        assert!(
            !bobstead.inventory.into_iter().any(|i| i.base.item_id == item_id),
            "bob still has his land giving item?",
        );
    };

    // spawn bob an item he can redeem for a tile if he has enough xp
    let requires_xp_item_id = spawn_bob(requires_xp_arch).await;

    // try and redeem this item bob doesn't have enough xp to redeem for land
    {
        let res = new_tile_bob(requires_xp_item_id).await;
        assert!(
            res.status().is_client_error(),
            "/tile/new Response Status (expected client error): {}\n{:#?}",
            res.status(),
            res.text().await
        );
    }

    // spawn an item bob can redeem for land without having enough xp
    let no_requires_xp_item_id = spawn_bob(no_requires_xp_arch).await;

    // try and redeem that item, this should actually work
    new_tile_bob_and_check(no_requires_xp_item_id, 1).await;

    // give bob enough xp to unlock the next level (hopefully)
    sqlx::query!(
        "UPDATE steaders SET xp = $1 WHERE steader_id = $2",
        std::i32::MAX,
        bob_steader_id
    )
    .execute(crate::db_conn().await?)
    .await?;

    // try and redeem the first item that does require xp to work, should work now.
    new_tile_bob_and_check(requires_xp_item_id, 2).await;

    // kill bob so he's not left in the database
    reqwest::Client::new()
        .post(&format!("http://127.0.0.1:{}/hackstead/remove", PORT))
        .json(&bob_id)
        .send()
        .await
        .expect("no send request");

    Ok(())
}
