#[actix_rt::test]
/// NOTE: relies on plant/new, item/spawn, plant/apply!
async fn plant_rub_wear_off() -> hcor::ClientResult<()> {
    use super::true_or_timeout;
    use futures::{stream, StreamExt};
    use hcor::{plant::RubEffect, wormhole::RudeNote::*, Hackstead};
    use log::*;

    // attempt to establish logging, do nothing if it fails
    // (it probably fails because it's already been established in another test)
    drop(pretty_env_logger::try_init());

    let (seed_config, rub_wear_off_config) = hcor::CONFIG
        .seeds()
        .find_map(|(grows_into, seed_config)| {
            Some((
                seed_config,
                hcor::CONFIG.items.keys().find(|c| {
                    RubEffect::item_on_plant(**c, grows_into)
                        .iter()
                        .any(|e| e.duration.is_some())
                })?,
            ))
        })
        .expect("no seeds in config that grow into plants we can rub with effects that wear off?");

    // create bob's stead!
    let bobstead = Hackstead::register().await?;

    // make plant
    info!("spawning first item");
    let seed_item = seed_config.spawn().await?;
    info!("seed item spawn");
    let tile = bobstead.free_tile().expect("new hackstead no open tiles");
    let plant = tile.plant_seed(&seed_item).await?;

    // rub 2 items, simultaneously
    for i in 0..2_usize {
        let rub_item = rub_wear_off_config.spawn().await?;
        let effects = plant.rub_with(&rub_item).await?;

        stream::iter(
            effects
                .clone()
                .into_iter()
                .filter_map(|e| Some((e, e.duration?))),
        )
        .for_each_concurrent(None, |(e, d)| {
            true_or_timeout("effect wear off", d, move |n| match n {
                RubEffectFinish { effect, .. } => return effect.effect_id == e.effect_id,
                _ => false,
            })
        })
        .await;

        info!("effect {} wore off!", i + 1);
    }

    // cleanup
    bobstead.slaughter().await?;

    Ok(())
}
