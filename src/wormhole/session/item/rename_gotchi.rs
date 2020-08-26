use super::SessSend;
use hcor::id::ItemId;
use hcor::{id, item, Item};
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
        write!(f, "couldn't spawn items: ")?;
        match self {
            NoSuch(e) => write!(f, "no such item: {}", e),
        }
    }
}

pub fn rename(ss: &mut SessSend, item_id: ItemId, new_name: String) -> Result<String, Error> {
    let item = ss.item_mut(item_id)?;
    let gotchi = item.gotchi_mut()?;
    gotchi.nickname = new_name.clone();
    Ok(new_name.clone())
}

#[cfg(all(test, features = "hcor_client"))]
mod test {
    #[actix_rt::test]
    /// NOTE: requires that at least one item exists in the config!
    async fn spawn() -> hcor::ClientResult<()> {
        use super::test::{ITEM_ARCHETYPE, ITEM_SPAWN_COUNT};
        use hcor::Hackstead;
        use log::*;

        // attempt to establish logging, do nothing if it fails
        // (it probably fails because it's already been established in another test)
        drop(pretty_env_logger::try_init());

        debug!("create bob's stead");
        let mut bobstead = Hackstead::register().await?;

        // we'll need to keep track of how many items we have to see if spawning works.
        fn count_relevant_items(hackstead: &Hackstead) -> usize {
            hackstead
                .inventory
                .iter()
                .filter(|i| i.archetype_handle == ITEM_ARCHETYPE)
                .count()
        }
        let starting_item_count = count_relevant_items(&bobstead);

        debug!("spawn bob some items and refresh his stead");
        let items = bobstead
            .spawn_items(ITEM_ARCHETYPE, ITEM_SPAWN_COUNT)
            .await?;
        debug!("Rename the first one");
        bobstead = Hackstead::fetch(&bobstead).await?;

        debug!("make sure those new items are in there");
        assert_eq!(
            count_relevant_items(&bobstead) - starting_item_count,
            ITEM_SPAWN_COUNT
        );

        debug!("make sure each of the items the API says we got are in bob's inventory.");
        for item in items {
            assert!(
                bobstead.inventory.contains(&item),
                "bobstead did not contain spawned item: \nitem: {:#?}\ninventory: {:#?}",
                item,
                bobstead.inventory
            );
        }

        debug!("kill bob so he's not left in the database");
        bobstead.slaughter().await?;

        Ok(())
    }
}
