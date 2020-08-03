#[actix_rt::test]
/// NOTE: relies on item/spawn!
async fn test_new_tile() -> hcor::ClientResult<()> {
    use hcor::Hackstead;

    // attempt to establish logging, do nothing if it fails
    // (it probably fails because it's already been established in another test)
    drop(pretty_env_logger::try_init());

    let requires_xp_arch = hcor::CONFIG
        .land_unlockers()
        .find(|(ul, _)| ul.requires_xp)
        .expect("no items in config that unlock land and require xp to do so?")
        .1;
    let no_requires_xp_arch = hcor::CONFIG
        .land_unlockers()
        .find(|(ul, _)| !ul.requires_xp)
        .expect("no items in config that unlock land and don't require xp to do so?")
        .1;
    let non_land_redeemable_arch = hcor::CONFIG
        .possession_archetypes
        .iter()
        .find(|x| x.unlocks_land.is_none())
        .expect("no items in config that don't unlock land?");

    // create bob's stead!
    let mut bobstead = Hackstead::register().await?;
    let starting_tile_count = bobstead.land.len();

    struct NewTileAssumptions {
        expected_success: bool,
        item_consumed: bool,
        expected_tiles: usize,
    }

    async fn new_tile_assuming(
        bobstead: &mut Hackstead,
        item: &hcor::Item,
        assumptions: NewTileAssumptions,
    ) -> hcor::ClientResult<()> {
        let requested_tile = item.redeem_for_tile().await;

        match (assumptions.expected_success, requested_tile) {
            (true, Ok(tile)) => assert_eq!(
                tile.base.owner_id, bobstead.profile.steader_id,
                "tile spawned for bob doesn't belong to bob: {:#?}",
                tile
            ),
            (false, Err(e)) => log::info!("/tile/new failed as expected: {}", e),
            (true, Err(e)) => panic!("/tile/new unexpectedly failed: {}", e),
            (false, Ok(tile)) => panic!("/tile/new unexpectedly returned tile: {:#?}", tile),
        };

        *bobstead = Hackstead::fetch(&*bobstead).await?;

        assert_eq!(
            bobstead.land.len(),
            assumptions.expected_tiles,
            "bob doesn't have the expected number of extra tiles",
        );

        assert_eq!(
            assumptions.item_consumed,
            !bobstead.has_item(item),
            "bob's land redeemable item was unexpectedly {}",
            if assumptions.item_consumed {
                "not consumed"
            } else {
                "consumed"
            }
        );

        Ok(())
    }

    // spawn bob an item he can redeem for a tile if he has enough xp
    let requires_xp_item = requires_xp_arch.spawn_for(&bobstead).await?;

    // try and redeem this item bob doesn't have enough xp to redeem for land
    new_tile_assuming(
        &mut bobstead,
        &requires_xp_item,
        NewTileAssumptions {
            expected_success: false,
            item_consumed: false,
            expected_tiles: starting_tile_count,
        },
    )
    .await?;

    // spawn an item bob can redeem for land without having enough xp
    let no_requires_xp_item = no_requires_xp_arch.spawn_for(&bobstead).await?;

    // try and redeem that item, this should actually work
    new_tile_assuming(
        &mut bobstead,
        &no_requires_xp_item,
        NewTileAssumptions {
            expected_success: true,
            item_consumed: true,
            expected_tiles: starting_tile_count + 1,
        },
    )
    .await?;

    // give bob enough xp to unlock the next level (hopefully)
    sqlx::query!(
        "UPDATE steaders SET xp = $1 WHERE steader_id = $2",
        std::i32::MAX,
        bobstead.profile.steader_id
    )
    .execute(crate::db_conn().await.unwrap())
    .await
    .expect("couldn't set xp in db");

    // try and redeem the first item that does require xp to work, should work now.
    new_tile_assuming(
        &mut bobstead,
        &requires_xp_item,
        NewTileAssumptions {
            expected_success: true,
            item_consumed: true,
            expected_tiles: starting_tile_count + 2,
        },
    )
    .await?;

    // try and redeem those items we've already used up
    new_tile_assuming(
        &mut bobstead,
        &requires_xp_item,
        NewTileAssumptions {
            expected_success: false,
            item_consumed: true,
            expected_tiles: starting_tile_count + 2,
        },
    )
    .await?;
    new_tile_assuming(
        &mut bobstead,
        &no_requires_xp_item,
        NewTileAssumptions {
            expected_success: false,
            item_consumed: true,
            expected_tiles: starting_tile_count + 2,
        },
    )
    .await?;

    // try to redeem the non-land-redeemable item for land
    let non_land_redeemable_item = non_land_redeemable_arch.spawn_for(&bobstead).await?;
    new_tile_assuming(
        &mut bobstead,
        &non_land_redeemable_item,
        NewTileAssumptions {
            expected_success: false,
            item_consumed: false,
            expected_tiles: starting_tile_count + 2,
        },
    )
    .await?;

    // kill bob so he's not left in the database
    bobstead.slaughter().await?;

    Ok(())
}
