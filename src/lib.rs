#![recursion_limit = "256"]
use actix_web::{error::ResponseError, HttpResponse};
use derive_more::Display;

mod hackstead;
pub use hackstead::{db_insert_hackstead, get_hackstead, new_hackstead, remove_hackstead};

pub async fn db_conn() -> Result<sqlx::PgConnection, ServiceError> {
    use sqlx::Connect;

    sqlx::PgConnection::connect(&std::env::var("DATABASE_URL").map_err(|_| {
        log::error!("no DATABASE_URL environment variable set");
        ServiceError::InternalServerError
    })?)
    .await
    .map_err(|e| {
        log::error!("couldn't make db connection: {}", e);
        ServiceError::InternalServerError
    })
}

pub async fn db_pool() -> Result<sqlx::PgPool, ServiceError> {
    sqlx::PgPool::new(&std::env::var("DATABASE_URL").map_err(|_| {
        log::error!("no DATABASE_URL environment variable set");
        ServiceError::InternalServerError
    })?)
    .await
    .map_err(|e| {
        log::error!("couldn't make db pool: {}", e);
        ServiceError::InternalServerError
    })
}

#[derive(Debug, Display)]
/// Hackagotchi's backend API was unable to service you, for any of these reasons.
pub enum ServiceError {
    #[display(fmt = "Internal Server Error")]
    /// Something went wrong on our end.
    InternalServerError,

    #[display(fmt = "Bad Request: {}", _0)]
    /// The request you send us was invalid or not usable for any number of reasons.
    BadRequest(String),

    #[display(fmt = "Unauthorized")]
    /// You aren't allowed to do that.
    Unauthorized,

    #[display(fmt = "No data found")]
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

impl ResponseError for ServiceError {
    fn error_response(&self) -> HttpResponse {
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

impl From<sqlx::Error> for ServiceError {
    fn from(e: sqlx::Error) -> ServiceError {
        log::error!("sqlx error: {}", e);
        match e {
            sqlx::Error::RowNotFound => ServiceError::bad_request("No such data"),
            _ => ServiceError::InternalServerError,
        }
    }
}