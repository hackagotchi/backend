#[actix_rt::test]
/// NOTE: relies on plant/new, item/spawn, plant/apply!
async fn plant_rub_wear_off() -> hcor::ClientResult<()> {
    use log::*;
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
        .expect("no seeds in config that grow into plants we can rub with effects that wear off?");

    // create bob's stead!
    let bobstead = Hackstead::register().await?;

    // make plant
    let seed_item = seed_arch.spawn_for(&bobstead).await?;
    let tile = bobstead.free_tile().expect("new hackstead no open tiles");
    let plant = tile.plant_seed(&seed_item).await?;

    // rub item
    let rub_item = rub_wear_off_arch.spawn_for(&bobstead).await?;
    let effects = plant.rub_with(&rub_item).await?;

    let wormhole = hcor::Wormhole::new(&bobstead).await?;
    let wh = &wormhole;

    stream::iter(effects.into_iter().filter(|e| e.duration.is_some()))
        .for_each_concurrent(None, |e| async move {
            const ERR_MARGIN_SECS: f64 = 1.0;

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

    // cleanup
    wormhole.disconnect();
    bobstead.slaughter().await?;

    Ok(())
}
