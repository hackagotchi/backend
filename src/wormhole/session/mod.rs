use std::time::Instant;

use actix::{Actor, ActorContext, Addr, AsyncContext, Handler, MailboxError, StreamHandler};
use actix_web_actors::ws;
use log::*;

use super::server::{self, Server};
use hcor::{
    wormhole::{AskMessage, CLIENT_TIMEOUT, HEARTBEAT_INTERVAL},
    Hackstead, IdentifiesSteader, Note, UPDATE_INTERVAL,
};

mod item;
mod ticker;
mod tile;
use tile::plant;

fn send_note(ctx: &mut SessionContext, note: &Note) {
    match serde_json::to_string(note) {
        Ok(json) => ctx.text(json),
        Err(e) => error!("couldn't serialize Note: {}", e),
    }
}

/// An individual user's session with the Server. It contains an address to that server so
/// that it can notify it when it connects/disconnects, and the server keeps an address to it so
/// that it can give it Notes to send down the Wormhole (or in this case, Websocket) to be displayed
/// to the user.
pub struct Session {
    hackstead: Hackstead,
    heartbeat: Instant,
    server: Addr<Server>,
    ticker: ticker::Ticker,
}
type SessionContext = ws::WebsocketContext<Session>;

impl Session {
    /// Constructs a new wormhole session from the uuid of the user who owns this session
    /// and an address which points to the Server.
    pub fn new(mut hackstead: Hackstead, srv: &Addr<Server>) -> Self {
        Self {
            heartbeat: Instant::now(),
            server: srv.clone(),
            ticker: ticker::Ticker::new(&mut hackstead),
            hackstead,
        }
    }

    /// This function is responsible for sending messages to the client to assure that we're still
    /// active and operational, and checking that the client has sent us a similar message recently
    /// to assure that they're still online. If they haven't sent any such message in a certain
    /// amount of time, we drop their connection and their session ends.
    fn heartbeat(ctx: &mut SessionContext) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.heartbeat) > CLIENT_TIMEOUT {
                warn!("Websocket Client heartbeat failed, disconnecting!");
                ctx.stop();
            } else {
                ctx.ping(b"");
            }
        });
    }

    #[allow(clippy::unused_self)]
    fn tick(&self, ctx: &mut SessionContext) {
        ctx.run_interval(*UPDATE_INTERVAL, |act, ctx| {
            act.ticker.tick(&mut act.hackstead, ctx)
        });
    }
}

impl Actor for Session {
    type Context = SessionContext;

    /// When a wormhole session starts, we want to immediately begin the hearbeat system, and
    /// send the server an address to this session, through which the server can begin sending us
    /// Notes.
    fn started(&mut self, ctx: &mut Self::Context) {
        // probably important to kick these off as soon as possible
        Session::heartbeat(ctx);
        self.tick(ctx);

        info!("session begins!");

        let addr = ctx.address();
        self.server
            .do_send(server::Connect(self.hackstead.steader_id(), addr));
    }

    fn stopping(&mut self, _: &mut Self::Context) -> actix::Running {
        // notify server
        info!("ending session!");
        self.server
            .do_send(server::Disconnect(self.hackstead.steader_id()));
        actix::Running::Stop
    }
}

/// Send out what the server tells us to send out, it's not hard :P
#[derive(actix::Message)]
#[rtype(result = "()")]
pub struct SendNote(pub Note);

impl Handler<SendNote> for Session {
    type Result = ();

    fn handle(&mut self, SendNote(note): SendNote, ctx: &mut Self::Context) {
        send_note(ctx, &note);
    }
}

#[derive(actix::Message)]
#[rtype(result = "Hackstead")]
pub struct GetStead;

impl Handler<GetStead> for Session {
    type Result = Hackstead;

    fn handle(&mut self, GetStead: GetStead, _: &mut Self::Context) -> Self::Result {
        let mut hs = self.hackstead.clone();
        hs.timers = self.ticker.timers.clone();
        hs
    }
}

type SteadEdit = Box<dyn FnMut(&mut Hackstead) + Send + 'static>;
#[derive(actix::Message)]
#[rtype(result = "()")]
pub struct ChangeStead(SteadEdit);

impl Handler<ChangeStead> for Session {
    type Result = ();

    fn handle(&mut self, ChangeStead(mut edit): ChangeStead, ctx: &mut Self::Context) {
        use hcor::serde_diff::Diff;

        let old = self.hackstead.clone();
        edit(&mut self.hackstead);
        let diff = serde_json::to_vec(&Diff::serializable(&old, &self.hackstead)).unwrap();
        send_note(ctx, &Note::Edit(diff))
    }
}

#[derive(actix::Message)]
#[rtype(result = "()")]
pub struct StartTimer(hcor::plant::Timer);

impl Handler<StartTimer> for Session {
    type Result = ();

    fn handle(&mut self, StartTimer(t): StartTimer, _: &mut Self::Context) {
        self.ticker.start(t);
    }
}

/// `WebSocket` message handler
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for Session {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        use ws::Message::*;

        trace!("got websockets message: {:#?}", msg);
        match msg {
            Err(e) => {
                // current ws error policy is: one error, we drop your session.
                // crude, but effective.
                error!("dropping client, websocket error: {}", e);
                ctx.stop();
            }
            Ok(Ping(msg)) => {
                self.heartbeat = Instant::now();
                ctx.pong(&msg);
            }
            Ok(Pong(_)) => self.heartbeat = Instant::now(),
            Ok(Text(t)) => {
                // we're more lenient with deserialization errors than websocket errors
                match serde_json::from_str(&t) {
                    Ok(ask) => {
                        let ss = SessSend::from_session(&self, ctx.address());
                        actix::spawn(async {
                            handle_ask(ss, ask)
                                .await
                                .unwrap_or_else(|e| error!("couldn't handle ask: {}", e))
                        });
                    }
                    Err(e) => error!("couldn't deserialize AskMessage: {}", e),
                }
            }
            Ok(Close(msg)) => {
                info!("closing websockets: {:#?}", msg);
                ctx.stop()
            }
            Ok(other) => debug!("ignoring websockets message: {:#?}", other),
        }
    }
}

fn strerr<T, E: ToString>(r: Result<T, E>) -> Result<T, String> {
    r.map_err(|e| e.to_string())
}

/// A place to store all of your pending edits to a User's Session.
pub struct SessEditStore {
    pub session: Addr<Session>,
    pub stead_edits: Vec<SteadEdit>,
    pub pending_timers: Vec<hcor::plant::Timer>,
    pub hackstead: Hackstead,
}
impl SessEditStore {
    pub fn new(session: Addr<Session>, hackstead: Hackstead) -> Self {
        Self {
            stead_edits: vec![],
            pending_timers: vec![],
            hackstead,
            session,
        }
    }

    pub async fn from_session(session: Addr<Session>) -> Result<Self, MailboxError> {
        let stead = session.send(GetStead).await?;
        Ok(Self::new(session, stead))
    }

    pub fn steddit<T, F: FnMut(&mut Hackstead) -> T + Send + 'static>(&mut self, mut f: F) -> T {
        let o = f(&mut self.hackstead);
        self.stead_edits.push(Box::new(move |hs| drop(f(hs))));
        o
    }

    pub async fn submit_stead_edits(&mut self) -> Result<(), MailboxError> {
        use futures::stream::{StreamExt, TryStreamExt};
        let mut stead_edits = std::mem::take(&mut self.stead_edits);

        futures::try_join!(
            futures::stream::iter(std::mem::take(&mut self.pending_timers))
                .map(|t| self.session.send(StartTimer(t)))
                .buffer_unordered(10)
                .try_for_each_concurrent(None, |t| async move { Ok(t) }),
            self.session.send(ChangeStead(Box::new(move |hs| {
                for edit_fn in stead_edits.iter_mut() {
                    edit_fn(hs)
                }
            })))
        )?;

        Ok(())
    }

    pub async fn send_note(&self, note: Note) -> Result<(), MailboxError> {
        self.session.send(SendNote(note)).await
    }

    pub fn set_timer(&mut self, t: hcor::plant::Timer) {
        self.pending_timers.push(t);
    }
}

/// Your `SessSend` is your interface for editing with a user's Session from afar.
/// It has helper methods for scheduling atomic edits for a user's hackstead,
/// and also exposes the addresses of the user's hackstead and server, so that
/// you can send them messages dirrectly if need be.
pub struct SessSend {
    edit_store: SessEditStore,
    server: Addr<Server>,
}
impl SessSend {
    fn from_session(ses: &Session, ses_addr: Addr<Session>) -> Self {
        Self {
            edit_store: SessEditStore::new(ses_addr, ses.hackstead.clone()),
            server: ses.server.clone(),
        }
    }
}
impl std::ops::Deref for SessSend {
    type Target = SessEditStore;

    fn deref(&self) -> &Self::Target {
        &self.edit_store
    }
}
impl<'a> std::ops::DerefMut for SessSend {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.edit_store
    }
}

async fn handle_ask(
    mut ss: SessSend,
    AskMessage { ask, ask_id }: AskMessage,
) -> Result<(), MailboxError> {
    use hcor::wormhole::{Ask::*, AskedNote::*};

    trace!(
        "got ask from {}: ask_id: {} ask: {:#?}",
        ss.hackstead.profile.steader_id,
        ask_id,
        ask
    );

    let note = match ask {
        KnowledgeSnort { xp } => {
            ss.steddit(move |hs| hs.profile.xp += xp);
            KnowledgeSnortResult(Ok(ss.hackstead.profile.xp))
        }
        Plant(p) => plant::handle_ask(&mut ss, p),
        Item(i) => item::handle_ask(&mut ss, i).await,
        TileSummon {
            tile_redeemable_item_id,
        } => TileSummonResult(strerr(tile::summon(&mut ss, tile_redeemable_item_id))),
    };
    let should_submit = note.err().is_none();

    futures::try_join!(
        // can't use `send_note` method because the borrow checker goes nuts
        ss.session.send(SendNote(Note::Asked { ask_id, note })),
        async {
            if should_submit {
                ss.submit_stead_edits().await
            } else {
                Ok(())
            }
        },
    )?;

    Ok(())
}
