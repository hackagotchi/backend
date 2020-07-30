use hcor::Hackstead;

/// NOTE: requires that at least one item exists in the config!
const ITEM_ARCHETYPE: hcor::config::ArchetypeHandle = 0;
const ITEM_SPAWN_COUNT: usize = 10;

#[actix_rt::test]
async fn test_spawn_item() -> hcor::ClientResult<()> {
    // attempt to establish logging, do nothing if it fails
    // (it probably fails because it's already been established in another test)
    drop(pretty_env_logger::try_init());

    // create bob's stead
    let mut bobstead = Hackstead::register().await?;

    // we'll need to keep track of how many items we have to see if spawning works.
    fn count_relevant_items(hackstead: &Hackstead) -> usize {
        hackstead
            .inventory
            .iter()
            .filter(|i| i.base.archetype_handle == ITEM_ARCHETYPE)
            .count()
    }
    let starting_item_count = count_relevant_items(&bobstead);

    // spawn bob some items and refresh his stead
    let items = bobstead
        .spawn_items(ITEM_ARCHETYPE, ITEM_SPAWN_COUNT)
        .await?;
    bobstead = Hackstead::fetch(&bobstead).await?;

    // make sure those new items are in there
    assert_eq!(
        count_relevant_items(&bobstead) - starting_item_count,
        ITEM_SPAWN_COUNT
    );

    // make sure each of the items the API says we got are in bob's inventory.
    for item in items {
        assert!(
            bobstead.inventory.contains(&item),
            "bobstead did not contain spawned item: \nitem: {:#?}\ninventory: {:#?}",
            item,
            bobstead.inventory
        );
    }

    // kill bob so he's not left in the database
    bobstead.slaughter().await?;

    Ok(())
}

#[actix_rt::test]
/// NOTE: requires that at least one item exists in the config!
/// relies on item/spawn!
async fn test_transfer_item() -> hcor::ClientResult<()> {
    // attempt to establish logging, do nothing if it fails
    // (it probably fails because it's already been established in another test)
    drop(pretty_env_logger::try_init());

    let mut bobstead = Hackstead::register().await?;
    let mut evestead = Hackstead::register().await?;

    // give bob some items
    let mut items = bobstead
        .spawn_items(ITEM_ARCHETYPE, ITEM_SPAWN_COUNT)
        .await?;

    // refresh our copy of bob's stead and assert that each item has only had one logged owner.
    for item in &items {
        assert_eq!(
            item.ownership_log.len(),
            1,
            "freshly spawned item has more than one owner: {:#?}",
            item
        );
        assert_eq!(
            item.ownership_log.first().unwrap().logged_owner_id,
            bobstead.profile.steader_id,
            "item spawned for bob doesn't log him as the first owner: {:#?}",
            item
        );
    }

    // give the items to eve
    items = bobstead.give_to(&evestead, &items).await?;
    bobstead = Hackstead::fetch(&bobstead).await?;
    evestead = Hackstead::fetch(&evestead).await?;

    // make sure bob doesn't have the items, but that eve does and their ownership log records bob
    // as the original owner.
    for item in &items {
        assert!(
            !bobstead.inventory.contains(item),
            "bob still has an item he gave away: {:#?}",
            item
        );
        assert!(
            evestead.inventory.contains(item),
            "eve doesn't have an item she was transferred: {:#?}",
            item
        );
        assert_eq!(
            item.ownership_log.len(),
            2,
            "spawned then traded item doesn't have two owners: {:#?}",
            item
        );
        assert_eq!(
            item.ownership_log.first().unwrap().logged_owner_id,
            bobstead.profile.steader_id,
            "item spawned for bob doesn't log him as the first owner: {:#?}",
            item
        );
        assert_eq!(
            item.ownership_log.get(1).unwrap().logged_owner_id,
            evestead.profile.steader_id,
            "item spawned for eve doesn't log her as the second owner: {:#?}",
            item
        );
    }

    bobstead.slaughter().await?;
    evestead.slaughter().await?;

    Ok(())
}
