use hcor::{Hackstead, UserId};

const PORT: usize = 8000;
const ITEM_ARCHETYPE: hcor::config::ArchetypeHandle = 0;
const ITEM_SPAWN_COUNT: usize = 10;

#[actix_rt::test]
/// NOTE: requires that at least one item exists in the config!
async fn test_spawn_item() -> Result<(), crate::ServiceError> {

    // attempt to establish logging, do nothing if it fails
    // (it probably fails because it's already been established in another test)
    drop(pretty_env_logger::try_init());

    let bobstead = {
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

        res.json::<Hackstead>().await.expect("bad new stead json")
    };
    let bob_id = UserId::Uuid(bobstead.profile.steader_id);

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

    let count_relevant_items = |hackstead: &Hackstead| {
        hackstead
            .inventory
            .iter()
            .filter(|i| i.base.archetype_handle == ITEM_ARCHETYPE)
            .count()
    };
    let starting_item_count = count_relevant_items(&bobstead);

    let items = (0..ITEM_SPAWN_COUNT)
        .map(move |_| hcor::Item::from_archetype_handle(
            ITEM_ARCHETYPE,
            bobstead.profile.steader_id,
            hcor::item::Acquisition::spawned()
        ))
        .collect::<Vec<hcor::Item>>();

    {
        let res = reqwest::Client::new()
            .post(&format!("http://127.0.0.1:{}/item/spawn", PORT))
            .json(&items)
            .send()
            .await
            .expect("bad post item/spawn request");

        assert!(
            res.status().is_success(),
            "/item/spawn Response status (expected success): {}",
            res.status()
        );
    }

    let bobstead = get_bob().await;
    assert_eq!(
        count_relevant_items(&bobstead) - starting_item_count,
        ITEM_SPAWN_COUNT
    );
    for item in items {
        assert!(
            bobstead.inventory.contains(&item),
            "bobstead did not contain spawned item: \nitem: {:#?}\ninventory: {:#?}",
            item,
            bobstead.inventory
        );
    }
    
    // kill bob so he's not left in the database
    reqwest::Client::new()
        .post(&format!("http://127.0.0.1:{}/hackstead/remove", PORT))
        .json(&bob_id)
        .send()
        .await
        .expect("no send request");

    Ok(())
}

#[actix_rt::test]
/// NOTE: requires that at least one item exists in the config!
/// relies on item/spawn!
async fn test_transfer_item() -> Result<(), crate::ServiceError> {
    let new_stead = || async {
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

        res.json::<Hackstead>().await.expect("bad new stead json")
    };
    let get_stead = |steader_id| async move {
        let res = reqwest::Client::new()
            .get(&format!("http://127.0.0.1:{}/hackstead/", PORT))
            .json(&UserId::Uuid(steader_id))
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

    let bobstead = new_stead().await;
    let bob_steader_id = bobstead.profile.steader_id;
    let evestead = new_stead().await;

    // give bob some items
    let item_ids: Vec<uuid::Uuid> = {
        let items = (0..ITEM_SPAWN_COUNT)
            .map(move |_| hcor::Item::from_archetype_handle(
                ITEM_ARCHETYPE,
                bobstead.profile.steader_id,
                hcor::item::Acquisition::spawned()
            ))
            .collect::<Vec<hcor::Item>>();

        let res = reqwest::Client::new()
            .post(&format!("http://127.0.0.1:{}/item/spawn", PORT))
            .json(&items)
            .send()
            .await
            .expect("bad post item/sapwn request");

        assert!(
            res.status().is_success(),
            "/item/spawn Response status (expected success): {}",
            res.status()
        );

        items.into_iter().map(|i| i.base.item_id).collect()
    };

    // refresh our copy of bob's stead and assert that each item has only one entry in its
    // ownership log.
    let bobstead = get_stead(bob_steader_id).await;
    for &item_id in &item_ids {
        let item = bobstead
            .inventory
            .iter()
            .find(|i| i.base.item_id == item_id)
            .expect("spawned item isn't in bob's inventory");

        assert_eq!(
            item.ownership_log,
            vec![hcor::item::LoggedOwner {
                logged_owner_id: bobstead.profile.steader_id,
                item_id,
                acquisition: hcor::item::Acquisition::spawned(),
                owner_index: 0,
            }]
        );
    }

    // give the items to eve
    {
        let res = reqwest::Client::new()
            .post(&format!("http://127.0.0.1:{}/item/transfer", PORT))
            .json(&hcor::item::ItemTransferRequest {
                sender_id: UserId::Uuid(bobstead.profile.steader_id),
                receiver_id: UserId::Uuid(evestead.profile.steader_id),
                item_ids: item_ids.clone(),
            })
            .send()
            .await
            .expect("bad post item/transfer request");

        assert!(
            res.status().is_success(),
            "/item/transfer Response status (expected success): {}",
            res.status()
        );
    }

    Ok(())
}
