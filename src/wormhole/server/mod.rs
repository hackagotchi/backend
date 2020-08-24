//! `Server` is an actor. It maintains a list of connected clients.
//! It updates them with Notes when necessary.

use std::collections::HashMap;

#[cfg(feature = "autoclose")]
use actix::AsyncContext;
use actix::{dev::Envelope, Actor, Addr, Context, Handler, Message};
use log::*;

use super::session::{self, Session};
use hcor::{IdentifiesSteader, Note, SteaderId};

mod throw;
pub use throw::ThrowItems;

/// New session is created
#[derive(Message)]
#[rtype(result = "()")]
pub struct Connect(pub SteaderId, pub Addr<Session>);

/// When a client connects, just record who they are associated with
/// the `addr` we can use to send messages to their Session actor.
impl Handler<Connect> for Server {
    type Result = ();

    fn handle(&mut self, Connect(u, a): Connect, _: &mut Context<Self>) {
        self.sessions.insert(u, a);

        info!("user connected - {} users online", self.sessions.len());
    }
}

/// Session disconnected
#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect(pub SteaderId);

impl Handler<Disconnect> for Server {
    type Result = ();

    #[allow(unused_variables)]
    fn handle(&mut self, Disconnect(u): Disconnect, ctx: &mut Context<Self>) {
        self.sessions.remove(&u);

        info!("user logged off - {} users online", self.sessions.len());

        #[cfg(feature = "autoclose")]
        if self.sessions.len() == 0 {
            warn!("ending process in 100 seconds if no more users log on");
            ctx.run_later(std::time::Duration::from_secs(100), |act, _| {
                if act.sessions.len() == 0 {
                    warn!("ending process");
                    std::process::exit(0)
                }
            });
        }
    }
}

/// Send note to all users
#[derive(Message)]
#[rtype(result = "()")]
pub struct BroadcastNote(pub Note);

impl Handler<BroadcastNote> for Server {
    type Result = ();

    fn handle(&mut self, BroadcastNote(n): BroadcastNote, _: &mut Context<Self>) {
        self.broadcast_note(&n)
    }
}

/// Get the Session associated with a user, if there is one currently registered for them.
#[derive(Message)]
#[rtype(result = "Option<Addr<Session>>")]
pub struct GetSession(pub SteaderId);

impl GetSession {
    pub fn new(iu: impl IdentifiesSteader) -> Self {
        Self(iu.steader_id())
    }
}

impl Handler<GetSession> for Server {
    type Result = Option<Addr<Session>>;

    fn handle(&mut self, GetSession(sr): GetSession, _: &mut Context<Self>) -> Self::Result {
        self.sessions.get(&sr).cloned()
    }
}

/// Get the Session associated with a user, if there is one currently registered for them.
#[derive(Message)]
#[rtype(result = "()")]
pub struct HandleEnvelope(pub Envelope<Server>);

impl Handler<HandleEnvelope> for Server {
    type Result = ();

    fn handle(&mut self, HandleEnvelope(mut e): HandleEnvelope, ctx: &mut Context<Self>) {
        use actix::dev::EnvelopeProxy;
        e.handle(self, ctx)
    }
}

/// `Server` manages connected clients and is responsible for dispatching Notes to them.
#[derive(Default)]
pub struct Server {
    sessions: HashMap<SteaderId, Addr<Session>>,
}

impl Server {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Send note to all users
    fn broadcast_note(&self, note: &Note) {
        for addr in self.sessions.values() {
            addr.do_send(session::SendNote(note.clone()));
        }
    }
}

impl Actor for Server {
    /// Simple Context: we just need ability to communicate with other actors.
    type Context = Context<Self>;
}
