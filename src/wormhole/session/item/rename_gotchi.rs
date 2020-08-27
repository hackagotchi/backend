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

        let (_, seed_arch) = hcor::CONFIG
            .seeds()
            .next()
            .expect("No items in config that are seeds?");

        debug!("create bob's stead");
        let mut bobstead = Hackstead::register().await?;
        debug!("spawn bob some items and refresh his stead");

        let seed_item = seed_arch.spawn().await?;
        let open_tile = bobstead.free_tile().expect("New hackstead no open land?");

        let plant = open_tile.plant_seed(seed_item).await?;
        plant.rename("bob's plant jr").await?;

        //Refresh stead
        bobstead = Hackstead::fetch(&bobstead).await?;

        assert_eq!(bobstead.plant(&plant).name, "bob's plant jr");

        debug!("kill bob so he's not left in the database");
        bobstead.slaughter().await?;

        Ok(())
    }
}
