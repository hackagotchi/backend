use super::SessSend;
use hcor::{item, Item};
use std::fmt;

#[derive(Debug)]
pub enum Error {
    NoSuchItemConf(item::Conf),
}
use Error::*;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "couldn't spawn items: ")?;
        match self {
            NoSuchItemConf(e) => write!(f, "no such item conf: {}", e),
        }
    }
}

pub fn spawn(ss: &mut SessSend, item_conf: item::Conf, amount: usize) -> Result<Vec<Item>, Error> {
    if item_conf.try_lookup().is_none() {
        return Err(NoSuchItemConf(item_conf));
    }

    let items: Vec<Item> = (0..amount)
        .map(|_| {
            Item::from_conf(
                item_conf,
                ss.profile.steader_id,
                item::Acquisition::spawned(),
            )
        })
        .collect();

    ss.inventory.append(&mut items.clone());
    Ok(items)
}

#[cfg(all(test, features = "hcor_client"))]
mod test {
    #[actix_rt::test]
    /// NOTE: requires that at least one item exists in the config!
    async fn spawn() -> hcor::ClientResult<()> {
        const ITEM_SPAWN_COUNT: _ = 10;
        let item_conf = *hcor::CONFIG.items().keys().next().unwrap();
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
                .filter(|i| i.conf == item_conf)
                .count()
        }
        let starting_item_count = count_relevant_items(&bobstead);

        debug!("spawn bob some items and refresh his stead");
        let items = bobstead.spawn_items(ITEM_CONF, ITEM_SPAWN_COUNT).await?;
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
