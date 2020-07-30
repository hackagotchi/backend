use hcor::hackstead::plant::{self, Plant, PlantBase};
use sqlx::PgPool;

pub async fn db_extend_plant_base(pool: &PgPool, base: PlantBase) -> sqlx::Result<Plant> {
    let (craft, effects) = futures::join!(
        sqlx::query_as!(
            plant::Craft,
            "SELECT * FROM plant_crafts WHERE tile_id = $1",
            base.tile_id
        )
        .fetch_one(pool),
        sqlx::query_as!(
            plant::Effect,
            "SELECT * FROM plant_effects WHERE tile_id = $1",
            base.tile_id
        )
        .fetch_all(pool)
    );

    Ok(Plant {
        base,
        craft: craft.ok(),
        effects: effects?,
        queued_xp_bonus: 0,
    })
}
