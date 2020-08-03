use sqlx::{PgConnection, PgPool};

use super::SenderBundle;
use crate::hackstead::db_get_hackstead;
use hcor::{IdentifiesUser, Note, Plant};

pub(super) async fn update_stead(db: PgPool, sender: SenderBundle) -> sqlx::Result<()> {
    let steader_id = sender.uuid;
    let mut tx = db.begin().await?;
    let mut stead = db_get_hackstead(&db, &steader_id.user_id()).await?;

    for plant in stead.plants_mut() {
        update_plant(&mut tx, plant, &sender).await?;
    }

    tx.commit().await.map(|_| ())
}

async fn update_plant(
    tx: &mut PgConnection,
    plant: &mut Plant,
    sender: &SenderBundle,
) -> sqlx::Result<()> {
    use hcor::IdentifiesTile;

    for (uf, e) in plant
        .effects
        .iter()
        .filter_map(|e| e.until_finish.map(|uf| (uf, e)))
    {
        if uf <= 0.0 {
            sender.send(Note::PlantEffectFinish {
                effect: *e,
                tile_id: plant.tile_id(),
            });
        } else {
            sender.send(Note::PlantEffectProgress {
                rub_index: e.rub_index,
                tile_id: plant.tile_id(),
                until_finish: uf,
            });
            sqlx::query!(
                "UPDATE plant_effects \
                    SET until_finish = until_finish - 1 \
                    WHERE tile_id = $1 AND rub_index = $2",
                plant.tile_id(),
                e.rub_index,
            )
            .execute(&mut *tx)
            .await?;
        }
    }

    Ok(())
}
