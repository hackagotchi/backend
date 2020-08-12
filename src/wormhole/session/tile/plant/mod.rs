use super::{strerr, SessSend};
use hcor::wormhole::{
    AskedNote::{self, *},
    PlantAsk::{self, *},
};

mod summon;
use summon::summon;

mod rub;
use rub::rub;

mod remove;

pub fn handle_ask(ss: &mut SessSend, ask: PlantAsk) -> AskedNote {
    match ask {
        Summon {
            tile_id,
            seed_item_id,
        } => PlantSummonResult(strerr(summon(ss, tile_id, seed_item_id))),
        Slaughter { tile_id } => {
            PlantSlaughterResult(strerr(ss.steddit(move |hs| hs.take_plant(tile_id))))
        }
        Craft {
            tile_id,
            recipe_index,
        } => PlantSlaughterResult(strerr(Err("unimplemented route"))),
        Rub {
            tile_id,
            rub_item_id,
        } => PlantRubStartResult(strerr(rub(ss, tile_id, rub_item_id))),
    }
}
/*
mod craft;
*/

/*
#[actix_rt::test]
/// NOTE: relies on plant/new, item/spawn, plant/rub!
async fn plant_craft() -> hcor::ClientResult<()> {
    use hcor::{Hackstead, Plant};

    // attempt to establish logging, do nothing if it fails
    // (it probably fails because it's already been established in another test)
    drop(pretty_env_logger::try_init());

    let seed_arch = hcor::CONFIG
        .possession_archetypes
        .iter()
        .find(|a| a.seed.is_some())
        .expect("no items in config that are seeds?");

    // create bob's stead!
    let mut bobstead = Hackstead::register().await?;
    let seed_item = seed_arch.spawn().await?;
    let tile = bobstead
        .free_tile()
        .expect("new hackstead no open tiles");
    let plant = tile.plant_seed(&seed_item).await?;

    bobstead.slaughter();
}*/
