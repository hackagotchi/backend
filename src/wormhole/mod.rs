use actix::Addr;
use actix_web::web;
use actix_web_actors::ws;

use hcor::wormhole::EstablishWormholeRequest;

pub mod session;
use session::Session;

pub mod server;
pub use server::Server;

/// This route facilitates establishing a connection to the Wormhole,
/// through which clients can receive messages about their hackstead.
pub async fn establish_wormhole(
    req: actix_web::HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<Server>>,
) -> Result<actix_web::HttpResponse, actix_web::Error> {
    use crate::ServiceError;
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
    let hs = match serde_json::from_str(r_header) {
        Ok(EstablishWormholeRequest { user_id }) => crate::hackstead::fs_get_stead(&user_id),
        Err(e) => Err(ServiceError::bad_request(format!(
            "couldn't parse EstablishWormholeRequest header: {}",
            e
        ))),
    }?;

    ws::start(Session::new(hs, &*srv), &req, stream)
}
