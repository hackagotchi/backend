use crate::ServiceError;
use actix_web::{get, post, web, HttpResponse};
use hcor::{
    hackstead::{self, NewHacksteadRequest},
    Hackstead, UserId,
};
use log::*;
use sqlx::{PgConnection, PgPool};

pub(crate) mod item;
#[cfg(all(test, feature = "hcor_client"))]
mod test;
pub(crate) mod tile;
pub(crate) use tile::plant;

pub async fn db_insert_hackstead(conn: &mut PgConnection, hs: Hackstead) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO steaders\
            ( steader_id\
            , slack_id\
            , xp\
            , extra_land_plot_count\
            , joined\
            , last_active\
            , last_farm\
            ) \
        VALUES ( $1, $2, $3, $4, $5, $6, $7 )",
        hs.profile.steader_id,
        hs.profile.slack_id,
        hs.profile.xp as i32,
        hs.profile.extra_land_plot_count as i32,
        hs.profile.joined,
        hs.profile.last_active,
        hs.profile.last_farm,
    )
    .execute(&mut *conn)
    .await?;

    for i in hs.inventory {
        item::db_insert_item(&mut *conn, i).await?;
    }
    for t in hs.land {
        tile::db_insert_tile(&mut *conn, t).await?;
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
                "SELECT * FROM steaders WHERE steader_id = $1",
                *uuid
            )
            .fetch_one(pool)
            .await
        }
        UserId::Slack(slack) => {
            sqlx::query_as!(
                hackstead::Profile,
                "SELECT * FROM steaders WHERE slack_id = $1",
                slack
            )
            .fetch_one(pool)
            .await
        }
    }
}

pub async fn uuid_or_lookup(pool: &PgPool, id: &UserId) -> sqlx::Result<uuid::Uuid> {
    match id {
        UserId::Uuid(uuid) | UserId::Both { uuid, .. } => Ok(*uuid),
        UserId::Slack(slack) => {
            sqlx::query!("SELECT steader_id FROM steaders WHERE slack_id = $1", slack)
                .fetch_one(pool)
                .await
                .map(|record| record.steader_id)
        }
    }
}

pub async fn db_get_hackstead(pool: &PgPool, id: &UserId) -> sqlx::Result<hackstead::Hackstead> {
    trace!("getting hackstead from db for {:#?}", id);

    let profile = db_get_profile(pool, id).await?;
    trace!("got profile: {:#?}", profile);

    Ok(hackstead::Hackstead {
        inventory: item::db_get_inventory(pool, profile.steader_id).await?,
        land: tile::db_get_land(pool, profile.steader_id).await?,
        profile,
    })
}

#[get("/hackstead/")]
/// Returns a user's hackstead, complete with Profile, Inventory, and Tiles.
pub async fn get_hackstead(
    db: web::Data<PgPool>,
    user: web::Json<UserId>,
) -> Result<HttpResponse, ServiceError> {
    debug!("servicing get_hackstead request");

    let stead: Hackstead = db_get_hackstead(&db, &*user).await?;
    trace!("got hackstead: {:#?}", stead);

    Ok(HttpResponse::Ok().json(stead))
}

#[post("/hackstead/new")]
pub async fn new_hackstead(
    db: web::Data<PgPool>,
    user: web::Json<NewHacksteadRequest>,
) -> Result<HttpResponse, ServiceError> {
    debug!("servicing new_hackstead request");

    let stead = Hackstead::new_user(user.slack_id.as_ref());
    db_insert_hackstead_transactional(&db, stead.clone()).await?;
    let user_id = UserId::Uuid(stead.profile.steader_id);

    // get a fresh stead because SQL likes to give the timestamps a higher resolution than they are
    // when they go in.
    Ok(HttpResponse::Created().json(&db_get_hackstead(&db, &user_id).await?))
}

#[post("/hackstead/remove")]
pub async fn remove_hackstead(
    db: web::Data<PgPool>,
    user: web::Json<UserId>,
) -> Result<HttpResponse, ServiceError> {
    debug!("servicing remove_hackstead request");

    let stead: Hackstead = db_get_hackstead(&db, &*user).await?;
    match &*user {
        UserId::Uuid(uuid) | UserId::Both { uuid, .. } => {
            sqlx::query!("DELETE FROM steaders * WHERE steader_id = $1", *uuid)
                .execute(&**db)
                .await
        }
        UserId::Slack(slack_id) => {
            sqlx::query!("DELETE FROM steaders * WHERE slack_id = $1", slack_id)
                .execute(&**db)
                .await
        }
    }?;
    debug!(":( removed hackstead: {:#?}", stead);

    Ok(HttpResponse::Ok().json(stead))
}
