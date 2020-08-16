use super::SessionContext;
use hcor::{
    id,
    plant::{Timer, TimerKind},
    wormhole::RudeNote,
    Hackstead,
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
    hs: &mut Hackstead,
    _: &mut SessionContext,
    Timer { tile_id, kind, .. }: Timer,
) -> Result<RudeNote, Error> {
    use TimerKind::*;

    let plant = hs.plant_mut(tile_id)?;
    Ok(match kind {
        Yield => RudeNote::YieldFinish {
            items: vec![],
            xp: 0,
            tile_id,
        },
        Craft { recipe_index } => RudeNote::CraftFinish {
            items: vec![],
            xp: 0,
            tile_id,
        },
        Rub { effect_id } => RudeNote::RubEffectFinish {
            effect: plant.take_effect(effect_id)?,
            tile_id,
        },
    })
}
