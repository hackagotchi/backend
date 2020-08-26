use super::SessSend;
use hcor::{
    id,
    plant::{ServerTimer, SharedTimer, TimerKind},
    wormhole::RudeNote,
};
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
        write!(f, "couldn't finish timer: ")?;

        match &self {
            NoSuch(ns) => write!(f, "{}", ns),
        }
    }
}

pub fn finish_timer(
    ss: &mut SessSend,
    SharedTimer { tile_id, kind, .. }: SharedTimer,
    _: ServerTimer,
) -> Result<Option<RudeNote>, Error> {
    use TimerKind::*;

    let plant = ss.plant_mut(tile_id)?;
    Ok(Some(match kind {
        Yield => RudeNote::YieldFinish {
            output: Default::default(),
            tile_id,
        },
        Craft { recipe_index } => RudeNote::CraftFinish {
            output: Default::default(),
            tile_id,
        },
        Rub { effect_id } => RudeNote::RubEffectFinish {
            effect: plant.take_rub_effect(effect_id)?,
            tile_id,
        },
        Xp => return Ok(None),
    }))
}
