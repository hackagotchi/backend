//! Dumps a CSV file of the old dynamodb format into the db.
//! input CSVs are assumed to be generated with this tool: https://pypi.org/project/export-dynamodb/
use hcor::{item, ItemId, TileId};
use serde::{Deserialize, Serialize};

/* To make the errors bearable.
 * This may need to be updated.

0  | cat,
1  | id,
2  | name,
3  | price,
4  | steader,
5  | market_name,
6  | ownership_log,
7  | archetype_handle,
8  | harvest_log,
9  | plant,
10 | joined,
11 | pedigree,
12 | last_active,
13 | last_farm,
14 | acquired,
15 | nickname,
16 | xp

 */

#[derive(Debug, serde::Deserialize)]
struct Row {
    cat: usize,
    id: String,
    name: Option<String>,
    steader: String,
    archetype_handle: Option<u32>,
    // tile
    plant: Option<String>,
    // gotchi
    nickname: Option<String>,
    harvest_log: Option<String>,
    // seed
    pedigree: Option<String>,
    // profile
    xp: Option<u64>,
    joined: Option<String>,
    last_active: Option<String>,
    last_farm: Option<String>,
    // market
    price: Option<u32>,
    market_name: Option<String>,
    // item
    ownership_log: Option<String>,
    acquired: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum OldAcquisition {
    Trade,
    Purchase { price: u64 },
    Farmed,
    Crafted,
    Hatched,
}
impl Into<item::Acquisition> for OldAcquisition {
    fn into(self) -> item::Acquisition {
        use item::Acquisition::*;
        match self {
            OldAcquisition::Trade | OldAcquisition::Purchase { .. } => Trade,
            OldAcquisition::Farmed => Farmed,
            OldAcquisition::Crafted => Crafted,
            OldAcquisition::Hatched => Hatched,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OldOwner {
    pub id: String,
    pub acquisition: OldAcquisition,
}

lazy_static::lazy_static! {
    /// for fixing the weird number storage in the dynamodb csv exports
    pub static ref NUM_UNWRAPPER: regex::Regex = regex::Regex::new(r"Decimal\('-?(?P<n>[\d\.]+)'\)").unwrap();
}
#[test]
fn num_unwrapper() {
    assert_eq!(NUM_UNWRAPPER.replace_all("Decimal('0')", "$n"), "0");
    assert_eq!(NUM_UNWRAPPER.replace_all("Decimal('0.0')", "$n"), "0.0");
    assert_eq!(NUM_UNWRAPPER.replace_all("Decimal('-0.0')", "$n"), "0.0");
}
fn as_json<D: serde::de::DeserializeOwned>(s: &str) -> Result<D, String> {
    let fixed = NUM_UNWRAPPER
        .replace_all(s, "$n")
        .replace("'", "\"")
        .replace("False", "false");

    serde_json::from_str(&fixed).map_err(|e| format!("{} is bad json: {}", fixed, e))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use chrono::{DateTime, Utc};
    use hcor::Hackstead;
    use std::collections::HashMap;

    pretty_env_logger::init();

    let mut rdr = csv::ReaderBuilder::new()
        .from_path("hackagotchi.csv")
        .map_err(|e| format!("invalid csv: {}", e))?;
    let mut hacksteads: HashMap<String, (Hackstead, bool)> = HashMap::new();

    fn parse_date_time(dt: String) -> DateTime<Utc> {
        DateTime::parse_from_str(&format!("{} +0000", dt), "%Y-%m-%dT%H:%M:%S%.fZ %z")
            .unwrap_or_else(|e| panic!("couldn't parse dt {}: {}", dt, e))
            .into()
    }

    for raw_row in rdr.deserialize::<Row>() {
        let r = match raw_row {
            Ok(r) => {
                assert!(r.steader != "", "no steader in {:?}", r);
                r
            }
            Err(e) => {
                log::error!("error parsing row: {}", e);
                continue;
            }
        };

        let (hs, found_profile) = hacksteads
            .entry(r.steader.clone())
            .or_insert((Hackstead::empty(Some(&r.steader)), false));

        match r.cat {
            0 => {
                let (joined, last_active, last_farm) = (
                    r.joined.expect("profile but no joined"),
                    r.last_active.expect("profile no last_active"),
                    r.last_farm.expect("profile no last_farm"),
                );
                hs.profile.joined = parse_date_time(joined);
                hs.profile.last_active = parse_date_time(last_active);
                hs.profile.last_farm = parse_date_time(last_farm);
                hs.profile.xp = r.xp.expect("profile no xp") as usize;
                *found_profile = true;
            }
            1 | 2 => {
                let archetype_handle = r.archetype_handle.expect("item no archetype");
                let item_id = ItemId(uuid::Uuid::parse_str(&r.id).expect("item id not uuid"));
                let mut item = item::Item::from_archetype_handle(
                    archetype_handle as usize,
                    hs.profile.steader_id,
                    item::Acquisition::Trade,
                )
                .unwrap();
                item.item_id = item_id;
                if let (Some(nickname), Some(_)) = (r.nickname, r.harvest_log) {
                    item.gotchi_mut().unwrap().nickname = nickname;
                }

                hs.inventory.push(item)
            }
            3 => {
                use hcor::plant::{Craft, Effect};

                #[derive(serde::Serialize, serde::Deserialize)]
                struct OldPlant {
                    pub xp: usize,
                    pub until_yield: f32,
                    pub craft: Option<OldCraft>,
                    #[serde(default)]
                    pub effects: Vec<OldEffect>,
                    pub archetype_handle: usize,
                    #[serde(default)]
                    pub queued_xp_bonus: usize,
                }
                #[derive(serde::Serialize, serde::Deserialize)]
                pub struct OldCraft {
                    pub until_finish: f32,
                    #[serde(alias = "makes")]
                    pub recipe_archetype_handle: usize,
                }
                #[derive(serde::Serialize, serde::Deserialize)]
                pub struct OldEffect {
                    pub until_finish: Option<f32>,
                    pub item_archetype_handle: usize,
                    pub effect_archetype_handle: usize,
                }
                impl std::ops::Deref for OldPlant {
                    type Target = hcor::config::PlantArchetype;

                    fn deref(&self) -> &Self::Target {
                        &hcor::config::CONFIG
                            .plant_archetypes
                            .get(self.archetype_handle as usize)
                            .expect("invalid archetype handle")
                    }
                }

                let acquired = r.acquired.expect("tiles need acquired dates");
                let tile_id = TileId(uuid::Uuid::parse_str(&r.id).expect("tile id not uuid"));
                let p: Option<OldPlant> = r
                    .plant
                    .as_ref()
                    .map(|p| as_json(p).expect("bad plant json"));

                hs.land.push(hcor::Tile {
                    acquired: parse_date_time(acquired),
                    tile_id,
                    owner_id: hs.profile.steader_id,
                    plant: p.map(|p| hcor::Plant {
                        xp: p.xp,
                        owner_id: hs.profile.steader_id,
                        archetype_handle: p.archetype_handle,
                        nickname: p.name.clone(),
                        tile_id,
                        lifetime_rubs: p.effects.len(),
                        effects: p
                            .effects
                            .into_iter()
                            .map(|e| Effect {
                                effect_id: hcor::plant::EffectId(uuid::Uuid::new_v4()),
                                effect_archetype_handle: e.effect_archetype_handle,
                                item_archetype_handle: e.item_archetype_handle,
                            })
                            .collect(),
                        craft: p.craft.map(|c| Craft {
                            recipe_archetype_handle: c.recipe_archetype_handle,
                        }),
                    }),
                })
            }
            other => panic!("unknown category: {}", other),
        }
    }

    let len = hacksteads.len();
    for (i, (id, (hs, profile))) in hacksteads.into_iter().enumerate() {
        if profile {
            println!(
                "[inserting hackstead {} of {} ({}% complete)]",
                i,
                len,
                (i as f32 / len as f32) * 100.0
            );
            backend::fs_put_stead(&hs)?;
        } else {
            println!("ignoring {}", id);
        }
    }

    Ok(())
}
