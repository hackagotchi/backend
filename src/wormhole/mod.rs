use actix::{Addr, Recipient};
use actix_web::web;
use actix_web_actors::ws;
use uuid::Uuid;

use hcor::wormhole::{EstablishWormholeRequest, Note};

mod session;
use session::Session;

pub mod server;
pub use server::Server as WormholeServer;
use server::Server;

/// This route facilitates establishing a connection to the Wormhole,
/// through which clients can receive messages about their hackstead.
pub async fn establish_wormhole(
    req: actix_web::HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<Server>>,
    db: web::Data<sqlx::PgPool>,
) -> Result<actix_web::HttpResponse, actix_web::Error> {
    use crate::{uuid_or_lookup, ServiceError};
    log::debug!("servicing establish_wormhole request");

    let r_header = match req.headers().get("EstablishWormholeRequest").map(|h| h.to_str()) {
        Some(Ok(s)) => Ok(s),
        Some(Err(e)) => Err(ServiceError::bad_request(format!(
            "error reading EstablishWormholeRequest: {}",
            e
        ))),
        None => Err(ServiceError::bad_request(
            "please supply an EstablishWormholeRequest header which contains a valid UserId as JSON."
        )),
    }?;
    let uuid = match serde_json::from_str(r_header) {
        Ok(EstablishWormholeRequest { user_id }) => {
            uuid_or_lookup(&db, &user_id).await.map_err(|e| e.into())
        }
        Err(e) => Err(ServiceError::bad_request(format!(
            "couldn't parse EstablishWormholeRequest header: {}",
            e
        ))),
    }?;

    ws::start(Session::new(uuid, &*srv), &req, stream)
}

/// Contains addresses to Actors which allow one to send messages to all connected clients, or to
/// a specific client. It also contains a Uuid indicating which client that this Bundle gives you
/// direct access to.
struct SenderBundle {
    broadcast: Recipient<server::BroadcastNote>,
    send: Recipient<session::SendNote>,
    uuid: Uuid,
}
impl SenderBundle {
    fn broadcast(&self, note: Note) {
        drop(self.broadcast.do_send(server::BroadcastNote(note)))
    }

    fn send(&self, note: Note) {
        drop(self.send.do_send(session::SendNote(note)))
    }
}
