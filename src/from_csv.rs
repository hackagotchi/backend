//! Dumps a CSV file of the old dynamodb format into the mongodb.
//! input CSVs are assumed to be generated with this tool: https://pypi.org/project/export-dynamodb/
use hcor::item;
use serde::{Deserialize, Serialize};

mod data;

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
            OldAcquisition::Trade => Trade,
            OldAcquisition::Purchase { price } => Purchase { price },
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
impl Into<item::Owner> for OldOwner {
    fn into(self) -> item::Owner {
        item::Owner {
            id: self.id,
            acquisition: self.acquisition.into(),
        }
    }
}

lazy_static::lazy_static! {
    /// for fixing the weird number storage in the dynamodb csv exports
    pub static ref NUM_UNWRAPPER: regex::Regex = regex::Regex::new(r"Decimal\('-?(?P<n>[\d\.]+)'\)").unwrap();
}
#[test]
fn num_unwrapper() {
    assert_eq!(NUM_UNWRAPPER.replace_all("Decimal('0')", "$n"), "0");
    assert_eq!(NUM_UNWRAPPER.replace_all("Decimal('0.0')", "$n"), "0");
    assert_eq!(NUM_UNWRAPPER.replace_all("Decimal('-0.0')", "$n"), "0");
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
    use chrono::NaiveDateTime;
    use futures::StreamExt;
    use hcor::Hackstead;
    use std::collections::HashMap;

    pretty_env_logger::init();

    let mut rdr = csv::ReaderBuilder::new().from_path("hackagotchi.csv")?;
    let mut hacksteads: HashMap<String, (Hackstead, bool)> = HashMap::new();

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
            .or_insert((Hackstead::new(r.steader.clone()), false));

        match r.cat {
            0 => {
                let (joined, last_active, last_farm) = (
                    r.joined.expect("profile but no joined"),
                    r.last_active.expect("profile no last_active"),
                    r.last_farm.expect("profile no last_farm"),
                );
                hs.profile.joined = NaiveDateTime::parse_from_str(&joined, "%Y-%m-%dT%H:%M:%S%.fZ")
                    .unwrap_or_else(|e| panic!("couldn't parse joined {}: {}", joined, e));
                hs.profile.last_active =
                    NaiveDateTime::parse_from_str(&last_active, "%Y-%m-%dT%H:%M:%S%.fZ")
                        .unwrap_or_else(|e| {
                            panic!("couldn't parse last_active {}: {}", last_active, e)
                        });
                hs.profile.last_farm =
                    NaiveDateTime::parse_from_str(&last_farm, "%Y-%m-%dT%H:%M:%S%.fZ")
                        .unwrap_or_else(|e| {
                            panic!("couldn't parse last_farm {}: {}", last_farm, e)
                        });
                hs.profile.xp = r.xp.expect("profile no xp");
                *found_profile = true;
            }
            1 | 2 => {
                let archetype_handle = r.archetype_handle.expect("item no archetype");
                hs.inventory.push(item::Item {
                    gotchi: if let (Some(nickname), Some(hl)) = (r.nickname, r.harvest_log) {
                        Some(item::Gotchi {
                            archetype_handle,
                            nickname,
                            harvest_log: as_json(&hl).unwrap_or_else(|e| {
                                log::error!("error parsing harvest log: {}", e);
                                Default::default()
                            }),
                        })
                    } else {
                        None
                    },
                    seed: r.pedigree.as_ref().map(|p| item::Seed {
                        pedigree: as_json(p).unwrap_or_else(|e| {
                            log::error!("error parsing seed pedigree: {}", e);
                            Default::default()
                        }),
                        archetype_handle,
                    }),
                    id: uuid::Uuid::parse_str(&r.id).expect("item id not uuid"),
                    archetype_handle,
                    steader: r.steader.clone(),
                    ownership_log: as_json::<Vec<OldOwner>>(
                        &r.ownership_log.expect("item no ownership_log"),
                    )
                    .unwrap_or_else(|e| {
                        log::error!("error parsing ownership log: {}", e);
                        Default::default()
                    })
                    .into_iter()
                    .map(|x| x.into())
                    .collect(),
                    sale_price: r.price.map(|x| x as i32),
                })
            }
            3 => {
                let acquired = r.acquired.expect("tiles need acquired dates");

                hs.land.push(hcor::hackstead::Tile {
                    acquired: NaiveDateTime::parse_from_str(&acquired, "%Y-%m-%dT%H:%M:%S%.fZ")
                        .unwrap_or_else(|e| {
                            panic!("couldn't parse tile acquired {}: {}", acquired, e)
                        }),
                    plant: r
                        .plant
                        .as_ref()
                        .map(|p| as_json(p).expect("bad plant json")),
                    id: uuid::Uuid::parse_str(&r.id).expect("tile id not uuid"),
                    steader: r.steader.clone(),
                })
            }
            other => panic!("unknown category: {}", other),
        }
    }

    // technically a colllection but whatever
    let hacksteads_db = data::hacksteads().await?;
    futures::stream::iter(
        hacksteads
            .into_iter()
            .filter(|(_, (_, found_profile))| *found_profile)
            .map(|(_, (hs, _))| {
                data::to_doc(&hs).unwrap_or_else(|e| {
                    panic!("couldn't deserialize {:#?}: {}", hs, e);
                })
            }),
    )
    .for_each_concurrent(500, |hs| async {
        hacksteads_db.insert_one(hs, None).await.unwrap();
    })
    .await;

    Ok(())
}
