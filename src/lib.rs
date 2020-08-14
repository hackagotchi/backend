//! # API Flow
//! Hackagotchi's backend uses a combination of HTTP and websockets to communicate with clients.
//! In the following code samples, `SERVER_URL` may be replaced with i.e. `http://localhost:8000`.
//!
//! ## Registering a user
//! Registering a user can be performed by sending an HTTP POST request to `/api/hackstead/summon`.
//! The body of the request should be JSON in the form of a
//! [`NewHacksteadRequest`](hcor::hackstead::NewHacksteadRequest). The response will be JSON in the
//! form of a [`Hackstead`](hcor::Hackstead).
//!
//! ```
//! # use serde_json::json;
//! # use hcor::hackstead::{NewHacksteadRequest, Hackstead};
//! # #[cfg(feature="awc_test")]
//! # #[actix_rt::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # use awc::Client;
//! let req = NewHacksteadRequest { slack_id: Some("U14MB0B".to_string()) };
//!
//! // the equivalent request, as JSON
//! assert_eq!(
//!     serde_json::from_value::<NewHacksteadRequest>(json!({
//!         "slack_id": "U14MB0B"
//!     }))?,
//!     req,
//! );
//!
//! // sending it to the server
//! let hs: Hackstead = Client::default()
//!     .post(concat!(env!("SERVER_URL"), "/api/hackstead/summon"))
//!     .send_json(&req)
//!     .await
//!     .expect("couldn't POST /hackstead/summon")
//!     .json()
//!     .await
//!     .expect("couldn't parse hackstead");
//! # // this prevents the slaughtering in a few lines from failing
//! # hcor::wormhole::connect(&hs).await?;
//!
//! assert_eq!(hs.profile.slack_id, req.slack_id);
//!
//! // remove the hackstead with the Rust API bindings
//! hs.slaughter().await?;
//! # Ok(())
//! # }
//! ```
//! ### Removing a user
//! One can remove a user by way of sending a HTTP POST request to `/api/hackstead/slaughter`.
//! The body of the request should be JSON in the form of a [`UserId`](hcor::UserId).
//! The response will be JSON in the form of a [`Hackstead`](hcor::Hackstead).
//! ```
//! # use serde_json::json;
//! # use hcor::{UserId, Hackstead, IdentifiesUser};
//! # #[cfg(all(feature="awc_test", feature="hcor_client"))]
//! # #[actix_rt::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # use awc::Client;
//! // make a hackstead using the Rust API bindings
//! let hs = Hackstead::register().await?;
//!
//! // a UserId like the one we're about to POST, in JSON
//! assert_eq!(
//!     serde_json::from_value::<UserId>(json!({
//!         "Uuid": hs.profile.steader_id
//!     }))?,
//!     hs.user_id()
//! );
//!
//! // kill 'em!
//! let dedsted: Hackstead = Client::default()
//!     .post(concat!(env!("SERVER_URL"), "/api/hackstead/slaughter"))
//!     .send_json(&hs.user_id())
//!     .await
//!     .expect("couldn't POST /hackstead/summon")
//!     .json()
//!     .await
//!     .expect("couldn't parse hackstead");
//!
//! // the hackstead slaughter returns should have the same id
//! assert_eq!(dedsted.user_id(), hs.user_id());
//! # Ok(())
//! # }
//! ```
//!
//! ## Acting as a user
//! In order to receive notifications (or to efficiently act on a user's behalf),
//! you must establish a websockets connection with the server. This is accomplished
//! by way of the `/api/wormhole` route. When connecting, the `WormholeUser`
//! header must be set to JSON in the form of an [`UserId`](hcor::UserId).
//!
//! Another header, `WormholeOrifice`, must be set to either `"Json"` or `"Bincode"`
//! (remember to include the quotes) to indicate whether messages should be formatted
//! in JSON or mozilla's [Bincode](https://github.com/servo/bincode). As a schemaless binary
//! format, [Bincode](https://github.com/servo/bincode) is a great deal more efficient,
//! but JSON is still provided for greater accessibility, as usable
//! [Bincode](https://github.com/servo/bincode) implementations are few and far between outside
//! of the Rust ecosystem. [`EditNote`s](hcor::wormhole::EditNote) in particular are a
//! great deal more efficient when encoded as [Bincode](https://github.com/servo/bincode).
//!
//! Input into the websockets connection (henceforth referred to as "the wormhole")
//! should take the form of an [`AskMessage`](hcor::wormhole::AskMessage).
//! Messages coming to the connected client through the wormhole will take the form of
//! [`Note`s](hcor::Note).
//!
//! If [Bincode](https://github.com/server/bincode) is specified when the wormhole connection is
//! being established, one should expect websockets messages only of the binary, ping, pong, or close
//! varieties, and every binary message coming from the server should be a valid [`Note`](hcor::Note)
//! encoded as [Bincode](https://github.com/server/bincode).  All messages sent to the server should
//! be much the same, with the exception that all binary messages should be valid
//! [`AskMessage`s](hcor::wormhole::AskMessage) encoded as [Bincode](https://github.com/server/bincode).
//!
//! Likewise, if one specifies `Json` as the `WormholeOrifice`, one should expect websockets
//! messages only of the text, ping, pong, or close varieties, and all text messages coming from
//! the server should be valid [`Note`s](hcor::Note) encoded as JSON. All messages sent to the server
//! should be much the same, with the exception that all text messages should be valid
//! [`AskMessage`s](hcor::wormhole::AskMessage) encoded as JSON.
//! ```
//! # use serde_json::json;
//! # use hcor::{UserId, Hackstead, IdentifiesUser};
//! # #[cfg(all(feature="awc_test", feature="hcor_client"))]
//! # #[actix_rt::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # use awc::Client;
//! // make a hackstead using the Rust API bindings
//! let hs = Hackstead::register_with_slack("U14M3V3").await?;
//!
//! // connect as 'em!
//! Client::default()
//!     .ws(concat!(env!("SERVER_URL"), "/api/wormhole"))
//!     .header("WormholeUser", r#"{ "Slack": "U14M3V3" }"#)
//!     .header("WormholeOrifice", "\"Json\"")
//!     .connect()
//!     .await
//!     .expect("couldn't make wormhole connection");
//!
//! // kill the stead
//! # // this prevents the slaughtering in a few lines from failing
//! # hcor::wormhole::connect(&hs).await?;
//! hs.slaughter().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Asking for Trouble
//! When requesting that the server perform an action through the wormhole, clients may
//! submit an arbitrary `ask_id` alongside an [`Ask`](hcor::Ask).
//!
//! The [`Ask`](hcor::Ask) enumeration covers each possible request the client may issue to the
//! server. Typically, these are filled with various identifiers indicating which objects the
//! client intends for the server to act upon.
//!
//! Intended to allow clients to track and identify each of their requests, `ask_id`s are largely
//! ignored by the server. Responses to [`Ask`s](hcor::Ask) (covered in *Ask and Ye Shall Receive*)
//! contain the `ask_id` of the [`Ask`s](hcor::Ask) they are intended to respond to. This behavior
//! allows clients to track the resolution of individual [`Ask`s](hcor::Ask) and make sure each of
//! them is addressed by the server.
//!
//! ```
//! # use uuid::Uuid;
//! # use serde_json::json;
//! # use hcor::{id::ItemId, wormhole::{Ask, AskMessage, ItemAsk::Hatch}};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let uuid = Uuid::new_v4();
//! assert_eq!(
//!     serde_json::from_value::<AskMessage>(json!({
//!         "ask_id": 1337,
//!         "ask": { "Item": { "Hatch": {
//!             "hatchable_item_id": uuid
//!         }}}
//!     }))?,
//!     AskMessage {
//!         ask_id: 1337,
//!         ask: Ask::Item(Hatch { hatchable_item_id: ItemId(uuid) })
//!     }
//! );
//! # Ok(())
//! # }
//! ```
//!
//! ### Ask and Ye Shall Receive
//! [`Ask`s](hcor::Ask) may be faulty for any number of reasons. Alternatively,
//! [`Ask`s](hcor::Ask) may need to return some information to the user. (In the example below,
//! the result of an hatching an item provides a new [`Item`](hcor::Item)).
//! Disregarding either of these, you may simply want to offer the user some sort of indication that
//! their action has been accepted and completed on the server, should that occur. When any of these
//! are desired, you may listen for [`AskedNote`s](hcor::wormhole::AskedNote) from the wormhole,
//! which will offer an indication that the server received your [`Ask`](hcor::Ask) accompanied by
//! descriptions of error cases or custom data provided that the [`Ask`](hcor::Ask) succeeds.
//! See the documentation on [`AskedNote`](hcor::wormhole::AskedNote)
//! for an enumeration of possible responses, common error cases, and more information.
//!
//! ```
//! # use serde_json::json;
//! # use uuid::Uuid;
//! # use hcor::{
//! #   id::{SteaderId, ItemId},
//! #   item::{Acquisition, Item, LoggedOwner},
//! #   wormhole::{AskedNote::ItemHatchResult, Note, AskMessage}
//! # };
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let item_id = ItemId(Uuid::new_v4());
//! # let owner_id = SteaderId(Uuid::new_v4());
//! assert_eq!(
//!     serde_json::from_value::<Note>(json!({
//!         "Asked": {
//!             "ask_id": 1337,
//!             "note": { "ItemHatchResult": {
//!                 "Ok": [
//!                     {
//!                         "item_id": item_id,
//!                         "owner_id": owner_id,
//!                         "archetype_handle": 5,
//!                         "ownership_log": [
//!                             {
//!                                 "logged_owner_id": owner_id,
//!                                 "acquisition": "Farmed",
//!                                 "owner_index": 0,
//!                             }
//!                         ]
//!                     }
//!                 ]
//!             }}
//!         }
//!     }))?,
//!     Note::Asked {
//!         ask_id: 1337,
//!         note: ItemHatchResult(Ok(vec![
//!             Item {
//!                 item_id,
//!                 owner_id,
//!                 archetype_handle: 5,
//!                 gotchi: None,
//!                 ownership_log: vec![
//!                     LoggedOwner {
//!                         logged_owner_id: owner_id,
//!                         owner_index: 0,
//!                         acquisition: Acquisition::Farmed
//!                     }
//!                 ]
//!             }
//!         ]))
//!     }
//! );
//! // or perhaps the Ask fails,
//! assert_eq!(
//!     serde_json::from_value::<Note>(json!({
//!         "Asked": {
//!             "ask_id": 1337,
//!             "note": { "ItemHatchResult": {
//!                 "Err": "provided item ITEM_ID, which, as a Warp Powder[5], \
//!                             is not configured to be hatched"
//!             }}
//!         }
//!     }))?,
//!     Note::Asked {
//!         ask_id: 1337,
//!         note: ItemHatchResult(Err(
//!             "provided item ITEM_ID, which, as a Warp Powder[5], \
//!                 is not configured to be hatched".to_string()
//!         ))
//!     }
//! );
//! # Ok(())
//! # }
//! ```
//!
//! #### [`Ask`](hcor::Ask); don't [`Beg`](hcor::wormhole::Beg)
//! Having read (and hopefully understood) this description of the behavior of this
//! [`Ask`](hcor::Ask) and [`AskedNote`](hcor::wormhole::AskedNote) system, you may find yourself
//! wondering what real advantage this offers over traditional HTTP requests. Sending messages
//! through an established websockets connection may offer a bit less overhead and higher message
//! throughput than simply using HTTP requests, but it hardly seems worthwhile, at least for a simple
//! client prototype.
//!
//! For this reason, one may POST to the `/api/beg` route, which takes a JSON body
//! in the form of a [`Beg`](hcor::wormhole::Beg) as input and returns JSON in the form of an
//! [`AskedNote`](hcor::wormhole::AskedNote). The [`Beg`](hcor::wormhole::Beg) itself is simply an
//! [`Ask`](hcor::Ask) paired with a [`SteaderId`](hcor::id::SteaderId) to identify the user to ask
//! for.
//!
//! Note that even with this route, it is still necessary to establish a wormhole to listen
//! for [`RudeNote`s](hcor::wormhole::RudeNote) and [`EditNote`s](hcor::wormhole::EditNote), so
//! that you can keep your local hackstead data in sync with what the server has, and show the user
//! when i.e. they are sent items from other users, their yields or crafts finish, effects wear
//! off, etc. One is spared only the effort of sending messages into the wormhole and pairing them
//! with their responses.
//! ```
//! # use serde_json::json;
//! # use hcor::{Hackstead, wormhole::{Beg, Ask, AskedNote::self}};
//! # #[cfg(all(feature="awc_test", feature="hcor_client"))]
//! # #[actix_rt::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # use awc::Client;
//! // make a hackstead using the Rust API bindings
//! let hs = Hackstead::register().await?;
//!
//! // use the beg API to give this user free xp
//! let total_xp_note: AskedNote = Client::default()
//!     .post(concat!(env!("SERVER_URL"), "/api/beg"))
//!     .send_json(&Beg {
//!         steader_id: hs.profile.steader_id,
//!         ask: Ask::KnowledgeSnort { xp: 100 }
//!     })
//!     .await
//!     .expect("couldn't send /api/beg")
//!     .json()
//!     .await
//!     .expect("couldn't parse /api/beg");
//!
//! // show the resulting AskedNote as JSON
//! assert_eq!(
//!     serde_json::from_value::<AskedNote>(json!({
//!         "KnowledgeSnortResult": { "Ok": 100 }
//!     }))?,
//!     total_xp_note
//! );
//!
//! // remove the hackstead with the Rust API bindings
//! hs.slaughter().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## [`Note`](hcor::Note)y Behavior
//! [`AskedNote`s](hcor::wormhole::AskedNote) are rather polite; you only receive them when you
//! explicitly ask for them. Unforunately, not all [`Note`](hcor::Note)s behave in this manner;
//! there are also notes which, rather rudely, barge in unannounced, when no one explicitly
//! *Ask*ed for them to be there. These [`RudeNote`s](hcor::wormhole::RudeNote) may sneak up on
//! you at any moment, and I believe you'd best be on the lookout for them and their
//! [`Note`](hcor::Note)-y Behavior.
//!
//! [`RudeNote`s](hcor::wormhole::RudeNote) are sent to clients through the wormhole to notify them
//! about events that are not directly under their control. The receival of items from other users,
//! ([`ItemThrowReceipt`](hcor::wormhole::RudeNote::ItemThrowReceipt)), the completion of plant
//! yields ([`YieldFinish`](hcor::wormhole::RudeNote::YieldFinish)), and even the wearing off of
//! items rubbed onto a plant ([`RubEffectFinish`](hcor::wormhole::RudeNote::RubEffectFinish)) are
//! all examples of events that users cannot request the immediate completion of, and therefore
//! have [`RudeNote`s](hcor::wormhole::RudeNote).
//! ```
//! # use uuid::Uuid;
//! # use serde_json::json;
//! # use hcor::{id::TileId, plant, wormhole::{Note, RudeNote::RubEffectFinish}};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let tile_id = TileId(Uuid::new_v4());
//! # let effect_id = plant::EffectId(Uuid::new_v4());
//! assert_eq!(
//!     serde_json::from_value::<Note>(json!({
//!         "Rude": { "RubEffectFinish": {
//!             "tile_id": tile_id,
//!             "effect": {
//!                 "effect_id": effect_id,
//!                 "item_archetype_handle": 5,
//!                 "effect_archetype_handle": 0,
//!             }
//!         }}
//!     }))?,
//!     Note::Rude(RubEffectFinish {
//!         tile_id,
//!         effect: plant::Effect {
//!             effect_id,
//!             item_archetype_handle: 5,
//!             effect_archetype_handle: 0,
//!         }
//!     })
//! );
//! # Ok(())
//! # }
//! ```
//! ## Keeping your steads Farm Fresh™
//! It is assumed that client implementations keep a copy of a user's
//! [`Hackstead`](hcor::Hackstead) in memory locally, to reference as application state
//! when rendering and facilitating user interaction.
//!
//! [`AskedNote`s](hcor::wormhole::AskedNote) and [`RudeNote`s](hcor::wormhole::RudeNote)
//! may modify [`Hackstead`s](hcor::Hackstead) in ways it it is vital to keep track of and
//! display to the user. For this reason, a third variety of note,
//! [`EditNote`s](hcor::wormhole::EditNote), are sent to indicate the need to change the
//! [`Hackstead`](hcor::Hackstead) data clients store locally, and even indicate how to change
//! your data.
//!
//! [`EditNote`s](hcor::wormhole::EditNote) themselves come in two different varieties, one for
//! each possible `WormholeOrifice`. Clients should expect to receive the variety that matches the
//! one they specified when establishing their wormhole connection, and any variation from that
//! should be regarded as a bug.
//!
//! It is not strictly necessary, however, that clients apply the list of atomic changes the server
//! supplies to their local data; they can simply ignore the data the server sends inside of the
//! [`EditNote`](hcor::wormhole::EditNote) and use the `/api/hackstead/spy` route to request brand new
//! [`Hackstead`](hcor::Hackstead) data. This would be grossly inefficient for larger hacksteads,
//! as it would be likely that the majority of the data supplied by the server would not have been
//! changed in the actual edit, but may still be worth implementing for the sake of rapid prototyping.
//! ```
//! # use uuid::Uuid;
//! # use serde_json::json;
//! # use hcor::{id::TileId, plant, wormhole::{Note, EditNote}};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let tile_id = TileId(Uuid::new_v4());
//! # let effect_id = plant::EffectId(Uuid::new_v4());
//! assert_eq!(
//!     serde_json::from_value::<Note>(json!({
//!         "Edit": { "Json":
//!             // Rust's r#""# string delimiting syntax is used here to avoid having to escape
//!             // each of the quotation marks in the string, as would have to be done in real JSON.
//!             r#"[{"Enter":{"Field":"land"}},{"Enter":{"CollectionIndex":0}},{"Enter":{"Field":"plant"}},{"Remove":1},"Exit","Exit"]"#
//!         }
//!     }))?,
//!     Note::Edit(EditNote::Json(
//!         r#"[{"Enter":{"Field":"land"}},{"Enter":{"CollectionIndex":0}},{"Enter":{"Field":"plant"}},{"Remove":1},"Exit","Exit"]"#.to_string()
//!     ))
//! );
//! # Ok(())
//! # }
//! ```
//! ### Keeping yourself `POST`-ed™
//! Intended to be used to retrieve a previously-registered user's [`Hackstead`](hcor::Hackstead),
//! the HTTP POST route `/api/hackstead/spy` takes a JSON body in the form of a [`UserId`](hcor::UserId),
//! and returns a JSON response in the form of the specified user's [`Hackstead`](hcor::Hackstead).
//!
//! As mentioned before, naive client implementations may also (ab)use this route for the purpose
//! of keeping a [`Hackstead`](hcor::Hackstead) in sync with the server.
//!
//! The unorthodox name and HTTP method are intended to ward off frequent usage, as should be
//! avoided in efficient client implementations (as well as to justify the pun in the header for
//! this section of the documentation).
//! ```
//! # use hcor::UserId;
//! # #[cfg(feature="awc_test")]
//! # #[actix_rt::main]
//! # async fn main() {
//! # use awc::{Client, http::StatusCode};
//! assert_eq!(
//!     Client::default()
//!         .post(concat!(env!("SERVER_URL"), "/api/hackstead/spy"))
//!         .send_json(&UserId::Slack("I don't exist".to_string()))
//!         .await
//!         .expect("couldn't POST hackstead/spy")
//!         .status(),
//!     StatusCode::NOT_FOUND,
//! );
//! # }

#![recursion_limit = "256"]
#![deny(clippy::pedantic)]
#![allow(clippy::enum_glob_use)]
#![allow(clippy::many_single_char_names)]
//#![forbid(missing_docs)]
#![forbid(unsafe_code)]
#![forbid(intra_doc_link_resolution_failure)]
use actix_web::{error::ResponseError, HttpResponse};
use log::*;
use std::fmt;

#[cfg(any(feature = "csv_migration", feature = "webserver"))]
mod hackstead;
#[cfg(feature = "csv_migration")]
pub use hackstead::fs_put_stead;
#[cfg(feature = "webserver")]
pub use hackstead::{hackstead_slaughter, hackstead_spy, hackstead_summon};

#[cfg(feature = "webserver")]
mod wormhole;
#[cfg(feature = "webserver")]
pub use wormhole::{establish_wormhole, Server as WormholeServer};

#[cfg(feature = "webserver")]
#[actix_web::post("/beg")]
pub async fn beg(
    beg: actix_web::web::Json<hcor::wormhole::Beg>,
    srv: actix_web::web::Data<actix::Addr<wormhole::Server>>,
) -> Result<actix_web::HttpResponse, ServiceError> {
    debug!("servicing beg request");
    let hcor::wormhole::Beg { ask, steader_id } = beg.clone();

    Ok(HttpResponse::Ok().json(
        srv.send(wormhole::server::GetSession::new(&steader_id))
            .await?
            .ok_or_else(|| {
                ServiceError::BadRequest(format!(
                    "{} has no active wormhole connection!",
                    steader_id
                ))
            })?
            .send(wormhole::session::DoAsk(ask))
            .await??,
    ))
}


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
    /// let is_br = matches!(br, ServiceError::BadRequest(_));
    /// assert!(is_br, "ServiceError::bad_request() should always return a BadRequest variant");
    /// ```
    pub fn bad_request<T: ToString + ?Sized>(t: &T) -> Self {
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
        ServiceError::bad_request(&e)
    }
}
