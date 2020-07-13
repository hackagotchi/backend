use actix_web::HttpRequest;
use actix_web::middleware::{Middleware, Started};
use hcor::errors::ServiceError;
use actix_web::Result;
use std::env;

use hmac::{Hmac, Mac, NewMac};


pub struct VerifySignature;

impl <S> Middleware<S> for VerifySignature {
    fn start(&self, req: &mut HttpRequest<S>) -> Result<Started> {
        use std::io::Read;

        let r = req.clone();
        let s = r.headers()
            .get("X-Signature")
            .ok_or(ServiceError::Unauthorized)?
            .to_str()
            .map_err(ServiceError::Unauthorized)?;

        let (_, sig) = s.split_at(5);

        let mut mac = Hmac::<Sha256>::new_varkey(String::as_bytes(std::env("SECERT_KEY").unwrap_or("changemepls")));


        let mut body = String::new();
        req.read_to_string(&mut body)
            .map_err(ServiceError::InternalServerError)?;

        mac.update(String::as_bytes(sig));

        mac.verify(String::as_bytes(body));

    }
}