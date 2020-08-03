use crate::ServiceError;
use actix_web::{post, web, HttpResponse};
use hcor::plant::{
    self, Plant, PlantBase, PlantCreationRequest, PlantRemovalRequest, PlantRubRequest,
};
use log::*;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

#[cfg(all(test, feature = "hcor_client"))]
mod test;

pub async fn db_get_plant(pool: &PgPool, tile_id: Uuid) -> sqlx::Result<Plant> {
    let plant_base = sqlx::query_as!(
        PlantBase,
        "SELECT * FROM plants WHERE tile_id = $1",
        tile_id
    )
    .fetch_one(pool)
    .await?;

    db_extend_plant_base(pool, plant_base).await
}

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
            "SELECT * FROM plant_effects WHERE tile_id = $1 ORDER BY rub_index",
            base.tile_id
        )
        .fetch_all(pool)
    );

    Ok(Plant {
        base,
        craft: craft.ok(),
        effects: effects?,
    })
}

pub async fn db_insert_plant(conn: &mut PgConnection, p: Plant) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO plants\
            ( tile_id\
            , xp\
            , nickname\
            , until_yield\
            , archetype_handle\
            , lifetime_effect_count\
            ) \
        VALUES ( $1, $2, $3, $4, $5, $6 )",
        p.base.tile_id,
        p.base.xp,
        p.base.nickname,
        p.base.until_yield,
        p.base.archetype_handle,
        p.base.lifetime_effect_count
    )
    .execute(&mut *conn)
    .await?;

    if let Some(c) = p.craft {
        sqlx::query!(
            "INSERT INTO plant_crafts\
                ( tile_id\
                , until_finish\
                , recipe_archetype_handle\
                ) \
            VALUES ( $1, $2, $3 )",
            c.tile_id,
            c.until_finish,
            c.recipe_archetype_handle
        )
        .execute(&mut *conn)
        .await?;
    }

    for e in p.effects {
        db_insert_effect(&mut *conn, e).await?;
    }

    Ok(())
}

pub async fn db_insert_effect(conn: &mut PgConnection, e: plant::Effect) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO plant_effects
            ( tile_id\
            , until_finish\
            , item_archetype_handle\
            , effect_archetype_handle\
            , rub_index\
            ) \
        VALUES ( $1, $2, $3, $4, $5 )",
        e.tile_id,
        e.until_finish,
        e.item_archetype_handle,
        e.effect_archetype_handle,
        e.rub_index
    )
    .execute(&mut *conn)
    .await?;

    Ok(())
}

pub async fn db_remove_plant(conn: &mut PgConnection, tile_id: Uuid) -> sqlx::Result<()> {
    sqlx::query!("DELETE FROM plants * WHERE tile_id = $1", tile_id)
        .execute(&mut *conn)
        .await?;

    Ok(())
}

#[post("/plant/summon")]
pub async fn new_plant(
    db: web::Data<PgPool>,
    req: web::Json<PlantCreationRequest>,
) -> Result<HttpResponse, ServiceError> {
    use crate::item::{db_get_item, db_remove_item};

    debug!("servicing plant_new request");

    let mut tx = db.begin().await?;

    // fetch and verify the seed item
    let item_id = req.seed_item_id;
    let item = db_get_item(&db, item_id).await?;
    db_remove_item(&mut tx, item_id).await?;
    let seed = item.seed.as_ref().ok_or_else(|| {
        ServiceError::bad_request(format!(
            "item {}[{}] is not configured to be used as a seed",
            item.name, item.base.archetype_handle,
        ))
    })?;

    // fetch and verify the tile the seed may soon occupy
    let tile_id = req.tile_id;
    let tile = super::db_get_tile(&db, tile_id).await?;
    if let Some(plant) = tile.plant {
        return Err(ServiceError::bad_request(format!(
            "can't plant here; tile {} is already occupied by a {}[{}] plant.",
            tile_id, plant.name, plant.base.archetype_handle,
        )));
    }

    let plant = Plant::from_seed(tile_id, seed).map_err(|_| {
        error!("seed grows into unknown archetype: {:#?}", item);
        ServiceError::InternalServerError
    })?;
    db_insert_plant(&mut tx, plant.clone()).await?;
    tx.commit().await?;

    Ok(HttpResponse::Ok().json(plant))
}

#[post("/plant/rub")]
pub async fn rub_plant(
    db: web::Data<PgPool>,
    req: web::Json<PlantRubRequest>,
) -> Result<HttpResponse, ServiceError> {
    use crate::item::{db_get_item, db_remove_item};

    debug!("servicing rub_plant request");

    let mut tx = db.begin().await?;

    // fetch and verify the seed item
    let item_id = req.rub_item_id;
    let item = db_get_item(&db, item_id).await?;
    db_remove_item(&mut tx, item_id).await?;

    // fetch and verify the tile the plant to rub onto
    let tile_id = req.tile_id;
    let mut tile = super::db_get_tile(&db, tile_id).await?;
    let plant = tile.plant.as_mut().ok_or_else(|| {
        ServiceError::bad_request(format!(
            "can't rub {}[{}]; tile {} is not occupied by a plant.",
            item.name, item.base.archetype_handle, tile_id
        ))
    })?;
    let plant_name = plant.name.clone();

    // find out which effects are relevant and rub them on, if any are.
    let mut effect_archetypes = item.rub_effects_for_plant_indexed(&plant_name).peekable();
    if effect_archetypes.peek().is_none() {
        return Err(ServiceError::bad_request(format!(
            "can't rub {}[{}]; \
                rubbing this item on the {}[{}] plant \
                living on tile {} would have no effect on it.",
            item.name, item.base.archetype_handle, plant.name, plant.base.archetype_handle, tile_id,
        )));
    }
    let mut effects: Vec<plant::Effect> = Vec::with_capacity(item.plant_rub_effects.len());
    for (i, a) in effect_archetypes {
        let e = plant::Effect {
            rub_index: plant.next_effect_index(),
            tile_id,
            until_finish: a.duration,
            item_archetype_handle: item.base.archetype_handle,
            effect_archetype_handle: i as hcor::config::ArchetypeHandle,
        };

        sqlx::query!(
            "UPDATE plants \
                SET lifetime_effect_count = lifetime_effect_count + 1 \
                WHERE tile_id = $1",
            tile_id,
        )
        .execute(&mut tx)
        .await?;

        db_insert_effect(&mut tx, e.clone()).await?;
        effects.push(e);
    }
    tx.commit().await?;

    Ok(HttpResponse::Ok().json(effects))
}

#[post("/plant/slaughter")]
pub async fn remove_plant(
    db: web::Data<PgPool>,
    req: web::Json<PlantRemovalRequest>,
) -> Result<HttpResponse, ServiceError> {
    debug!("servicing remove_plant request");

    let PlantRemovalRequest { tile_id } = req.clone();
    let plant: Plant = db_get_plant(&db, tile_id).await?;
    db_remove_plant(&mut *db.acquire().await?, tile_id).await?;

    debug!(":( removed plant: {:#?}", plant);

    Ok(HttpResponse::Ok().json(plant))
}
