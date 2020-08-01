use hcor::{Hackstead, IdentifiesSteader};
use log::*;

const ITEM_ARCHETYPE: hcor::config::ArchetypeHandle = 0;
const ITEM_SPAWN_COUNT: usize = 10;

#[actix_rt::test]
/// NOTE: requires that at least one item exists in the config!
/// relies on item/spawn!
async fn test_spawn_item() -> hcor::ClientResult<()> {
    // attempt to establish logging, do nothing if it fails
    // (it probably fails because it's already been established in another test)
    drop(pretty_env_logger::try_init());

    debug!("create bob's stead");
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

    debug!("spawn bob some items and refresh his stead");
    let items = bobstead
        .spawn_items(ITEM_ARCHETYPE, ITEM_SPAWN_COUNT)
        .await?;
    bobstead = Hackstead::fetch(&bobstead).await?;

    debug!("make sure those new items are in there");
    assert_eq!(
        count_relevant_items(&bobstead) - starting_item_count,
        ITEM_SPAWN_COUNT
    );

    debug!("make sure each of the items the API says we got are in bob's inventory.");
    for item in items {
        assert!(
            bobstead.inventory.contains(&item),
            "bobstead did not contain spawned item: \nitem: {:#?}\ninventory: {:#?}",
            item,
            bobstead.inventory
        );
    }

    debug!("kill bob so he's not left in the database");
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

    debug!("give bob some items");
    let mut items = bobstead
        .spawn_items(ITEM_ARCHETYPE, ITEM_SPAWN_COUNT)
        .await?;

    debug!(
        "refresh our copy of bob's stead and assert that \
          each item has only had one logged owner."
    );
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

    debug!("give the items to eve");
    items = bobstead.throw_items(&evestead, &items).await?;
    bobstead = Hackstead::fetch(&bobstead).await?;
    evestead = Hackstead::fetch(&evestead).await?;

    debug!(
        "make sure bob doesn't have the items, \
        but that eve does and their ownership log records bob \
        as the original owner."
    );
    for item in &items {
        assert!(
            !bobstead.has_item(item),
            "bob still has an item he gave away: {:#?}",
            item
        );
        assert!(
            evestead.has_item(item),
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

    debug!("transfer a single item back to bob");
    let item = items.last().unwrap().throw_at(&bobstead).await?;
    debug!("make sure bob shows up in two different places now");
    assert_eq!(
        vec![
            bobstead.steader_id(),
            evestead.steader_id(),
            bobstead.steader_id()
        ],
        item.ownership_log
            .iter()
            .map(|o| o.logged_owner_id)
            .collect::<Vec<_>>(),
    );

    debug!("try to give an item from bob to bob");
    match item.throw_at(&bobstead).await {
        Err(e) => info!("received error as expected trying to self-give: {}", e),
        Ok(i) => panic!("unexpectedly able to give item to self: {:#?}", i),
    }

    debug!("try to give bob's item to bob as if eve owns it");
    match evestead.throw_items(&bobstead, &vec![item]).await {
        Err(e) => info!(
            "received error as expected trying to give someone else's item: {}",
            e
        ),
        Ok(i) => panic!("unexpectedly able to give someone else's item: {:#?}", i),
    }

    debug!("make sure all gives still fail when the items are of mixed ownership");
    match evestead.throw_items(&bobstead, &items).await {
        Err(e) => info!(
            "received error as expected trying to give items of mixed ownership: {}",
            e
        ),
        Ok(i) => panic!(
            "unexpectedly able to give away items of mixed ownership: {:#?}",
            i
        ),
    }

    debug!(
        "mixed ownership transaction should have completely failed \
        so eve should still have her items, \
        excluding the last one which was legitimately given to bob."
    );
    items.pop();
    evestead = Hackstead::fetch(&evestead).await?;
    for item in &items {
        assert!(
            evestead.has_item(item),
            "mixed-ownership transaction partially succeeded, item was transferred: {:#?}",
            item
        )
    }

    bobstead.slaughter().await?;
    evestead.slaughter().await?;

    Ok(())
}

#[actix_rt::test]
/// NOTE: requires that hatchable and unhatchable items exist in the config!
/// relies on item/spawn!
async fn test_hatch_item() -> hcor::ClientResult<()> {
    // attempt to establish logging, do nothing if it fails
    // (it probably fails because it's already been established in another test)
    drop(pretty_env_logger::try_init());

    let mut bobstead = Hackstead::register().await?;

    debug!("finding prequisites in config...");
    let unhatchable_arch = hcor::CONFIG
        .possession_archetypes
        .iter()
        .find(|x| x.hatch_table.is_none())
        .expect("no unhatchable items in config?");
    let hatchable_arch = hcor::CONFIG
        .possession_archetypes
        .iter()
        .find(|x| x.hatch_table.is_some())
        .expect("no unhatchable items in config?");

    debug!("to prepare, we need to spawn bob hatchable and unhatchable items.");
    let unhatchable_item = unhatchable_arch.spawn_for(&bobstead).await?;
    let hatchable_item = hatchable_arch.spawn_for(&bobstead).await?;
    bobstead = Hackstead::fetch(&bobstead).await?;

    debug!("let's start off by hatching the unhatchable and making sure that doesn't work.");
    match unhatchable_item.hatch().await {
        Ok(items) => panic!("unhatchable item unexpectedly hatched into: {:#?}", items),
        Err(e) => info!(
            "received error as expected upon attempting to hatch unhatchable item: {}",
            e
        ),
    }
    assert_eq!(
        bobstead.inventory.len(),
        Hackstead::fetch(&bobstead).await?.inventory.len(),
        "failing to hatch modified inventory item count somehow",
    );

    debug!("great, now let's try actually hatching something hatchable!");
    let hatched_items = hatchable_item.hatch().await?;

    debug!(
        "let's make sure bob's inventory grew proportionally \
          to the amount of items hatching produced"
    );
    let starting_inventory = bobstead.inventory.clone();
    let new_inventory = Hackstead::fetch(&bobstead).await?.inventory;
    assert_eq!(
        hatched_items.len(),
        (new_inventory.len() - (starting_inventory.len() - 1)),
        "the number of items in bob's inventory changed differently \
            than the number of items produced by hatching this item, somehow. \
            starting inventory: {:#?}\nitems hatched: {:#?}\nnew inventory: {:#?}",
        starting_inventory,
        hatched_items,
        new_inventory,
    );

    debug!("okay, but can we hatch the already hatched item?");
    match hatchable_item.hatch().await {
        Ok(items) => panic!(
            "pretty sure I ain't supposed to be able \
                to hatch this twice, got: {:#?}",
            items
        ),
        Err(e) => info!(
            "got error as expected from hatching already hatched item: {}",
            e
        ),
    }

    debug!("kill bob so he's not left in the database");
    bobstead.slaughter().await?;

    Ok(())
}
