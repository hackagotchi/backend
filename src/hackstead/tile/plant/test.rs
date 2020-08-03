use log::*;

#[actix_rt::test]
/// NOTE: relies on item/spawn!
async fn test_new_plant() -> hcor::ClientResult<()> {
    use hcor::{Hackstead, Item, Tile};

    // attempt to establish logging, do nothing if it fails
    // (it probably fails because it's already been established in another test)
    drop(pretty_env_logger::try_init());

    let (_, seed_arch) = hcor::CONFIG
        .seeds()
        .next()
        .expect("no items in config that are seeds?");
    let not_seed_arch = hcor::CONFIG
        .possession_archetypes
        .iter()
        .find(|a| a.seed.is_none())
        .expect("no items in config that aren't seeds?");

    // create bob's stead!
    let mut bobstead = Hackstead::register().await?;

    let seed_item = seed_arch.spawn_for(&bobstead).await?;
    let not_seed_item = not_seed_arch.spawn_for(&bobstead).await?;
    let open_tile = bobstead
        .open_tiles()
        .next()
        .expect("fresh hackstead no open land?")
        .clone();

    struct NewPlantAssumptions {
        expected_success: bool,
        item_consumed: bool,
    }

    async fn new_plant_assuming(
        bobstead: &mut Hackstead,
        tile: &Tile,
        seed_item: &Item,
        assumptions: NewPlantAssumptions,
    ) -> hcor::ClientResult<()> {
        let requested_plant = tile.plant_seed(seed_item).await;

        match (assumptions.expected_success, requested_plant) {
            (true, Ok(plant)) => {
                assert_eq!(
                    plant.base.tile_id, tile.base.tile_id,
                    "plant planted for bob is on a different tile than expected: {:#?}",
                    plant
                );
                assert_eq!(
                    seed_item.seed.as_ref().unwrap().grows_into,
                    plant.name,
                    "seed grew into unexpected type of plant"
                );
            }
            (false, Err(e)) => info!("/plant/new failed as expected: {}", e),
            (true, Err(e)) => panic!("/plant/new unexpectedly failed: {}", e),
            (false, Ok(tile)) => panic!("/plant/new unexpectedly returned plant: {:#?}", tile),
        };

        *bobstead = Hackstead::fetch(&*bobstead).await?;

        assert_eq!(
            assumptions.item_consumed,
            !bobstead.has_item(seed_item),
            "bob's seed item was unexpectedly {}",
            if assumptions.item_consumed {
                "not consumed"
            } else {
                "consumed"
            }
        );

        Ok(())
    }

    // try to plant this non-seed item
    new_plant_assuming(
        &mut bobstead,
        &open_tile,
        &not_seed_item,
        NewPlantAssumptions {
            expected_success: false,
            item_consumed: false,
        },
    )
    .await?;

    // try and redeem an item that's actually a seed, this should actually work
    new_plant_assuming(
        &mut bobstead,
        &open_tile,
        &seed_item,
        NewPlantAssumptions {
            expected_success: true,
            item_consumed: true,
        },
    )
    .await?;

    // try and redeem the item that's already been consumed
    new_plant_assuming(
        &mut bobstead,
        &open_tile,
        &seed_item,
        NewPlantAssumptions {
            expected_success: false,
            item_consumed: true,
        },
    )
    .await?;

    // kill bob so he's not left in the database
    bobstead.slaughter().await?;

    Ok(())
}

#[actix_rt::test]
/// NOTE: relies on plant/new, item/spawn!
async fn test_plant_remove() -> hcor::ClientResult<()> {
    use hcor::{Hackstead, Plant, Tile};

    // attempt to establish logging, do nothing if it fails
    // (it probably fails because it's already been established in another test)
    drop(pretty_env_logger::try_init());

    let (_, seed_arch) = hcor::CONFIG
        .seeds()
        .next()
        .expect("no items in config that are seeds?");

    // create bob's stead!
    let mut bobstead = Hackstead::register().await?;
    let seed_item = seed_arch.spawn_for(&bobstead).await?;
    let tile = bobstead
        .open_tiles()
        .next()
        .expect("new hackstead no open tiles")
        .clone();

    // we have to make a custom function for this because hcor doesn't provide an API
    // like this by default; it would allow people to make unnecessary requests to kill
    // plants on open tiles, which we only want to do for testing purposes here anyway.
    async fn slaughter_from_tile(tile: &Tile) -> hcor::ClientResult<Plant> {
        hcor::client_internal::request(
            "plant/slaughter",
            &hcor::plant::PlantRemovalRequest {
                tile_id: tile.base.tile_id,
            },
        )
        .await
    };

    // try to kill his plant when he still doesn't have one.
    // (let's hope this fails)
    match slaughter_from_tile(&tile).await {
        Ok(p) => panic!(
            "plant/remove somehow killed plant on an open tile: {:#?}",
            p
        ),
        Err(e) => info!(
            "got error as expected upon killing nonexistant plant: {}",
            e
        ),
    };

    // now let's actually give him a plant to kill
    let mut doomed_plant: Plant = tile.plant_seed(&seed_item).await?;

    // make sure that tile is no longer open
    bobstead = Hackstead::fetch(&bobstead).await?;
    assert!(
        !bobstead
            .open_tiles()
            .any(|t| t.base.tile_id == tile.base.tile_id),
        "bob's plant is still not open even though we just killed its plant!"
    );

    // kill the plant
    doomed_plant = doomed_plant.slaughter().await?;

    // make sure there's no plant now
    bobstead = Hackstead::fetch(&bobstead).await?;
    assert!(
        bobstead
            .open_tiles()
            .any(|t| t.base.tile_id == tile.base.tile_id),
        "bob's plant is still not open even though we just killed its plant!"
    );

    // now let's try to kill that dead plant, again
    match doomed_plant.slaughter().await {
        Ok(p) => panic!("plant/remove somehow killed a plant twice: {:#?}", p),
        Err(e) => info!("got error as expected upon killing dead plant: {}", e),
    };

    // kill bob so he's not left in the db
    bobstead.slaughter().await?;

    Ok(())
}

/*
#[actix_rt::test]
/// NOTE: relies on plant/new, item/spawn, plant/rub!
async fn test_plant_craft() -> hcor::ClientResult<()> {
    use hcor::{Hackstead, Plant};

    // attempt to establish logging, do nothing if it fails
    // (it probably fails because it's already been established in another test)
    drop(pretty_env_logger::try_init());

    let seed_arch = hcor::CONFIG
        .possession_archetypes
        .iter()
        .find(|a| a.seed.is_some())
        .expect("no items in config that are seeds?");

    // create bob's stead!
    let mut bobstead = Hackstead::register().await?;
    let seed_item = seed_arch.spawn_for(&bobstead).await?;
    let tile = bobstead
        .open_tiles()
        .next()
        .expect("new hackstead no open tiles")
        .clone();
    let plant = tile.plant_seed(&seed_item).await?;

    bobstead.slaughter();
}*/

#[actix_rt::test]
/// NOTE: relies on plant/new, item/spawn!
async fn test_plant_rub() -> hcor::ClientResult<()> {
    use futures::{stream, StreamExt};
    use hcor::{Hackstead, Note::*};
    use std::time::Duration;
    use tokio::time::timeout;

    // attempt to establish logging, do nothing if it fails
    // (it probably fails because it's already been established in another test)
    drop(pretty_env_logger::try_init());

    let (seed_arch, rub_wear_off_arch) = hcor::CONFIG
        .seeds()
        .find_map(|(seed, seed_arch)| {
            Some((
                seed_arch,
                hcor::CONFIG.possession_archetypes.iter().find(|a| {
                    a.rub_effects_for_plant(&seed.grows_into)
                        .any(|e| e.duration.is_some())
                })?,
            ))
        })
        .expect("no items in config that are seeds we can rub with effects that wear off?");

    // create bob's stead!
    let mut bobstead = Hackstead::register().await?;
    let seed_item = seed_arch.spawn_for(&bobstead).await?;
    let rub_item = rub_wear_off_arch.spawn_for(&bobstead).await?;
    let tile = bobstead
        .open_tiles()
        .next()
        .expect("new hackstead no open tiles")
        .clone();
    let mut plant = tile.plant_seed(&seed_item).await?;
    let effects = plant.rub_with(&rub_item).await?;

    bobstead = Hackstead::fetch(&bobstead).await?;
    plant = bobstead.plant(&plant).unwrap().clone();
    assert_eq!(
        plant.effects, effects,
        "brand new plant has more effects than those from the item that was just rubbed on",
    );
    assert!(
        rub_wear_off_arch
            .rub_effects_for_plant(&plant.name)
            .enumerate()
            .all(|(i, _)| {
                effects
                    .iter()
                    .any(|e| e.effect_archetype_handle == i as hcor::config::ArchetypeHandle)
            }),
        "the effects of this item we just rubbed on can't be found on this plant"
    );

    let wormhole = hcor::Wormhole::new(&bobstead).await?;
    let wh = &wormhole;

    stream::iter(effects.into_iter().filter(|e| e.duration.is_some()))
        .for_each_concurrent(None, |e| async move {
            const ERR_MARGIN_SECS: f64 = 0.2;

            let expected_ticks = e.duration.unwrap();
            let mut expected_ticks_left = expected_ticks;
            let expected_seconds = expected_ticks / hcor::UPDATES_PER_SECOND as f64 + ERR_MARGIN_SECS;
            let expected_duration = Duration::from_millis((expected_seconds * 1000.0) as u64);

            info!(
                "preparing to wait no more than {:.4} seconds for this effect to wear off",
                expected_seconds
            );

            let until_effect_finish = wh.until(|note| {
                debug!("note from wormhole: {:#?}", note);
                match note {
                    PlantEffectFinish { effect, .. } => return effect.rub_index == e.rub_index,
                    PlantEffectProgress {
                        rub_index,
                        until_finish,
                        ..
                    } if rub_index == e.rub_index => {
                        assert_eq!(
                            until_finish,
                            expected_ticks_left,
                            "updates out of order or skipped or repeated?",
                        );
                        expected_ticks_left -= 1.0;

                        info!(
                            "[plant effect wearing off progress: [{:.3}% complete]]",
                            100.0 - (until_finish / expected_ticks as f64) * 100.0
                        );
                    }
                    _ => {}
                }

                false
            });

            timeout(expected_duration, until_effect_finish)
                .await
                .expect("time out waiting for effect to finish")
                .expect("wormhole error while waiting for effect to finish wearing off");

            info!("plant effect wore off in expected time!");
        })
        .await;
    wormhole.disconnect();

    bobstead.slaughter().await?;

    Ok(())
}
