use actix::Addr;
use actix_web::{http::HeaderValue, web};
use actix_web_actors::ws;

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

    fn json_header<D: serde::de::DeserializeOwned>(
        header_name: &str,
        err: &str,
        req: &actix_web::HttpRequest,
    ) -> Result<D, ServiceError> {
        let header_str = match req.headers().get(header_name).map(HeaderValue::to_str) {
            Some(Ok(s)) => Ok(s),
            Some(Err(e)) => Err(ServiceError::bad_request(&format!(
                "error reading {}: {}",
                header_name, e
            ))),
            None => Err(ServiceError::bad_request(&format!(
                "please supply a {} header which contains {}",
                header_name, err
            ))),
        }?;

        serde_json::from_str(header_str).map_err(|e| {
            ServiceError::bad_request(&format!(
                "couldn't parse {} header (got '{}'): {}",
                header_name, header_str, e
            ))
        })
    }

    let hs = crate::hackstead::fs_get_stead(&json_header::<hcor::UserId>(
        "WormholeUser",
        "valid UserId JSON",
        &req,
    )?)?;
    let orifice =
        json_header::<session::Orifice>("WormholeOrifice", "either 'Bincode' or 'Json'", &req)?;

    ws::start(Session::new(hs, &*srv, orifice), &req, stream)
}
