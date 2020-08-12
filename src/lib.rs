#![recursion_limit = "256"]
use actix_web::{error::ResponseError, HttpResponse};
use log::*;
use std::fmt;

mod hackstead;
pub use hackstead::fs_put_stead;
pub use hackstead::{get_hackstead, new_hackstead, remove_hackstead};

pub mod wormhole;
pub use wormhole::establish_wormhole;

#[derive(Debug)]
/// Hackagotchi's backend API was unable to service you, for any of these reasons.
pub enum ServiceError {
    /// Something went wrong on our end.
    InternalServerError,
    /// The request you send us was invalid or not usable for any number of reasons.
    BadRequest(String),
    /// You aren't allowed to do that.
    Unauthorized,
    /// We don't know anything about what you requested.
    NoData,
}
impl ServiceError {
    /// A shortcut for making a `ServiceError::BadRequest`.
    /// ```
    /// use backend::ServiceError;
    ///
    /// let br = ServiceError::bad_request("you're bad and you should feel bad");
    /// let is_br = match br {
    ///     ServiceError::BadRequest(_) => true,
    ///     _ => false,
    /// };
    /// assert!(is_br, "ServiceError::bad_request() should always return a BadRequest variant");
    /// ```
    pub fn bad_request<T: ToString>(t: T) -> Self {
        Self::BadRequest(t.to_string())
    }
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ServiceError::*;

        match self {
            InternalServerError => write!(f, "Internal Server Error"),
            BadRequest(s) => write!(f, "Bad Request: {}", s),
            Unauthorized => write!(f, "Unauthorized"),
            NoData => write!(f, "No data found"),
        }
    }
}

impl std::error::Error for ServiceError {}

impl ResponseError for ServiceError {
    fn error_response(&self) -> HttpResponse {
        error!("{}", self);
        match self {
            ServiceError::InternalServerError => {
                HttpResponse::InternalServerError().body("Internal Server Error. Try again later.")
            }
            ServiceError::BadRequest(s) => HttpResponse::BadRequest().body(s),
            ServiceError::Unauthorized => HttpResponse::Unauthorized().body("Unauthorized"),
            ServiceError::NoData => HttpResponse::NotFound().body("Data not found"),
        }
    }
}

impl From<serde_json::Error> for ServiceError {
    fn from(e: serde_json::Error) -> ServiceError {
        error!("serde json error: {}", e);
        ServiceError::InternalServerError
    }
}

impl From<std::io::Error> for ServiceError {
    fn from(e: std::io::Error) -> ServiceError {
        error!("io error: {}", e);
        match e.kind() {
            std::io::ErrorKind::NotFound => ServiceError::NoData,
            _ => ServiceError::InternalServerError,
        }
    }
}

impl From<actix::MailboxError> for ServiceError {
    fn from(e: actix::MailboxError) -> ServiceError {
        error!("mailbox error: {}", e);
        ServiceError::InternalServerError
    }
}

impl From<hcor::ConfigError> for ServiceError {
    fn from(e: hcor::ConfigError) -> ServiceError {
        ServiceError::bad_request(e)
    }
}
