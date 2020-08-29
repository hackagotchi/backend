#[cfg(all(feature = "hcor_client", test))]
#[actix_rt::test]
/// NOTE: relies on plant/new, item/spawn!
async fn slaughter() -> hcor::ClientResult<()> {
    use hcor::{Hackstead, Plant, Tile};
    use log::*;

    // attempt to establish logging, do nothing if it fails
    // (it probably fails because it's already been established in another test)
    drop(pretty_env_logger::try_init());

    let (_, seed_arch) = hcor::CONFIG
        .seeds()
        .next()
        .expect("no items in config that are seeds?");

    // create bob's stead!
    let mut bobstead = Hackstead::register().await?;
    let seed_item = seed_arch.spawn().await?;
    let tile = bobstead.free_tile().expect("new hackstead no open tiles");

    // we have to make a custom function for this because hcor doesn't provide an API
    // like this by default; it would allow people to make unnecessary requests to kill
    // plants on open tiles, which we only want to do for testing purposes here anyway.
    async fn slaughter_from_tile(tile: &Tile) -> hcor::ClientResult<Plant> {
        use hcor::{
            wormhole::{ask, until_ask_id_map, Ask, AskedNote, PlantAsk},
            ClientError,
        };
        let a = Ask::Plant(PlantAsk::Slaughter {
            tile_id: tile.tile_id,
        });

        let ask_id = ask(a.clone()).await?;

        until_ask_id_map(ask_id, |n| match n {
            AskedNote::PlantSlaughterResult(r) => Some(r),
            _ => None,
        })
        .await?
        .map_err(|e| ClientError::bad_ask(a, "PlantSlaughter", e))
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
    bobstead.server_sync().await?;
    assert!(
        !bobstead.free_tiles().any(|t| t.tile_id == tile.tile_id),
        "bob's plant is still not open even though we just killed its plant!"
    );

    // kill the plant
    doomed_plant = doomed_plant.slaughter().await?;

    // make sure there's no plant now
    bobstead.server_sync().await?;
    assert!(
        bobstead.free_tiles().any(|t| t.tile_id == tile.tile_id),
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
