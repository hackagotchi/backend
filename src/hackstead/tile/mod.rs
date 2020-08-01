use crate::ServiceError;
use actix_web::{post, web, HttpResponse};
use futures::stream::{self, StreamExt, TryStreamExt};
use hcor::tile::{Tile, TileBase, TileCreationRequest};
use log::*;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

pub(crate) mod plant;
#[cfg(all(test, feature = "hcor_client"))]
mod test;

pub async fn db_get_land(pool: &PgPool, steader_id: Uuid) -> sqlx::Result<Vec<Tile>> {
    let mut tiles: Vec<Tile> = stream::iter(
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
    .await?;

    tiles.sort_unstable_by_key(|t| t.base.acquired);

    Ok(tiles)
}

pub async fn db_get_tile(pool: &PgPool, tile_id: Uuid) -> sqlx::Result<Tile> {
    let tile_base = sqlx::query_as!(TileBase, "SELECT * FROM tiles WHERE tile_id = $1", tile_id)
        .fetch_one(pool)
        .await?;

    db_extend_tile_base(pool, tile_base).await
}

pub async fn db_extend_tile_base(pool: &PgPool, base: TileBase) -> sqlx::Result<Tile> {
    let plant = match plant::db_get_plant(&pool, base.tile_id).await {
        Ok(p) => Ok(Some(p)),
        Err(sqlx::Error::RowNotFound) => Ok(None),
        Err(other) => Err(other),
    }?;

    Ok(Tile { base, plant })
}

pub async fn db_insert_tile(conn: &mut PgConnection, t: Tile) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO tiles ( tile_id, owner_id, acquired ) \
            VALUES ( $1, $2, $3 )",
        t.base.tile_id,
        t.base.owner_id,
        t.base.acquired,
    )
    .execute(&mut *conn)
    .await?;

    if let Some(p) = t.plant {
        plant::db_insert_plant(&mut *conn, p).await?;
    }

    Ok(())
}

#[post("/tile/summon")]
pub async fn new_tile(
    db: web::Data<PgPool>,
    req: web::Json<TileCreationRequest>,
) -> Result<HttpResponse, ServiceError> {
    use crate::item::{db_get_item, db_remove_item};

    debug!("servicing new_tile request");

    let mut tx = db.begin().await?;
    let item_id = req.tile_redeemable_item_id;
    let item = db_get_item(&db, item_id).await?;
    db_remove_item(&mut tx, item_id).await?;

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
