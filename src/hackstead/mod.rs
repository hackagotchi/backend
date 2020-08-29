use crate::{
    wormhole::{self, server},
    ServiceError,
};
use actix_web::{post, web, HttpResponse};
use hcor::{hackstead::NewHacksteadRequest, Hackstead, IdentifiesSteader, IdentifiesUser, UserId};
use log::*;
use std::fs;

#[cfg(all(test, feature = "hcor_client"))]
mod test;

fn user_path(iu: impl IdentifiesUser) -> String {
    match iu.user_id() {
        UserId::Uuid(uuid) | UserId::Both { uuid, .. } => stead_path(uuid),
        UserId::Slack(slack_id) => slack_path(&slack_id),
    }
}

fn stead_path(is: impl IdentifiesSteader) -> String {
    format!("stead/{}.bincode", is.steader_id())
}

fn slack_path(slack: &str) -> String {
    format!("slack/{}.bincode", slack)
}

pub fn fs_get_stead(user_id: impl IdentifiesUser) -> Result<Hackstead, ServiceError> {
    Ok(bincode::deserialize(&fs::read(user_path(user_id))?)?)
}

pub fn fs_put_stead(hs: &Hackstead) -> Result<(), ServiceError> {
    let stead_path = stead_path(hs);
    fs::write(&stead_path, bincode::serialize(hs)?)?;

    if let Some(s) = hs.profile.slack_id.as_ref() {
        fs::hard_link(&stead_path, &slack_path(s))?;
    }

    Ok(())
}

#[post("/hackstead/spy")]
/// Returns a user's hackstead, complete with Profile, Inventory, and Tiles.
pub async fn hackstead_spy(
    user: web::Json<UserId>,
    srv: web::Data<actix::Addr<wormhole::Server>>,
) -> Result<HttpResponse, ServiceError> {
    debug!("servicing get_hackstead request");

    let mut stead = fs_get_stead(&*user)?;
    trace!("got hackstead from fs: {:#?}", stead);

    // if there's already a Session up for this user, that Session will have a much fresher
    // hackstead than we just read off of the fs.
    if let Some(ses) = srv.send(server::GetSession::new(&stead)).await? {
        stead = ses.send(wormhole::session::GetStead).await?;
        trace!("got fresh hackstead from session: {:#?}", stead);
    }

    Ok(HttpResponse::Ok().json(stead))
}

#[post("/hackstead/summon")]
pub async fn hackstead_summon(
    user: web::Json<NewHacksteadRequest>,
) -> Result<HttpResponse, ServiceError> {
    debug!("servicing new_hackstead request");

    let slack = user.slack_id.as_ref();
    let stead = Hackstead::new_user(slack);

    fs_put_stead(&stead)?;

    Ok(HttpResponse::Created().json(&stead))
}

#[post("/hackstead/slaughter")]
pub async fn hackstead_slaughter(user: web::Json<UserId>) -> Result<HttpResponse, ServiceError> {
    debug!("servicing remove_hackstead request");

    let stead = fs_get_stead(&*user)?;
    debug!(":( removing hackstead: {:#?}", stead);

    fs::remove_file(&stead_path(stead.profile.steader_id))?;
    if let Some(slack) = stead.profile.slack_id.as_ref() {
        fs::remove_file(&slack_path(slack))?;
    }

    Ok(HttpResponse::Ok().json(stead))
}
