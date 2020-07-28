use crate::{db_pool, ServiceError};
use actix_web::{get, post, web, HttpResponse};
use hcor::{hackstead, Hackstead, UserId};
use log::*;
pub use sqlx::PgPool;
pub use sqlx::{Executor, Postgres};

//mod tile;
#[cfg(test)]
mod test;

async fn insert_item(
    mut exec: impl Executor<Database = Postgres>,
    i: hcor::item::Item,
) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
INSERT INTO items ( item_id, owner_id, archetype_handle )
VALUES ( $1, $2, $3 )"#,
        i.base.item_id,
        i.base.owner_id,
        i.base.archetype_handle
    )
    .execute(&mut exec)
    .await?;

    if let Some(g) = i.gotchi {
        sqlx::query!(
            r#"
INSERT INTO gotchi ( item_id, nickname )
VALUES ( $1, $2 )"#,
            i.base.item_id,
            g.nickname
        )
        .execute(&mut exec)
        .await?;
    }

    for ol in i.ownership_log {
        sqlx::query!(
            r#"
INSERT INTO ownership_logs ( item_id, logged_owner_id, owner_index, acquisition )
VALUES ( $1, $2, $3, $4 )"#,
            ol.item_id,
            ol.logged_owner_id,
            ol.owner_index,
            ol.acquisition as i32,
        )
        .execute(&mut exec)
        .await?;
    }

    Ok(())
}
async fn insert_tile(
    mut exec: impl Executor<Database = Postgres>,
    t: hackstead::Tile,
) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
INSERT INTO tiles ( tile_id, owner_id, acquired )
VALUES ( $1, $2, $3 )"#,
        t.base.tile_id,
        t.base.owner_id,
        t.base.acquired,
    )
    .execute(&mut exec)
    .await?;

    if let Some(p) = t.plant {
        sqlx::query!(
            r#"
INSERT INTO plants ( tile_id, xp, nickname, until_yield, archetype_handle )
VALUES ( $1, $2, $3, $4, $5 )"#,
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
                r#"
INSERT INTO plant_crafts ( tile_id, until_finish, recipe_archetype_handle )
VALUES ( $1, $2, $3 )"#,
                c.tile_id,
                c.until_finish,
                c.recipe_archetype_handle
            )
            .execute(&mut exec)
            .await?;
        }

        for e in p.effects {
            sqlx::query!(
                r#"
INSERT INTO plant_effects ( tile_id, until_finish, item_archetype_handle, effect_archetype_handle )
VALUES ( $1, $2, $3, $4 )"#,
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

pub async fn db_insert_hackstead(
    exec: &mut impl Executor<Database = Postgres>,
    hs: Hackstead,
) -> sqlx::Result<()> {
    sqlx::query!(r#"
INSERT INTO steaders ( steader_id, slack_id, xp, extra_land_plot_count, joined, last_active, last_farm )
VALUES ( $1, $2, $3, $4, $5, $6, $7 )"#,
        hs.profile.steader_id,
        hs.profile.slack_id,
        hs.profile.xp as i32,
        hs.profile.extra_land_plot_count as i32,
        hs.profile.joined,
        hs.profile.last_active,
        hs.profile.last_farm,
    )
    .execute(&mut *exec)
    .await?;

    for i in hs.inventory {
        insert_item(&mut *exec, i).await?;
    }
    for t in hs.land {
        insert_tile(&mut *exec, t).await?;
    }

    Ok(())
}

pub async fn db_insert_hackstead_transactional(pool: &PgPool, hs: Hackstead) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    db_insert_hackstead(&mut tx, hs).await?;
    tx.commit().await?;

    Ok(())
}

pub async fn db_get_profile(pool: &PgPool, id: &UserId) -> sqlx::Result<hackstead::Profile> {
    match id {
        UserId::Uuid(uuid) | UserId::Both { uuid, .. } => {
            sqlx::query_as!(
                hackstead::Profile,
                r#"
SELECT *
FROM steaders
WHERE steader_id = $1"#,
                *uuid
            )
            .fetch_one(pool)
            .await
        }
        UserId::Slack(slack) => {
            sqlx::query_as!(
                hackstead::Profile,
                r#"
SELECT *
FROM steaders
WHERE slack_id = $1"#,
                slack
            )
            .fetch_one(pool)
            .await
        }
    }
}

async fn uuid_or_lookup(pool: &PgPool, id: &UserId) -> sqlx::Result<uuid::Uuid> {
    match id {
        UserId::Uuid(uuid) | UserId::Both { uuid, .. } => Ok(*uuid),
        UserId::Slack(slack) => sqlx::query!(
            r#"
SELECT steader_id FROM steaders
WHERE slack_id = $1
            "#,
            slack
        )
        .fetch_one(pool)
        .await
        .map(|record| record.steader_id),
    }
}

pub async fn db_extend_item_base(
    pool: &PgPool,
    base: hcor::item::ItemBase,
) -> sqlx::Result<hcor::Item> {
    let (gotchi, ownership_log) = futures::join!(
        sqlx::query_as!(
            hcor::item::Gotchi,
            "SELECT * FROM gotchi WHERE item_id = $1",
            base.item_id
        )
        .fetch_one(pool),
        async {
            sqlx::query!(
                "SELECT * FROM ownership_logs WHERE item_id = $1",
                base.item_id
            )
            .fetch_all(pool)
            .await
            .map(|a| {
                a.into_iter()
                    .filter_map(|rec| {
                        Some(hcor::item::LoggedOwner {
                            item_id: rec.item_id,
                            logged_owner_id: rec.logged_owner_id,
                            owner_index: rec.owner_index,
                            acquisition: match hcor::item::Acquisition::try_from_i32(
                                rec.acquisition,
                            ) {
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
    );

    Ok(hcor::Item {
        base,
        ownership_log: ownership_log?,
        gotchi: gotchi.ok(),
    })
}

pub async fn db_extend_plant_base(
    pool: &PgPool,
    base: hcor::hackstead::plant::PlantBase,
) -> sqlx::Result<hcor::hackstead::plant::Plant> {
    let (craft, effects) = futures::join!(
        sqlx::query_as!(
            hcor::hackstead::plant::Craft,
            "SELECT * FROM plant_crafts WHERE tile_id = $1",
            base.tile_id
        )
        .fetch_one(pool),
        sqlx::query_as!(
            hcor::hackstead::plant::Effect,
            "SELECT * FROM plant_effects WHERE tile_id = $1",
            base.tile_id
        )
        .fetch_all(pool)
    );

    Ok(hcor::hackstead::plant::Plant {
        base,
        craft: craft.ok(),
        effects: effects?,
        queued_xp_bonus: 0,
    })
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

    Ok(hcor::hackstead::Tile {
        base,
        plant: match plant_base.ok() {
            Some(pb) => Some(db_extend_plant_base(pool, pb).await?),
            None => None,
        },
    })
}

pub async fn db_get_hackstead(pool: &PgPool, id: &UserId) -> sqlx::Result<hackstead::Hackstead> {
    use futures::stream::{StreamExt, TryStreamExt};

    let profile = db_get_profile(pool, id).await?;

    let inventory = futures::stream::iter(
        sqlx::query_as!(
            hcor::item::ItemBase,
            r#"
SELECT *
FROM items
WHERE owner_id = $1
            "#,
            profile.steader_id
        )
        .fetch_all(pool)
        .await?,
    )
    .map(|base| db_extend_item_base(pool, base))
    .buffer_unordered(50)
    .try_collect()
    .await?;

    let land = futures::stream::iter(
        sqlx::query_as!(
            hcor::hackstead::TileBase,
            r#"
SELECT *
FROM tiles
WHERE owner_id = $1
            "#,
            profile.steader_id
        )
        .fetch_all(pool)
        .await?,
    )
    .map(|base| db_extend_tile_base(pool, base))
    .buffer_unordered(50)
    .try_collect()
    .await?;

    Ok(hackstead::Hackstead {
        profile,
        inventory,
        land,
    })
}

#[get("/hackstead/")]
pub async fn get_hackstead(user: web::Json<UserId>) -> Result<HttpResponse, ServiceError> {
    debug!("servicing get_hackstead request");

    let stead: Hackstead = db_get_hackstead(&db_pool().await?, &*user).await?;
    trace!("got hackstead: {:#?}", stead);

    Ok(HttpResponse::Ok().json(stead))
}

#[post("/hackstead/new")]
pub async fn new_hackstead(user: web::Json<UserId>) -> Result<HttpResponse, ServiceError> {
    debug!("servicing new_hackstead request");

    db_insert_hackstead_transactional(&db_pool().await?, Hackstead::new_user(user.slack())).await?;

    Ok(HttpResponse::Created().finish())
}

#[post("/hackstead/remove")]
pub async fn remove_hackstead(user: web::Json<UserId>) -> Result<HttpResponse, ServiceError> {
    debug!("servicing remove_hackstead request");

    let pool = db_pool().await?;
    let stead: Hackstead = db_get_hackstead(&pool, &*user).await?;
    match &*user {
        UserId::Uuid(uuid) | UserId::Both { uuid, .. } => {
            sqlx::query!("DELETE FROM steaders * WHERE steader_id = $1", *uuid)
                .execute(&pool)
                .await
        }
        UserId::Slack(slack_id) => {
            sqlx::query!("DELETE FROM steaders * WHERE slack_id = $1", slack_id)
                .execute(&pool)
                .await
        }
    }?;
    debug!(":( removed hackstead: {:#?}", stead);

    Ok(HttpResponse::Ok().json(stead))
}
