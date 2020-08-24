use super::SessSend;
use hcor::{id, plant, Item, ItemId, Plant, TileId};
use log::*;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    NoSuch(id::NoSuch),
    NotConfigured(Item),
    AlreadyOccupied(TileId, Plant),
}
use Error::*;

impl From<id::NoSuch> for Error {
    fn from(ns: id::NoSuch) -> Error {
        Error::NoSuch(ns)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "couldn't make new plant from seed: ")?;
        match self {
            NoSuch(ns) => write!(f, "{}", ns),
            NotConfigured(item) => write!(
                f,
                "item {}[{}] is not configured to be used as a seed",
                item.name, item.conf,
            ),
            AlreadyOccupied(tile_id, plant) => write!(
                f,
                "tile {} is already occupied by a {}[{}] plant.",
                tile_id, plant.name, plant.conf
            ),
        }
    }
}

pub fn summon(ss: &mut SessSend, tile_id: TileId, item_id: ItemId) -> Result<Plant, Error> {
    let item = ss.take_item(item_id)?;
    let seed = item.grows_into.ok_or_else(|| NotConfigured(item.clone()))?;

    let plant = Plant::from_conf(item.owner_id, tile_id, seed);
    if let Some(until_finish) = plant.base_yield_duration {
        trace!("adding yield timer");
        ss.set_timer(plant::Timer {
            until_finish,
            tile_id,
            lifecycle: plant::timer::Lifecycle::Perennial {
                duration: until_finish,
            },
            kind: plant::TimerKind::Yield,
        })
    }

    let tile = ss.tile_mut(tile_id)?;
    if let Some(plant) = tile.plant.as_ref() {
        return Err(AlreadyOccupied(tile_id, plant.clone()));
    }

    tile.plant = Some(plant.clone());
    Ok(plant)
}

#[cfg(all(feature = "hcor_client", test))]
mod test {
    #[actix_rt::test]
    /// NOTE: relies on item/spawn!
    async fn summon() -> hcor::ClientResult<()> {
        use hcor::{Hackstead, Item, Tile};
        use log::*;

        // attempt to establish logging, do nothing if it fails
        // (it probably fails because it's already been established in another test)
        drop(pretty_env_logger::try_init());

        let (_, seed_config) = hcor::CONFIG
            .seeds()
            .next()
            .expect("no items in config that are seeds?");
        let not_seed_config = hcor::CONFIG
            .items
            .values()
            .find(|a| a.grows_into.is_none())
            .expect("no items in config that aren't seeds?");

        // create bob's stead!
        let mut bobstead = Hackstead::register().await?;

        let seed_item = seed_config.spawn().await?;
        let not_seed_item = not_seed_config.spawn().await?;
        let open_tile = bobstead.free_tile().expect("fresh hackstead no open land?");

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
                        plant.tile_id, tile.tile_id,
                        "plant planted for bob is on a different tile than expected: {:#?}",
                        plant
                    );
                    assert_eq!(
                        seed_item.grows_into.unwrap(),
                        plant.conf,
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

        // kill bob so he's not left
        bobstead.slaughter().await?;

        Ok(())
    }
}
