use super::SessSend;
use hcor::{config::evalput, id, item, Item, ItemId};
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
                i.item_id, i.name, i.conf
            ),
        }
    }
}

pub fn hatch(ss: &mut SessSend, item_id: ItemId) -> Result<evalput::Output<Item>, Error> {
    let item = ss.take_item(item_id)?;
    let hatch_table = item
        .hatch_table
        .as_ref()
        .ok_or_else(|| NotConfigured(item.clone()))?;

    let output = hatch_table
        .evaluated(&mut rand::thread_rng())
        .spawned(item.owner_id, item::Acquisition::Hatched);

    output.copy_into(&mut *ss);

    Ok(output)
}

#[cfg(all(test, feature = "hcor_client"))]
mod test {
    #[actix_rt::test]
    /// NOTE: requires that hatchable and unhatchable items exist in the config!
    /// relies on item/spawn!
    async fn hatch() -> hcor::ClientResult<()> {
        use hcor::Hackstead;
        use log::*;
        use tokio::time::{delay_for, Duration};

        // attempt to establish logging, do nothing if it fails
        // (it probably fails because it's already been established in another test)
        drop(pretty_env_logger::try_init());

        let mut bobstead = Hackstead::register().await?;

        debug!("finding prequisites in config...");
        let unhatchable_config = hcor::CONFIG
            .items
            .values()
            .find(|x| x.hatch_table.is_none())
            .expect("no unhatchable items in config?");
        let hatchable_config = hcor::CONFIG
            .items
            .values()
            .find(|x| x.hatch_table.is_some())
            .expect("no unhatchable items in config?");

        debug!("to prepare, we need to spawn bob hatchable and unhatchable items.");
        let unhatchable_item = unhatchable_config.spawn().await?;
        let hatchable_item = hatchable_config.spawn().await?;
        bobstead.server_sync().await?;

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
            {
                delay_for(Duration::from_millis(400)).await;
                bobstead.server_sync().await?;
                bobstead.inventory.len()
            },
            "failing to hatch modified inventory item count somehow",
        );

        debug!(
            "great, now let's try actually hatching something hatchable ({})!",
            hatchable_item.name
        );
        let hatch_output = hatchable_item.hatch().await?;

        debug!(
            "let's make sure bob's inventory grew proportionally \
          to the amount of items hatching produced"
        );
        let starting_inventory = bobstead.inventory.clone();

        delay_for(Duration::from_millis(400)).await;
        bobstead.server_sync().await?;
        let new_inventory = bobstead.inventory.clone();
        assert_eq!(
            hatch_output.items.len(),
            (new_inventory.len() - (starting_inventory.len() - 1)),
            "the number of items in bob's inventory changed differently \
            than the number of items produced by hatching this item, somehow. \
            starting inventory: {:#?}\nitems hatched: {:#?}\nnew inventory: {:#?}",
            starting_inventory,
            hatch_output.items,
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
