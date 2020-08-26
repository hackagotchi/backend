use hcor::serde_diff::Diff;
use hcor::Hackstead;

/// We need all changes to a Hackstead to be also sent to the client;
/// to insure that we do not mutate the hackstead without also sending changes to the client,
/// we have this HacksteadGuard struct.
pub struct HacksteadGuard {
    hackstead: Hackstead,
    old: Hackstead,
}

impl HacksteadGuard {
    pub fn new(hs: Hackstead) -> Self {
        HacksteadGuard {
            hackstead: hs.clone(),
            old: hs,
        }
    }

    pub fn apply(&mut self, new: Hackstead) -> (bool, Diff<Hackstead>) {
        self.old = self.hackstead.clone();
        self.hackstead = new;

        (
            self.old != self.hackstead,
            Diff::serializable(&self.old, &self.hackstead),
        )
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
