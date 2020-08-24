use super::SessSend;
use hcor::{
    id,
    plant::{self, RubEffect},
    Item, ItemId, Plant, TileId,
};
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
                item.name, item.conf
            ),
            NoEffect(Some(plant), item) => write!(
                f,
                "rubbing item {}[{}] on plant {}[{}] would have no effects",
                item.name, item.conf, plant.name, plant.conf
            ),
        }
    }
}

pub fn rub(ss: &mut SessSend, tile_id: TileId, item_id: ItemId) -> Result<Vec<RubEffect>, Error> {
    let item = ss.take_item(item_id)?;
    if item.conf.plant_rub_effects.is_empty() {
        return Err(NoEffect(None, item));
    }

    let plant = ss.plant(tile_id)?;

    let effects = RubEffect::item_on_plant(item.conf, plant.conf);
    if effects.is_empty() {
        return Err(NoEffect(Some(plant.clone()), item.clone()));
    }

    for (until_finish, effect_id) in effects
        .iter()
        .filter_map(|e| Some((e.duration?, e.effect_id)))
    {
        // register any timers we'll need for the effects that'll wear off
        ss.set_timer(plant::Timer {
            until_finish,
            tile_id,
            lifecycle: plant::timer::Lifecycle::Annual,
            kind: plant::TimerKind::Rub {
                effect_id: effect_id,
            },
        })
    }

    ss.plant_mut(tile_id)?
        .rub_effects
        .append(&mut effects.clone());
    Ok(effects)
}

#[cfg(all(feature = "hcor_client", test))]
mod test {
    #[actix_rt::test]
    /// NOTE: relies on plant/new, item/spawn!
    async fn rub() -> hcor::ClientResult<()> {
        use hcor::{plant::RubEffect, Hackstead};

        // attempt to establish logging, do nothing if it fails
        // (it probably fails because it's already been established in another test)
        drop(pretty_env_logger::try_init());

        let (seed_config, rub_config) = hcor::CONFIG
            .seeds()
            .find_map(|(grows_into, seed_config)| {
                Some((
                    seed_config,
                    hcor::CONFIG
                        .items
                        .keys()
                        .find(|c| !RubEffect::item_on_plant(**c, grows_into).is_empty())?,
                ))
            })
            .expect("no seeds in config that grow into plants we can rub with effects?");

        // create bob's stead!
        let mut bobstead = Hackstead::register().await?;

        // make plant
        let seed_item = seed_config.spawn().await?;
        let tile = bobstead.free_tile().expect("new hackstead no open tiles");
        let mut plant = tile.plant_seed(&seed_item).await?;

        // rub item
        let rub_item = rub_config.spawn().await?;
        let effects = plant.rub_with(&rub_item).await?;

        bobstead = Hackstead::fetch(&bobstead).await?;
        plant = bobstead.plant(&plant).unwrap().clone();
        assert_eq!(
            plant.rub_effects, effects,
            "brand new plant has more effects than those from the item that was just rubbed on",
        );
        assert!(
            RubEffect::item_on_plant(rub_item.conf, plant.conf)
                .iter()
                .all(|a| effects
                    .iter()
                    .any(|b| { a.item_conf == b.item_conf && a.effect_index == b.effect_index })),
            "the effects of this item we just rubbed on can't be found on this plant"
        );

        bobstead.slaughter().await?;

        Ok(())
    }
}
