use super::SessSend;
use hcor::{id, plant, Item, ItemId, Plant, TileId};
use log::*;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    NoSuch(id::NoSuch),
}
use Error::*;

impl From<id::NoSuch> for Error {
    fn from(ns: id::NoSuch) -> Error {
        Error::NoSuch(ns)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "couldn't rename plant: ")?;
        match self {
            NoSuch(ns) => write!(f, "{}", ns),
            
        }
    }
}

pub fn nickname(ss: &mut SessSend, tile_id: TileId, new_name: String) -> Result<String, Error> {

    let plant = ss.plant_mut(tile_id)?;
    plant.nickname = new_name.clone(); 

   Ok(new_name.clone())
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

        let seed_item = seed_arch.spawn().await?;
        let not_seed_item = not_seed_arch.spawn().await?;
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

        // kill bob so he's not left
        bobstead.slaughter().await?;

        Ok(())
    }
}
