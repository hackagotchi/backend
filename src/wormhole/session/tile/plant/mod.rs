use super::{strerr, SessSend};
use hcor::wormhole::{
    AskedNote::{self, *},
    PlantAsk::{self, *},
};

mod summon;
use summon::summon;

mod rub;
use rub::rub;

mod skill_unlock;
use skill_unlock::skill_unlock;

mod slaughter;

pub fn handle_ask(ss: &mut SessSend, ask: PlantAsk) -> AskedNote {
    match ask {
        Summon {
            tile_id,
            seed_item_id,
        } => PlantSummonResult(strerr(summon(ss, tile_id, seed_item_id))),
        Slaughter { tile_id } => PlantSlaughterResult(strerr(ss.take_plant(tile_id))),
        KnowledgeSnort { tile_id, xp } => PlantKnowledgeSnortResult(strerr({
            ss.ticker
                .increase_xp(tile_id, xp as f32)
                .map(|_| ss.ticker.xp(tile_id) as usize)
                .map_err(|_| format!("couldn't knowledge snort, no such plant, {}", tile_id))
        })),
        Rub {
            tile_id,
            rub_item_id,
        } => PlantRubStartResult(strerr(rub(ss, tile_id, rub_item_id))),
        Craft { .. } => PlantCraftStartResult(strerr(Err("unimplemented route"))),
        Nickname { .. } => PlantNicknameResult(strerr(Err("unimplemented route"))),
        SkillUnlock {
            tile_id,
            source_skill_conf,
            unlock_index,
        } => PlantSkillUnlockResult(strerr(skill_unlock(
            ss,
            tile_id,
            source_skill_conf,
            unlock_index,
        ))),
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

    let seed_config = hcor::CONFIG
        .possession_configetypes
        .iter()
        .find(|a| a.seed.is_some())
        .expect("no items in config that are seeds?");

    // create bob's stead!
    let mut bobstead = Hackstead::register().await?;
    let seed_item = seed_config.spawn().await?;
    let tile = bobstead
        .free_tile()
        .expect("new hackstead no open tiles");
    let plant = tile.plant_seed(&seed_item).await?;

    bobstead.slaughter();
}*/
