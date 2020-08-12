use super::{strerr, SessSend};
use hcor::{id, Item, ItemId, Tile};
use std::fmt;

pub mod plant;

#[derive(Debug)]
pub enum Error {
    NoSuch(id::NoSuch),
    NotConfigured(Item),
    Ineligible,
}
use Error::*;

impl From<id::NoSuch> for Error {
    fn from(ns: id::NoSuch) -> Error {
        Error::NoSuch(ns)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "couldn't provide new tile: ")?;
        match self {
            NoSuch(ns) => write!(f, "{}", ns),
            NotConfigured(item) => write!(
                f,
                "item {}[{}] is not configured to unlock land",
                item.name, item.archetype_handle,
            ),
            Ineligible => write!(f, "you aren't eligible to unlock more land."),
        }
    }
}

pub fn summon(ss: &mut SessSend, item_id: ItemId) -> Result<Tile, Error> {
    let item = ss.hackstead.item(item_id)?;
    let land_unlock = item
        .unlocks_land
        .as_ref()
        .ok_or_else(|| NotConfigured(item.clone()))?;

    if !land_unlock.requires_xp {
        ss.steddit(|hs| hs.profile.extra_land_plot_count += 1);
    }

    if ss.hackstead.land_unlock_eligible() {
        let tile = Tile::new(ss.hackstead.profile.steader_id);

        ss.steddit(move |hs| {
            hs.take_item(item_id)?;
            hs.land.push(tile.clone());
            Ok(tile.clone())
        })
    } else {
        Err(Ineligible)
    }
}

#[cfg(all(feature = "hcor_client", test))]
mod test {
    #[actix_rt::test]
    /// NOTE: relies on item/spawn!
    async fn summon() -> hcor::ClientResult<()> {
        use hcor::Hackstead;
        use log::*;

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

        debug!("create bob's stead!");
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
                    tile.owner_id, bobstead.profile.steader_id,
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

        debug!("spawn bob an item he can redeem for a tile if he has enough xp");
        let requires_xp_item = requires_xp_arch.spawn().await?;

        debug!("try and redeem this item bob doesn't have enough xp to redeem for land");
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

        debug!("spawn an item bob can redeem for land without having enough xp");
        let no_requires_xp_item = no_requires_xp_arch.spawn().await?;

        debug!("try and redeem that item, this should actually work");
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
        let now_xp = bobstead.knowledge_snort(std::usize::MAX).await?;
        debug!("bob's xp now set to {}", now_xp);

        debug!("try and redeem the first item that does require xp to work, should work now.");
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

        debug!("try and redeem those items we've already used up");
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

        debug!("try to redeem the non-land-redeemable item for land");
        let non_land_redeemable_item = non_land_redeemable_arch.spawn().await?;
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

        debug!("kill bob so he's not left in the database");
        bobstead.slaughter().await?;

        Ok(())
    }
}
