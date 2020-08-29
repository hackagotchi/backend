use super::Orifice;
use hcor::serde_diff::Diff;
use hcor::{wormhole::EditNote, Hackstead};
use std::fmt;

/// We need all changes to a Hackstead to be also sent to the client;
/// to insure that we do not mutate the hackstead without also sending changes to the client,
/// we have this HacksteadGuard struct.
pub struct HacksteadGuard {
    hackstead: Hackstead,
}

#[derive(Debug)]
pub enum DiffError {
    Json(serde_json::Error),
    Bincode(bincode::Error),
    Patching(std::io::Error),
}
type DiffResult<T> = Result<T, DiffError>;
impl From<bincode::Error> for DiffError {
    fn from(e: bincode::Error) -> DiffError {
        DiffError::Bincode(e)
    }
}
impl From<serde_json::Error> for DiffError {
    fn from(e: serde_json::Error) -> DiffError {
        DiffError::Json(e)
    }
}
impl From<std::io::Error> for DiffError {
    fn from(e: std::io::Error) -> DiffError {
        DiffError::Patching(e)
    }
}
impl fmt::Display for DiffError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "couldn't create diff for EditNote: ")?;

        match self {
            DiffError::Bincode(e) => {
                write!(f, "couldn't serialize/deserialize hackstead bincode: {}", e)
            }
            DiffError::Json(e) => write!(f, "couldn't serialize/deserialize hackstead json: {}", e),
            DiffError::Patching(e) => write!(f, "couldn't create bincode binary patch: {}", e),
        }
    }
}

impl HacksteadGuard {
    pub fn new(hs: Hackstead) -> Self {
        HacksteadGuard {
            hackstead: hs.clone(),
        }
    }

    fn json_diff(&mut self, new: &Hackstead) -> DiffResult<String> {
        Ok(serde_json::to_string(&Diff::serializable(
            &self.hackstead,
            &new,
        ))?)
    }

    fn bincode_diff(&mut self, new: &Hackstead) -> DiffResult<Vec<u8>> {
        let old_bincode = bincode::serialize(&self.hackstead)?;
        let new_bincode = bincode::serialize(&new)?;

        let mut diff_data = vec![];
        hcor::bidiff::simple_diff(&old_bincode, &new_bincode, &mut diff_data)?;

        Ok(diff_data)
    }

    pub fn set(&mut self, new: Hackstead, orifice: Orifice) -> DiffResult<EditNote> {
        let note = match orifice {
            Orifice::Json => EditNote::Json(self.json_diff(&new)?),
            Orifice::Bincode => EditNote::Bincode(self.bincode_diff(&new)?),
        };

        self.hackstead = new;
        Ok(note)
    }
}

/*
#[cfg(test)]
fn hs_with_rubbed_plant() -> Hackstead {
    use hcor::{plant, Plant};

    let mut hs = Hackstead::new_user(Some(""));
    let tile_id = hs.free_tile().unwrap().tile_id;
    let conf = hcor::CONFIG.seeds().next().unwrap().0;
    let mut plant = Plant::from_conf(hs.profile.steader_id, tile_id, conf);
    plant.rub_effects.extend(plant::RubEffect::item_on_plant(
        hcor::CONFIG.item_named("Warp Powder").unwrap().conf,
        conf,
    ));
    hs.tile_mut(tile_id).unwrap().plant = Some(plant);
    hs
}

#[test]
fn rub_effect_diff() {
    use bincode::Options;
    use hcor::serde_diff::Apply;

    let mut old = hs_with_rubbed_plant();

    let mut new = old.clone();
    new.plants_mut().next().unwrap().rub_effects.pop().unwrap();

    let diff: Diff<Hackstead> = Diff::serializable(&old, &new);

    let json = serde_json::to_string_pretty(&diff).unwrap();
    println!("{}", json);

    let bincode_data = bincode::serialize(&diff).unwrap();
    bincode::options()
        .deserialize_seed(Apply::deserializable(&mut old), &bincode_data)
        .unwrap();
}

#[test]
fn tile_diff() {
    use bincode::Options;
    use hcor::serde_diff::Apply;

    let mut old = Hackstead::new_user(Some(""));

    let mut new = old.clone();
    new.land.pop().unwrap();

    let diff: Diff<Hackstead> = Diff::serializable(&old, &new);

    {
        let json = serde_json::to_string_pretty(&diff).unwrap();
        println!("{}", json);
    }

    let bincode_data = bincode::serialize(&diff).unwrap();

    {
        let diff: Diff<Hackstead> = bincode::deserialize(&bincode_data).unwrap();
        let json = serde_json::to_string_pretty(&diff).unwrap();
        println!("{}", json);
    }

    bincode::options()
        .deserialize_seed(Apply::deserializable(&mut old), &bincode_data)
        .unwrap();
}*/

impl std::ops::Deref for HacksteadGuard {
    type Target = Hackstead;

    fn deref(&self) -> &Self::Target {
        &self.hackstead
    }
}
