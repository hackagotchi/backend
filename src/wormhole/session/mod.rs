use std::time::Instant;

use actix::{Actor, ActorContext, Addr, AsyncContext, Handler, MailboxError, StreamHandler};
use actix_web_actors::ws;
use log::*;

use super::server::{self, Server};
use hcor::{
    wormhole::{AskMessage, AskedNote, EditNote, CLIENT_TIMEOUT, HEARTBEAT_INTERVAL},
    Hackstead, IdentifiesSteader, Note, UPDATE_INTERVAL,
};

mod item;
mod ticker;
mod tile;
use tile::plant;

/// Which opening to the wormhole are they making use of?
#[derive(Copy, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Orifice {
    /// Binary messages, encoded in Bincode
    Bincode,
    /// Text messages, encoded in JSON
    Json,
}

/// An individual user's session with the Server. It contains an address to that server so
/// that it can notify it when it connects/disconnects, and the server keeps an address to it so
/// that it can give it Notes to send down the Wormhole (or in this case, Websocket) to be displayed
/// to the user.
pub struct Session {
    hackstead: Hackstead,
    heartbeat: Instant,
    orifice: Orifice,
    server: Addr<Server>,
    ticker: ticker::Ticker,
}
type SessionContext = ws::WebsocketContext<Session>;

impl Session {
    /// Constructs a new wormhole session from the uuid of the user who owns this session
    /// and an address which points to the Server.
    pub fn new(mut hackstead: Hackstead, srv: &Addr<Server>, orifice: Orifice) -> Self {
        Self {
            heartbeat: Instant::now(),
            server: srv.clone(),
            ticker: ticker::Ticker::new(&mut hackstead),
            orifice,
            hackstead,
        }
    }

    fn send_note(&self, ctx: &mut SessionContext, note: &Note) {
        match self.orifice {
            Orifice::Json => match serde_json::to_string(note) {
                Ok(json) => ctx.text(json),
                Err(e) => error!("couldn't Json serialize Note: {}", e),
            },
            Orifice::Bincode => match bincode::serialize(note) {
                Ok(bytes) => ctx.binary(bytes),
                Err(e) => error!("couldn't Bincode serialize Note: {}", e),
            },
        }
    }

    fn spawn_ask_handler(&self, ctx: &SessionContext, ask: AskMessage) {
        let ss = SessSend::new(self.hackstead.clone(), ctx.address(), self.server.clone());
        actix::spawn(async {
            handle_ask(ss, ask)
                .await
                .map(|_| ())
                .unwrap_or_else(|e| error!("couldn't handle ask: {}", e))
        });
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
            let mut ticker = std::mem::take(&mut act.ticker);
            ticker.tick(act, ctx);
            act.ticker = ticker;
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
        self.send_note(ctx, &note);
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

#[derive(actix::Message)]
#[rtype(result = "Result<AskedNote, MailboxError>")]
pub struct DoAsk(pub hcor::Ask);

impl Handler<DoAsk> for Session {
    type Result = actix::ResponseFuture<Result<AskedNote, MailboxError>>;

    fn handle(&mut self, DoAsk(ask): DoAsk, ctx: &mut Self::Context) -> Self::Result {
        let ss = SessSend::new(self.hackstead.clone(), ctx.address(), self.server.clone());
        Box::pin(handle_ask(ss, AskMessage { ask, ask_id: 1337 }))
    }
}

#[derive(actix::Message)]
#[rtype(result = "()")]
pub struct ChangeStead(Hackstead);

impl Handler<ChangeStead> for Session {
    type Result = ();

    fn handle(&mut self, ChangeStead(mut new): ChangeStead, ctx: &mut Self::Context) {
        use hcor::serde_diff::Diff;

        let old = self.hackstead.clone();
        assert_eq!(new.local_version, old.local_version);

        self.send_note(
            ctx,
            &Note::Edit(match self.orifice {
                Orifice::Json => {
                    EditNote::Json(serde_json::to_string(&Diff::serializable(&old, &new)).unwrap())
                }
                Orifice::Bincode => {
                    EditNote::Bincode(bincode::serialize(&Diff::serializable(&old, &new)).unwrap())
                }
            }),
        );

        new.local_version += 1;
        self.hackstead = new;
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
            Ok(Text(t)) if self.orifice == Orifice::Json => {
                // we're more lenient with deserialization errors than websocket errors
                match serde_json::from_str(&t) {
                    Ok(ask) => self.spawn_ask_handler(ctx, ask),
                    Err(e) => error!("couldn't deserialize JSON AskMessage: {}", e),
                }
            }
            Ok(Binary(b)) if self.orifice == Orifice::Bincode => {
                // we're more lenient with deserialization errors than websocket errors
                match bincode::deserialize(&b) {
                    Ok(ask) => self.spawn_ask_handler(ctx, ask),
                    Err(e) => error!("couldn't deserialize Bincode AskMessage: {}", e),
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
///
/// Note that these edits are "transactional", in that nothing actually changes until `.submit` is
/// called. This comes in especially handy in the Ask handling, where Asks may fail for any number
/// of reasons. If sacrificing an item for a tile fails halfway through, you don't want users to
/// still lose their item, which is why `SessSend`s are only submitted if ask handlers return
/// `Ok(_)` variants.
///
/// Your `SessSend` is your interface for editing with a user's Session from afar.
/// It has helper methods for scheduling transactional edits for a user's hackstead,
/// and also exposes the addresses of the user's hackstead and server, so that
/// you can send them messages directly if need be.
pub struct SessSend {
    pub session: Addr<Session>,
    pub server: Addr<Server>,
    pub pending_timers: Vec<hcor::plant::Timer>,
    pub pending_notes: Vec<Note>,
    pub hackstead: Hackstead,
}
impl SessSend {
    /// Create a new SessSend using the data we need from a Session.
    pub fn new(hackstead: Hackstead, session: Addr<Session>, server: Addr<Server>) -> Self {
        Self {
            pending_timers: vec![],
            pending_notes: vec![],
            hackstead,
            session,
            server,
        }
    }

    /// Create a SessSend without a Hackstead by querying the underlying Session Address; may take
    /// significantly longer than `SessSend::new` and should therefore be avoided wherever
    /// possible.
    pub async fn lookup_from_addrs(
        session: Addr<Session>,
        server: Addr<Server>,
    ) -> Result<Self, MailboxError> {
        Ok(Self::new(session.send(GetStead).await?, session, server))
    }

    /// Schedule a Note to be sent to this user when this SessSend is submitted.
    pub fn send_note(&mut self, note: Note) {
        self.pending_notes.push(note)
    }

    /// Schedule a Timer to be set on this user's hackstead when this SessSend is submitted.
    pub fn set_timer(&mut self, t: hcor::plant::Timer) {
        self.pending_timers.push(t);
    }

    /// Consumes a `SessSend`, sending all of the desired changes to the user's Session to be
    /// applied.
    pub async fn submit(self) -> Result<(), MailboxError> {
        use futures::stream::{StreamExt, TryStreamExt};
        let Self {
            session,
            pending_timers,
            pending_notes,
            hackstead,
            ..
        } = self;

        futures::try_join!(
            futures::stream::iter(pending_timers)
                .map(|t| session.send(StartTimer(t)))
                .buffer_unordered(10)
                .try_for_each_concurrent(None, |t| async move { Ok(t) }),
            futures::stream::iter(pending_notes)
                .map(|n| session.send(SendNote(n)))
                .buffer_unordered(10)
                .try_for_each_concurrent(None, |n| async move { Ok(n) }),
            session.send(ChangeStead(hackstead))
        )?;

        Ok(())
    }
}
impl std::ops::Deref for SessSend {
    type Target = Hackstead;

    fn deref(&self) -> &Self::Target {
        &self.hackstead
    }
}
impl<'a> std::ops::DerefMut for SessSend {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.hackstead
    }
}

/// If the ask fails for whatever reason, the `SessSend` is not submitted,
/// and therefore no changes are made to the user's session,
/// in the form of hackstead mutations or set timers.
async fn handle_ask(
    mut ss: SessSend,
    AskMessage { ask, ask_id }: AskMessage,
) -> Result<AskedNote, MailboxError> {
    use hcor::wormhole::{Ask::*, AskedNote::*};

    trace!(
        "got ask from {}: ask_id: {} ask: {:#?}",
        ss.hackstead.profile.steader_id,
        ask_id,
        ask
    );

    let note = match ask {
        KnowledgeSnort { xp } => KnowledgeSnortResult(Ok({
            let hs = &mut ss.hackstead;
            hs.profile.xp += xp;
            hs.profile.xp
        })),
        Plant(p) => plant::handle_ask(&mut ss, p),
        Item(i) => item::handle_ask(&mut ss, i).await,
        TileSummon {
            tile_redeemable_item_id,
        } => TileSummonResult(strerr(tile::summon(&mut ss, tile_redeemable_item_id))),
    };

    ss.send_note(Note::Asked {
        ask_id,
        note: note.clone(),
    });

    if note.err().is_none() {
        ss.submit().await?;
    }

    Ok(note)
}
