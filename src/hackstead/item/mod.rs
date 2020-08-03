use crate::{uuid_or_lookup, ServiceError};
use actix_web::{post, web, HttpResponse};
use futures::stream::{self, StreamExt, TryStreamExt};
use hcor::item::{self, Item, ItemBase, ItemHatchRequest, ItemSpawnRequest, ItemTransferRequest};
use log::*;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

#[cfg(all(test, feature = "hcor_client"))]
mod test;

pub async fn db_get_inventory(pool: &PgPool, steader_id: Uuid) -> sqlx::Result<Vec<Item>> {
    let mut items: Vec<Item> = stream::iter(
        sqlx::query_as!(
            ItemBase,
            "SELECT * FROM items WHERE owner_id = $1",
            steader_id
        )
        .fetch_all(pool)
        .await?,
    )
    .map(|base| db_extend_item_base(pool, base))
    .buffer_unordered(crate::MIN_DB_CONNECTIONS as usize)
    .try_collect()
    .await?;

    items.sort_unstable_by_key(|i| i.base.archetype_handle);

    Ok(items)
}

pub async fn db_get_item(pool: &PgPool, item_id: Uuid) -> sqlx::Result<Item> {
    let item_base = sqlx::query_as!(ItemBase, "SELECT * FROM items WHERE item_id = $1", item_id)
        .fetch_one(pool)
        .await?;

    db_extend_item_base(pool, item_base).await
}

pub async fn db_remove_item(conn: &mut PgConnection, item_id: Uuid) -> sqlx::Result<()> {
    sqlx::query!("DELETE FROM items * WHERE item_id = $1", item_id)
        .execute(&mut *conn)
        .await?;

    Ok(())
}

pub async fn db_insert_item(conn: &mut PgConnection, i: Item) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO items ( item_id, owner_id, archetype_handle ) \
            VALUES ( $1, $2, $3 )",
        i.base.item_id,
        i.base.owner_id,
        i.base.archetype_handle
    )
    .execute(&mut *conn)
    .await?;

    if let Some(g) = i.gotchi {
        sqlx::query!(
            "INSERT INTO gotchi ( item_id, nickname ) \
                VALUES ( $1, $2 )",
            i.base.item_id,
            g.nickname
        )
        .execute(&mut *conn)
        .await?;
    }

    for ol in i.ownership_log {
        db_insert_logged_owner(&mut *conn, ol).await?;
    }

    Ok(())
}

pub async fn db_insert_logged_owner(
    conn: &mut PgConnection,
    ol: item::LoggedOwner,
) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO ownership_logs\
            ( item_id\
            , logged_owner_id\
            , owner_index\
            , acquisition\
            ) \
            VALUES ( $1, $2, $3, $4 )",
        ol.item_id,
        ol.logged_owner_id,
        ol.owner_index,
        ol.acquisition as i32,
    )
    .execute(&mut *conn)
    .await?;

    Ok(())
}

pub async fn db_get_ownership_logs(
    pool: &PgPool,
    item_id: Uuid,
) -> sqlx::Result<Vec<item::LoggedOwner>> {
    let q = sqlx::query!(
        "SELECT * FROM ownership_logs \
            WHERE item_id = $1
            ORDER BY owner_index",
        item_id
    );

    q.fetch_all(pool).await.map(|a| {
        a.into_iter()
            .filter_map(|rec| {
                Some(item::LoggedOwner {
                    item_id: rec.item_id,
                    logged_owner_id: rec.logged_owner_id,
                    owner_index: rec.owner_index,
                    acquisition: match item::Acquisition::try_from_i32(rec.acquisition) {
                        Some(a) => a,
                        _ => {
                            error!("unknown acquisition #: {}", rec.acquisition);
                            return None;
                        }
                    },
                })
            })
            .collect()
    })
}

pub async fn db_extend_item_base(pool: &PgPool, base: ItemBase) -> sqlx::Result<Item> {
    let (gotchi, ownership_log) = futures::join!(
        sqlx::query_as!(
            item::Gotchi,
            "SELECT * FROM gotchi WHERE item_id = $1",
            base.item_id
        )
        .fetch_one(pool),
        db_get_ownership_logs(pool, base.item_id)
    );

    Ok(Item {
        base,
        ownership_log: ownership_log?,
        gotchi: gotchi.ok(),
    })
}

#[post("/item/godyeet")]
pub async fn spawn_items(
    db: web::Data<PgPool>,
    req: web::Json<ItemSpawnRequest>,
) -> Result<HttpResponse, ServiceError> {
    debug!("servicing spawn_items request");

    let ItemSpawnRequest {
        receiver_id,
        item_archetype_handle,
        amount,
    } = req.clone();

    let receiver_uuid = uuid_or_lookup(&db, &receiver_id).await?;
    let mut tx = db.begin().await?;

    let items: Vec<Item> = (0..amount)
        .map(|_| {
            Item::from_archetype_handle(
                item_archetype_handle,
                receiver_uuid,
                item::Acquisition::spawned(),
            )
        })
        .collect::<hcor::ConfigResult<_>>()?;

    for item in items.clone() {
        db_insert_item(&mut tx, item).await?;
    }

    tx.commit().await?;

    Ok(HttpResponse::Ok().json(items))
}

#[post("/item/throw")]
pub async fn transfer_items(
    db: web::Data<PgPool>,
    req: web::Json<ItemTransferRequest>,
) -> Result<HttpResponse, ServiceError> {
    debug!("servicing transfer_items request");

    let mut tx = db.begin().await?;
    let receiver_id = uuid_or_lookup(&db, &req.receiver_id).await?;
    let sender_id = uuid_or_lookup(&db, &req.sender_id).await?;

    if receiver_id == sender_id {
        return Err(ServiceError::bad_request(format!(
            "can't transfer items from user {} to user {}; \
                they're the same user.",
            sender_id, receiver_id
        )));
    }

    let mut item_bases: Vec<ItemBase> = Vec::with_capacity(req.item_ids.len());
    for &item_id in &req.item_ids {
        let current_owner = db_get_ownership_logs(&db, item_id)
            .await?
            .into_iter()
            .max_by_key(|ol| ol.owner_index)
            .ok_or_else(|| {
                error!("item {} has no logged owners!", item_id);
                ServiceError::InternalServerError
            })?;

        if current_owner.logged_owner_id != sender_id {
            return Err(ServiceError::bad_request(format!(
                "can't transfer item {} from user {} to user {}; \
                    it doesn't belong to sender.",
                item_id, sender_id, receiver_id
            )));
        }

        db_insert_logged_owner(
            &mut tx,
            item::LoggedOwner {
                item_id,
                logged_owner_id: receiver_id,
                acquisition: item::Acquisition::Trade,
                owner_index: current_owner.owner_index + 1,
            },
        )
        .await?;

        let base = sqlx::query_as!(
            ItemBase,
            "UPDATE items \
                SET owner_id = $1 \
                WHERE item_id = $2 AND owner_id = $3 \
                RETURNING *",
            receiver_id,
            item_id,
            sender_id,
        )
        .fetch_one(&mut tx)
        .await?;

        item_bases.push(base);
    }

    tx.commit().await?;

    let items: Vec<Item> = stream::iter(item_bases)
        .map(|base| db_extend_item_base(&db, base))
        .buffer_unordered(crate::MIN_DB_CONNECTIONS as usize)
        .try_collect()
        .await?;

    Ok(HttpResponse::Ok().json(items))
}

#[post("/item/hatch")]
pub async fn hatch_item(
    db: web::Data<PgPool>,
    req: web::Json<ItemHatchRequest>,
) -> Result<HttpResponse, ServiceError> {
    debug!("servicing new_tile request");

    let mut tx = db.begin().await?;
    let item_id = req.hatchable_item_id;
    let item = db_get_item(&db, item_id).await?;
    db_remove_item(&mut tx, item_id).await?;

    let hatch_table = item.hatch_table.as_ref().ok_or_else(|| {
        ServiceError::bad_request(format!(
            "item {}[{}] is not configured to be hatchable",
            item.name, item.base.archetype_handle,
        ))
    })?;

    let items = hcor::config::spawn(&hatch_table, &mut rand::thread_rng())
        .map(|item_name| {
            Item::from_archetype(
                hcor::CONFIG.find_possession(&item_name)?,
                item.base.owner_id,
                item::Acquisition::Hatched,
            )
        })
        .collect::<Result<Vec<Item>, hcor::ConfigError>>()
        .map_err(|e| {
            error!("hatch table produced: {}", e);
            ServiceError::InternalServerError
        })?;

    for i in items.clone() {
        db_insert_item(&mut tx, i).await?;
    }
    tx.commit().await?;

    Ok(HttpResponse::Ok().json(items))
}