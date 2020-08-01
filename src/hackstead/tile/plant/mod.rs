use crate::ServiceError;
use actix_web::{post, web, HttpResponse};
use hcor::plant::{
    self, Plant, PlantApplicationRequest, PlantBase, PlantCreationRequest, PlantRemovalRequest,
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

pub async fn db_insert_plant(conn: &mut PgConnection, p: Plant) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO plants\
            ( tile_id\
            , xp\
            , nickname\
            , until_yield\
            , archetype_handle\
            ) \
        VALUES ( $1, $2, $3, $4, $5 )",
        p.base.tile_id,
        p.base.xp,
        p.base.nickname,
        p.base.until_yield,
        p.base.archetype_handle
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
            ) \
        VALUES ( $1, $2, $3, $4 )",
        e.tile_id,
        e.until_finish,
        e.item_archetype_handle,
        e.effect_archetype_handle
    )
    .execute(&mut *conn)
    .await?;

    Ok(())
}

#[post("/plant/new")]
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

#[post("/plant/apply")]
pub async fn apply_plant(
    db: web::Data<PgPool>,
    req: web::Json<PlantApplicationRequest>,
) -> Result<HttpResponse, ServiceError> {
    use crate::item::{db_get_item, db_remove_item};

    debug!("servicing apply_plant request");

    let mut tx = db.begin().await?;

    // fetch and verify the seed item
    let item_id = req.applicable_item_id;
    let item = db_get_item(&db, item_id).await?;
    db_remove_item(&mut tx, item_id).await?;
    let plant_application = item.plant_application.as_ref().ok_or_else(|| {
        ServiceError::bad_request(format!(
            "item {}[{}] is not configured to be applied to a plant",
            item.name, item.base.archetype_handle,
        ))
    })?;

    // fetch and verify the tile the plant to apply to
    let tile_id = req.tile_id;
    let tile = super::db_get_tile(&db, tile_id).await?;
    let plant = tile.plant.as_ref().ok_or_else(|| {
        ServiceError::bad_request(format!(
            "can't apply {}[{}]; tile {} is not occupied by a plant.",
            item.name, item.base.archetype_handle, tile_id
        ))
    })?;

    // find out which effects are relevant and apply them, if any are.
    let mut effect_archetypes = plant_application
        .effects
        .iter()
        .enumerate()
        .filter(|(_, e)| e.for_plants.allows(&plant.name))
        .peekable();
    if effect_archetypes.peek().is_none() {
        return Err(ServiceError::bad_request(format!(
            "can't apply {}[{}]; this item would have no effect to the {}[{}] plant living on tile {}.",
            item.name, item.base.archetype_handle,
            plant.name, plant.base.archetype_handle, tile_id, 
        )));
    }
    let mut effects: Vec<plant::Effect> = Vec::with_capacity(plant_application.effects.len());
    for (i, a) in effect_archetypes {
        let e = plant::Effect {
            tile_id,
            until_finish: a.duration,
            item_archetype_handle: item.base.archetype_handle,
            effect_archetype_handle: i as hcor::config::ArchetypeHandle,
        };
        db_insert_effect(&mut tx, e.clone()).await?;
        effects.push(e);
    }
    tx.commit().await?;

    Ok(HttpResponse::Ok().json(effects))
}

#[post("/plant/remove")]
pub async fn remove_plant(
    db: web::Data<PgPool>,
    req: web::Json<PlantRemovalRequest>,
) -> Result<HttpResponse, ServiceError> {
    debug!("servicing remove_plant request");

    let PlantRemovalRequest { tile_id } = req.clone();
    let plant: Plant = db_get_plant(&db, tile_id).await?;
    sqlx::query!("DELETE FROM plants * WHERE tile_id = $1", tile_id)
        .execute(&**db)
        .await?;

    debug!(":( removed plant: {:#?}", plant);

    Ok(HttpResponse::Ok().json(plant))
}
