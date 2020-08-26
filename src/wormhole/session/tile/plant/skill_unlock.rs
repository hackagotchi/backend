use super::SessSend;
use hcor::{
    id,
    plant::{self, skill},
    TileId,
};
use std::fmt;

#[derive(Debug)]
pub enum Error {
    NoSuch(id::NoSuch),
    NoSkill(plant::Conf, skill::Conf),
    NoUnlock(plant::Conf, skill::Conf, usize),
    NoAfford(usize),
}
use Error::*;

impl From<id::NoSuch> for Error {
    fn from(ns: id::NoSuch) -> Error {
        Error::NoSuch(ns)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "couldn't unlock skill for plant: ")?;
        match self {
            NoSuch(ns) => write!(f, "{}", ns),
            NoSkill(plant_conf, skill_conf) => write!(
                f,
                "there's no valid skill with the conf {} on this {}[{}] plant.",
                skill_conf, plant_conf.name, plant_conf
            ),
            NoUnlock(plant_conf, skill_conf, unlock_index) => write!(
                f,
                "there's no valid unlock at the index {} on the skill conf {} on this {}[{}] plant.",
                unlock_index, skill_conf, plant_conf.name, plant_conf
            ),
            NoAfford(0) => write!(f, "You can't afford to unlock this skill."),
            NoAfford(n) => write!(f, "You need {} more skillpoints to unlock this skill", n),
        }
    }
}

pub fn skill_unlock(
    ss: &mut SessSend,
    tile_id: TileId,
    source_skill: skill::Conf,
    unlock_index: usize,
) -> Result<usize, Error> {
    let (plant_conf, plant_id, plant_xp) = {
        let plant = ss.plant(tile_id)?;
        (plant.conf, plant.tile_id, ss.ticker.xp(&*plant) as usize)
    };

    if source_skill.try_lookup().is_none() {
        return Err(NoSkill(plant_conf, source_skill));
    }

    let unlock = source_skill
        .unlocks
        .get(unlock_index)
        .ok_or_else(|| NoUnlock(plant_conf, source_skill, unlock_index))?;

    unlock
        .costs
        .charge(&mut *ss, plant_xp, plant_id)?
        .map_err(|e| NoAfford(e))?;

    let plant = ss.plant_mut(tile_id)?;
    plant.skills.unlocked.push(unlock.skill);
    Ok(plant.skills.available_points(plant_xp))
}

#[cfg(all(feature = "hcor_client", test))]
mod test {
    #[actix_rt::test]
    /// NOTE: relies on plant/new, item/spawn!
    async fn skill_unlock() -> hcor::ClientResult<()> {
        use hcor::{plant, Hackstead};
        use log::*;

        // attempt to establish logging, do nothing if it fails
        // (it probably fails because it's already been established in another test)
        drop(pretty_env_logger::try_init());

        let (_, seed_config) = hcor::CONFIG
            .seeds()
            .next()
            .expect("no seeds in config that grow into plants we can rub with effects?");

        // create bob's stead!
        let bobstead = Hackstead::register().await?;

        // make plant
        let seed_item = seed_config.spawn().await?;
        let tile = bobstead.free_tile().expect("new hackstead no open tiles");
        let plant = tile.plant_seed(&seed_item).await?;
        assert!(plant.skills.available_points(0) == 0);
        let next_skill_unlock = plant
            .skills
            .unlocked
            .iter()
            .flat_map(|s| s.unlocks.iter())
            .find(|s| s.costs == plant::skill::Cost::points(1))
            .expect("no skills I can unlock for just 1 point? D:");

        match next_skill_unlock.unlock_for(&plant).await {
            Ok(uh) => error!(
                "next skill unlock unexpectedly succeeded, skillpoints left: {}",
                uh
            ),
            Err(e) => info!("next skill unlock failed as expected, err: {}", e),
        }

        plant
            .knowledge_snort(
                *plant
                    .skillpoint_unlock_xps
                    .get(1)
                    .expect("no more skillpoint unlock xps?"),
            )
            .await?;

        assert_eq!(
            0,
            next_skill_unlock.unlock_for(&plant).await?,
            "got skill but more than 0 points left",
        );

        bobstead.slaughter().await?;

        Ok(())
    }
}
