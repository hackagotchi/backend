#[actix_rt::test]
async fn plant_yield() -> hcor::ClientResult<()> {
    use super::true_or_timeout;
    use hcor::{wormhole::RudeNote::*, Hackstead, IdentifiesTile, CONFIG};
    use log::*;

    // attempt to establish logging, do nothing if it fails
    // (it probably fails because it's already been established in another test)
    drop(pretty_env_logger::try_init());

    let (seed_arch, yield_duration) = hcor::CONFIG
        .seeds()
        .filter_map(|(seed, seed_arch)| {
            Some((
                seed_arch,
                CONFIG
                    .find_plant(&seed.grows_into)
                    .ok()?
                    .base_yield_duration?,
            ))
        })
        .min_by_key(|(_, yd)| *yd as usize)
        .expect("no seeds in config that yield?");

    // create bob's stead!
    let bobstead = Hackstead::register().await?;

    // make plant
    let plant = bobstead
        .free_tile()
        .unwrap()
        .plant_seed(&seed_arch.spawn().await?)
        .await?;
    let tid = plant.tile_id();

    for i in 0..2 {
        true_or_timeout("yield", yield_duration, move |n| match n {
            &YieldFinish { tile_id, .. } => return tile_id == tid,
            _ => false,
        })
        .await;
        info!("yield {} completed!", i + 1);
    }

    // cleanup
    bobstead.slaughter().await?;

    Ok(())
}
