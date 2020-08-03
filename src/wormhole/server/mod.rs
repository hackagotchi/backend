//! `Server` is an actor. It maintains a list of connected clients.
//! It updates them with Notes when necessary.

use std::collections::HashMap;

use actix::{Actor, Addr, AsyncContext, Context, Handler, Message};
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    session::{self, Session},
    SenderBundle,
};
use hcor::{wormhole::Note, UPDATE_INTERVAL};

mod update;
use update::update_stead;

/// New session is created
#[derive(Message)]
#[rtype(result = "()")]
pub(super) struct Connect(pub(super) Uuid, pub(super) Addr<Session>);

/// When a client connects, just record who they are associated with
/// the `addr` we can use to send messages to their Session actor.
impl Handler<Connect> for Server {
    type Result = ();

    fn handle(&mut self, Connect(u, a): Connect, _: &mut Context<Self>) {
        self.sessions.insert(u, a);
    }
}

/// Session disconnected
#[derive(Message)]
#[rtype(result = "()")]
pub(super) struct Disconnect(pub(super) Uuid);

impl Handler<Disconnect> for Server {
    type Result = ();

    fn handle(&mut self, Disconnect(u): Disconnect, _: &mut Context<Self>) {
        self.sessions.remove(&u);
    }
}

/// Send note to all users
#[derive(Message)]
#[rtype(result = "()")]
pub(super) struct BroadcastNote(pub(super) Note);

impl Handler<BroadcastNote> for Server {
    type Result = ();

    fn handle(&mut self, BroadcastNote(n): BroadcastNote, _: &mut Context<Self>) {
        self.broadcast_note(n)
    }
}

/// `Server` manages connected clients and is responsible for dispatching Notes to them.
pub struct Server {
    sessions: HashMap<Uuid, Addr<Session>>,
    db: PgPool,
}

impl Server {
    pub fn new(db: PgPool) -> Self {
        Self {
            sessions: HashMap::new(),
            db,
        }
    }

    /// Send note to all users
    fn broadcast_note(&self, note: Note) {
        for addr in self.sessions.values() {
            addr.do_send(session::SendNote(note.clone()));
        }
    }

    fn update(&mut self, ctx: &mut Context<Self>) {
        use actix::fut::WrapFuture;
        use futures::FutureExt;

        ctx.run_interval(UPDATE_INTERVAL, |act, ctx| {
            for (uuid, send) in act.sessions.clone() {
                ctx.wait(
                    update_stead(
                        act.db.clone(),
                        SenderBundle {
                            uuid,
                            send: send.recipient(),
                            broadcast: ctx.address().recipient(),
                        },
                    )
                    .map(|res| {
                        if let Err(e) = res {
                            log::error!("error updating hackstead: {}", e);
                        }
                    })
                    .into_actor(act),
                );
            }
        });
    }
}

impl Actor for Server {
    /// Simple Context: we just need ability to communicate with other actors.
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.update(ctx)
    }
}
