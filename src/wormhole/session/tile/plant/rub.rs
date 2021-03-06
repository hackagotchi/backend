use super::SessSend;
use hcor::{id, plant, Item, ItemId, Plant, TileId};
use std::fmt;

#[derive(Debug)]
pub enum Error {
    NoSuch(id::NoSuch),
    NoEffect(Option<Plant>, Item),
}
use Error::*;

impl From<id::NoSuch> for Error {
    fn from(ns: id::NoSuch) -> Error {
        Error::NoSuch(ns)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "couldn't rub item on plant: ")?;
        match self {
            NoSuch(ns) => write!(f, "{}", ns),
            NoEffect(None, item) => write!(
                f,
                "item {}[{}] isn't configured to impart any effects when rubbed on plants",
                item.name, item.archetype_handle
            ),
            NoEffect(Some(plant), item) => write!(
                f,
                "rubbing item {}[{}] on plant {}[{}] would have no effects",
                item.name, item.archetype_handle, plant.name, plant.archetype_handle
            ),
        }
    }
}

pub fn rub(
    ss: &mut SessSend,
    tile_id: TileId,
    item_id: ItemId,
) -> Result<Vec<plant::Effect>, Error> {
    let item = ss.take_item(item_id)?;
    if item.plant_rub_effects.is_empty() {
        return Err(NoEffect(None, item));
    }

    let plant = ss.plant(tile_id)?;
    let plant_name = plant.name.clone();

    let mut effect_confs = item.rub_effects_for_plant_indexed(&plant_name).peekable();
    if effect_confs.peek().is_none() {
        return Err(NoEffect(Some(plant.clone()), item.clone()));
    }

    let effects: Vec<plant::Effect> = effect_confs
        .into_iter()
        .map(|(i, a)| {
            let effect_id = plant::EffectId(uuid::Uuid::new_v4());

            // register any timers we'll need for the effects that'll wear off
            if let Some(until_finish) = a.duration {
                ss.set_timer(plant::Timer {
                    until_finish,
                    tile_id,
                    lifecycle: plant::timer::Lifecycle::Annual,
                    kind: plant::TimerKind::Rub { effect_id },
                })
            }

            plant::Effect {
                effect_id,
                item_archetype_handle: item.archetype_handle,
                effect_archetype_handle: i,
            }
        })
        .collect();

    ss.plant_mut(tile_id)?.effects.append(&mut effects.clone());
    Ok(effects)
}

#[cfg(all(feature = "hcor_client", test))]
mod test {
    #[actix_rt::test]
    /// NOTE: relies on plant/new, item/spawn!
    async fn rub() -> hcor::ClientResult<()> {
        use hcor::Hackstead;

        // attempt to establish logging, do nothing if it fails
        // (it probably fails because it's already been established in another test)
        drop(pretty_env_logger::try_init());

        let (seed_arch, rub_arch) = hcor::CONFIG
            .seeds()
            .find_map(|(seed, seed_arch)| {
                Some((
                    seed_arch,
                    hcor::CONFIG
                        .possession_archetypes
                        .iter()
                        .find(|a| a.rub_effects_for_plant(&seed.grows_into).count() > 0)?,
                ))
            })
            .expect("no seeds in config that grow into plants we can rub with effects?");

        // create bob's stead!
        let mut bobstead = Hackstead::register().await?;

        // make plant
        let seed_item = seed_arch.spawn().await?;
        let tile = bobstead.free_tile().expect("new hackstead no open tiles");
        let mut plant = tile.plant_seed(&seed_item).await?;

        // rub item
        let rub_item = rub_arch.spawn().await?;
        let effects = plant.rub_with(&rub_item).await?;

        bobstead = Hackstead::fetch(&bobstead).await?;
        plant = bobstead.plant(&plant).unwrap().clone();
        assert_eq!(
            plant.effects, effects,
            "brand new plant has more effects than those from the item that was just rubbed on",
        );
        assert!(
            rub_arch
                .rub_effects_for_plant(&plant.name)
                .enumerate()
                .all(|(i, _)| {
                    effects
                        .iter()
                        .any(|e| e.effect_archetype_handle == i as hcor::config::ArchetypeHandle)
                }),
            "the effects of this item we just rubbed on can't be found on this plant"
        );

        bobstead.slaughter().await?;

        Ok(())
    }
}
