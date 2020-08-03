use std::time::Instant;

use actix::{Actor, ActorContext, Addr, AsyncContext, Handler, StreamHandler};
use actix_web_actors::ws;
use uuid::Uuid;

pub use super::server::{self, Server};
use hcor::wormhole::{Note, CLIENT_TIMEOUT, HEARTBEAT_INTERVAL};

/// An individual user's session with the Server. It contains an address to that server so
/// that it can notify it when it connects/disconnects, and the server keeps an address to it so
/// that it can give it Notes to send down the Wormhole (or in this case, Websocket) to be displayed
/// to the user.
pub struct Session {
    uuid: Uuid,
    hb: Instant,
    addr: Addr<Server>,
}

impl Session {
    /// Constructs a new wormhole session from the uuid of the user who owns this session
    /// and an address which points to the Server.
    pub fn new(uuid: Uuid, srv: &Addr<Server>) -> Self {
        Self {
            uuid,
            hb: Instant::now(),
            addr: srv.clone(),
        }
    }

    /// This function is responsible for sending messages to the client to assure that we're still
    /// active and operational, and checking that the client has sent us a similar message recently
    /// to assure that they're still online. If they haven't sent any such message in a certain
    /// amount of time, we drop their connection and their session ends.
    fn heartbeat(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                log::warn!("Websocket Client heartbeat failed, disconnecting!");
                ctx.stop();
            } else {
                ctx.ping(b"");
            }
        });
    }
}

impl Actor for Session {
    type Context = ws::WebsocketContext<Self>;

    /// When a wormhole session starts, we want to immediately begin the hearbeat system, and
    /// send the server an address to this session, through which the server can begin sending us
    /// Notes.
    fn started(&mut self, ctx: &mut Self::Context) {
        // probably important to kick this off as soon as possible
        self.heartbeat(ctx);

        let addr = ctx.address();
        self.addr.do_send(server::Connect(self.uuid, addr));
    }

    fn stopping(&mut self, _: &mut Self::Context) -> actix::Running {
        // notify chat server
        self.addr.do_send(server::Disconnect(self.uuid));
        actix::Running::Stop
    }
}

/// Send out what the server tells us to send out, it's not hard :P
#[derive(actix::Message)]
#[rtype(result = "()")]
pub(super) struct SendNote(pub(super) Note);

impl Handler<SendNote> for Session {
    type Result = ();

    fn handle(&mut self, SendNote(note): SendNote, ctx: &mut Self::Context) {
        match serde_json::to_string(&note) {
            Ok(json) => ctx.text(json),
            Err(e) => log::error!("couldn't serialize Note: {}", e),
        }
    }
}

/// WebSocket message handler
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for Session {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        use ws::Message::*;

        // current error policy is: you fuck up once, we drop your session.
        // crude, but effective.
        let msg = match msg {
            Err(e) => {
                log::error!("dropping client, websocket error: {}", e);
                ctx.stop();
                return;
            }
            Ok(msg) => msg,
        };

        match msg {
            Ping(msg) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Pong(_) => self.hb = Instant::now(),
            _ => {}
        }
    }
}
