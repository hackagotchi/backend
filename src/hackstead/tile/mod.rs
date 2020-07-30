use crate::ServiceError;
use actix_web::{post, web, HttpResponse};
use futures::stream::{self, StreamExt, TryStreamExt};
use hcor::hackstead::{Tile, TileBase, TileCreationRequest};
use log::*;
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

mod plant;
#[cfg(all(test, feature = "hcor_client"))]
mod test;

pub async fn db_get_land(pool: &PgPool, steader_id: Uuid) -> sqlx::Result<Vec<Tile>> {
    stream::iter(
        sqlx::query_as!(
            TileBase,
            "SELECT * FROM tiles WHERE owner_id = $1",
            steader_id
        )
        .fetch_all(pool)
        .await?,
    )
    .map(|base| db_extend_tile_base(pool, base))
    .buffer_unordered(crate::MIN_DB_CONNECTIONS as usize)
    .try_collect()
    .await
}

pub async fn db_extend_tile_base(
    pool: &PgPool,
    base: hcor::hackstead::TileBase,
) -> sqlx::Result<hcor::hackstead::Tile> {
    let plant_base = sqlx::query_as!(
        hcor::hackstead::plant::PlantBase,
        "SELECT * FROM plants WHERE tile_id = $1",
        base.tile_id
    )
    .fetch_one(pool)
    .await;

    Ok(Tile {
        base,
        plant: match plant_base.ok() {
            Some(pb) => Some(plant::db_extend_plant_base(pool, pb).await?),
            None => None,
        },
    })
}

pub async fn db_insert_tile(
    mut exec: impl Executor<Database = Postgres>,
    t: Tile,
) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO tiles ( tile_id, owner_id, acquired ) \
            VALUES ( $1, $2, $3 )",
        t.base.tile_id,
        t.base.owner_id,
        t.base.acquired,
    )
    .execute(&mut exec)
    .await?;

    if let Some(p) = t.plant {
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
        .execute(&mut exec)
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
            .execute(&mut exec)
            .await?;
        }

        for e in p.effects {
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
            .execute(&mut exec)
            .await?;
        }
    }

    Ok(())
}

#[post("/tile/new")]
pub async fn new_tile(
    db: web::Data<PgPool>,
    req: web::Json<TileCreationRequest>,
) -> Result<HttpResponse, ServiceError> {
    debug!("servicing new_tile request");

    let mut tx = db.begin().await?;
    let item_id = req.tile_redeemable_item_id;
    let item = super::item::db_get_item(&db, item_id).await?;
    super::item::db_remove_item(&mut tx, item_id).await?;

    let land_unlock = item.unlocks_land.as_ref().ok_or_else(|| {
        ServiceError::bad_request(format!(
            "item {}[{}] is not configured to unlock land",
            item.name, item.base.archetype_handle,
        ))
    })?;

    let mut hs = super::db_get_hackstead(&db, &hcor::UserId::Uuid(item.base.owner_id)).await?;
    if !land_unlock.requires_xp {
        hs.profile.extra_land_plot_count += 1;

        sqlx::query!(
            "UPDATE steaders \
                SET extra_land_plot_count = extra_land_plot_count + 1 \
                WHERE steader_id = $1",
            item.base.owner_id
        )
        .execute(&mut tx)
        .await?;
    }

    if hs.land_unlock_eligible() {
        let tile = Tile::new(hs.profile.steader_id);
        db_insert_tile(&mut tx, tile.clone()).await?;
        tx.commit().await?;

        Ok(HttpResponse::Ok().json(tile))
    } else {
        Err(ServiceError::bad_request(format!(
            "Steader {} is not eligible to redeem more land",
            hs.profile.steader_id,
        )))
    }
}
