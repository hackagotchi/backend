use super::SessSend;
use hcor::{id, item, Item, ItemId};
use std::fmt;

#[derive(Debug)]
pub enum Error {
    NoSuch(id::NoSuch),
    NotConfigured(Item),
}
use Error::*;

impl From<id::NoSuch> for Error {
    fn from(ns: id::NoSuch) -> Error {
        Error::NoSuch(ns)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "couldn't spawn items: ")?;
        match self {
            NoSuch(e) => write!(f, "{}", e),
            NotConfigured(i) => write!(
                f,
                "provided item {}, which, as a {}[{}], is not configured to be hatched",
                i.item_id, i.name, i.archetype_handle
            ),
        }
    }
}

pub fn hatch(ss: &mut SessSend, item_id: ItemId) -> Result<Vec<Item>, Error> {
    let item = ss.steddit(move |hs| hs.take_item(item_id))?;
    let hatch_table = item
        .hatch_table
        .as_ref()
        .ok_or_else(|| NotConfigured(item.clone()))?;

    let items = hcor::config::spawn(&hatch_table, &mut rand::thread_rng())
        .map(|item_name| {
            Item::from_archetype(
                hcor::CONFIG.find_possession(&item_name)?,
                item.owner_id,
                item::Acquisition::Hatched,
            )
        })
        .collect::<Result<Vec<Item>, hcor::ConfigError>>()
        .unwrap_or_else(|e| {
            panic!("hatch table produced: {}", e);
        });

    ss.steddit(move |hs| {
        hs.inventory.append(&mut items.clone());
        Ok(items.clone())
    })
}

#[cfg(all(test, feature = "hcor_client"))]
mod test {
    #[actix_rt::test]
    /// NOTE: requires that hatchable and unhatchable items exist in the config!
    /// relies on item/spawn!
    async fn hatch() -> hcor::ClientResult<()> {
        use hcor::Hackstead;
        use log::*;

        // attempt to establish logging, do nothing if it fails
        // (it probably fails because it's already been established in another test)
        drop(pretty_env_logger::try_init());

        let mut bobstead = Hackstead::register().await?;

        debug!("finding prequisites in config...");
        let unhatchable_arch = hcor::CONFIG
            .possession_archetypes
            .iter()
            .find(|x| x.hatch_table.is_none())
            .expect("no unhatchable items in config?");
        let hatchable_arch = hcor::CONFIG
            .possession_archetypes
            .iter()
            .find(|x| x.hatch_table.is_some())
            .expect("no unhatchable items in config?");

        debug!("to prepare, we need to spawn bob hatchable and unhatchable items.");
        let unhatchable_item = unhatchable_arch.spawn().await?;
        let hatchable_item = hatchable_arch.spawn().await?;
        bobstead = Hackstead::fetch(&bobstead).await?;

        debug!("let's start off by hatching the unhatchable and making sure that doesn't work.");
        match unhatchable_item.hatch().await {
            Ok(items) => panic!("unhatchable item unexpectedly hatched into: {:#?}", items),
            Err(e) => info!(
                "received error as expected upon attempting to hatch unhatchable item: {}",
                e
            ),
        }
        assert_eq!(
            bobstead.inventory.len(),
            Hackstead::fetch(&bobstead).await?.inventory.len(),
            "failing to hatch modified inventory item count somehow",
        );

        debug!("great, now let's try actually hatching something hatchable!");
        let hatched_items = hatchable_item.hatch().await?;

        debug!(
            "let's make sure bob's inventory grew proportionally \
          to the amount of items hatching produced"
        );
        let starting_inventory = bobstead.inventory.clone();
        let new_inventory = Hackstead::fetch(&bobstead).await?.inventory;
        assert_eq!(
            hatched_items.len(),
            (new_inventory.len() - (starting_inventory.len() - 1)),
            "the number of items in bob's inventory changed differently \
            than the number of items produced by hatching this item, somehow. \
            starting inventory: {:#?}\nitems hatched: {:#?}\nnew inventory: {:#?}",
            starting_inventory,
            hatched_items,
            new_inventory,
        );

        debug!("okay, but can we hatch the already hatched item?");
        match hatchable_item.hatch().await {
            Ok(items) => panic!(
                "pretty sure I ain't supposed to be able \
                to hatch this twice, got: {:#?}",
                items
            ),
            Err(e) => info!(
                "got error as expected from hatching already hatched item: {}",
                e
            ),
        }

        debug!("kill bob so he's not left in the database");
        bobstead.slaughter().await?;

        Ok(())
    }
}
