use std::time::Instant;

use actix::{
    dev::Envelope, Actor, ActorContext, Addr, AsyncContext, Handler, MailboxError, StreamHandler,
};
use actix_web_actors::ws;
use futures_channel::oneshot;
use log::*;

use super::server::{self, Server};
use hcor::{
    wormhole::{AskMessage, AskedNote, EditNote, CLIENT_TIMEOUT, HEARTBEAT_INTERVAL},
    Hackstead, IdentifiesSteader, Note, UPDATE_INTERVAL,
};

mod hackstead_guard;
mod item;
mod ticker;
mod tile;
use hackstead_guard::HacksteadGuard;
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
    hackstead: HacksteadGuard,
    heartbeat: Instant,
    orifice: Orifice,
    server: Addr<Server>,
    ticker: ticker::Ticker,
}
type SessionContext = ws::WebsocketContext<Session>;

impl Session {
    /// Constructs a new wormhole session from the uuid of the user who owns this session
    /// and an address which points to the Server.
    pub fn new(mut hs: Hackstead, srv: &Addr<Server>, orifice: Orifice) -> Self {
        Self {
            heartbeat: Instant::now(),
            server: srv.clone(),
            ticker: ticker::Ticker::new(&mut hs),
            orifice,
            hackstead: HacksteadGuard::new(hs),
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

    fn apply_change(
        &mut self,
        ctx: &mut SessionContext,
        sss: SessSendSubmit,
        sess_send: SessSend,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> {
        match sss {
            SessSendSubmit::Submit => sess_send.submit(self, ctx),
            SessSendSubmit::Cancel(note) => self.send_note(ctx, &note),
            SessSendSubmit::ServerRelinquishAsk { msg, ask_id } => {
                let server = self.server.clone();
                let session = ctx.address();
                return Box::pin(async move {
                    async move {
                        let note = msg.send(&server).await;
                        session.send(SendNote(Note::Asked { note, ask_id })).await
                    }
                    .await
                    .unwrap_or_else(|e| error!("couldn't relinquish task to server: {}", e));
                });
            }
        }

        Box::pin(async move { () })
    }

    fn spawn_ask_handler(&mut self, ctx: &mut SessionContext, ask: AskMessage) {
        let mut ss = SessSend::new(self.hackstead.clone());
        actix::spawn(self.apply_change(ctx, handle_ask(&mut ss, ask).into(), ss))
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
        ctx.run_interval(*UPDATE_INTERVAL, |ses, ctx| {
            use actix::fut::WrapFuture;
            let mut ss = SessSend::new(ses.hackstead.clone());
            let submit = ses.ticker.tick(&mut ss);
            let fut = ses.apply_change(ctx, submit, ss);
            ctx.wait(fut.into_actor(ses))
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
        self.hackstead.clone()
    }
}

#[derive(actix::Message)]
#[rtype(result = "Result<AskedNote, MailboxError>")]
pub struct DoAsk(pub hcor::Ask);

impl Handler<DoAsk> for Session {
    type Result = actix::ResponseFuture<Result<AskedNote, MailboxError>>;

    fn handle(&mut self, DoAsk(ask): DoAsk, ctx: &mut Self::Context) -> Self::Result {
        let mut ss = SessSend::new(self.hackstead.clone());
        let HandledAsk { kind, ask_id } = handle_ask(&mut ss, AskMessage { ask, ask_id: 1337 });

        match kind {
            HandledAskKind::Direct(note) => {
                let f = self.apply_change(
                    ctx,
                    HandledAsk {
                        kind: HandledAskKind::Direct(note.clone()),
                        ask_id,
                    }
                    .into(),
                    ss,
                );
                Box::pin(async move {
                    f.await;
                    Ok(note)
                })
            }
            HandledAskKind::ServerRelinquish(msg) => {
                let server = self.server.clone();
                Box::pin(async move { Ok(msg.send(&server).await) })
            }
        }
    }
}

#[derive(actix::Message)]
#[rtype(result = "Result<(), ()>")]
pub struct ChangeStead<F: FnOnce(&mut SessSend) -> SessSendSubmit + Send + 'static>(F);

impl<F: FnOnce(&mut SessSend) -> SessSendSubmit + Send + 'static> Handler<ChangeStead<F>>
    for Session
{
    type Result = actix::ResponseFuture<Result<(), ()>>;

    fn handle(
        &mut self,
        ChangeStead(change): ChangeStead<F>,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let mut sess_send = SessSend::new(self.hackstead.clone());
        let f = self.apply_change(ctx, change(&mut sess_send), sess_send);
        Box::pin(async move {
            f.await;
            Ok(())
        })
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

pub fn strerr<T, E: ToString>(r: Result<T, E>) -> Result<T, String> {
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
    pub pending_timers: Vec<hcor::plant::Timer>,
    pub pending_notes: Vec<Note>,
    pub hackstead: Hackstead,
}
impl SessSend {
    /// Create a new SessSend
    pub fn new(hackstead: Hackstead) -> Self {
        Self {
            pending_timers: vec![],
            pending_notes: vec![],
            hackstead,
        }
    }

    /// Schedule a Note to be sent to this user when this SessSend is submitted.
    pub fn send_note(&mut self, note: Note) {
        self.pending_notes.push(note)
    }

    /// Schedule a Timer to be set on this user's hackstead when this SessSend is submitted.
    pub fn set_timer(&mut self, t: hcor::plant::Timer) {
        self.pending_timers.push(t);
    }

    pub async fn submit_afar(self, addr: &Addr<Session>) -> Result<(), MailboxError> {
        addr.send(ChangeStead(|ss| {
            *ss = self;
            SessSendSubmit::Submit
        }))
        .await?
        .unwrap();

        Ok(())
    }

    /// Consumes a `SessSend`, sending all of the desired changes to the user's Session to be
    /// applied.
    pub fn submit(self, session: &mut Session, ctx: &mut SessionContext) {
        let SessSend {
            hackstead,
            mut pending_notes,
            pending_timers,
            ..
        } = self;

        //session.hackstead.apply(hackstead);
        let (changes, diff) = session.hackstead.apply(hackstead);

        if changes {
            pending_notes.push(Note::Edit(EditNote::Json(
                serde_json::to_string(&diff).unwrap(),
            )));
            /* TODO:
            pending_notes.push(Note::Edit(match session.orifice {
                Orifice::Json => EditNote::Json(serde_json::to_string(&diff).unwrap()),
                Orifice::Bincode => EditNote::Bincode(bincode::serialize(&diff).unwrap()),
            }));*/
        }

        for t in pending_timers {
            session.ticker.start(t);
        }

        for n in pending_notes {
            session.send_note(ctx, &n);
        }
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

pub enum SessSendSubmit {
    Cancel(Note),
    ServerRelinquishAsk { msg: NoteEnvelope, ask_id: usize },
    Submit,
}
impl From<HandledAsk> for SessSendSubmit {
    fn from(HandledAsk { kind, ask_id }: HandledAsk) -> Self {
        match kind {
            HandledAskKind::Direct(note) if note.err().is_none() => SessSendSubmit::Submit,
            HandledAskKind::Direct(note) => SessSendSubmit::Cancel(Note::Asked { note, ask_id }),
            HandledAskKind::ServerRelinquish(msg) => {
                SessSendSubmit::ServerRelinquishAsk { msg, ask_id }
            }
        }
    }
}

pub struct NoteEnvelope {
    envelope: Envelope<super::Server>,
    hook: oneshot::Receiver<AskedNote>,
}
impl NoteEnvelope {
    fn new<M>(msg: M) -> Self
    where
        super::Server: Handler<M>,
        M: actix::Message<Result = AskedNote> + Send + 'static,
    {
        let (tx, hook) = oneshot::channel();
        Self {
            envelope: Envelope::new(msg, Some(tx)),
            hook,
        }
    }

    async fn send(self, server: &Addr<Server>) -> AskedNote {
        let NoteEnvelope { envelope, hook } = self;
        server.do_send(server::HandleEnvelope(envelope));
        hook.await.expect("receiver somehow cancelled")
    }
}

enum HandledAskKind {
    Direct(AskedNote),
    ServerRelinquish(NoteEnvelope),
}
pub struct HandledAsk {
    ask_id: usize,
    kind: HandledAskKind,
}

/// If the ask fails for whatever reason, the `SessSend` is not submitted,
/// and therefore no changes are made to the user's session,
/// in the form of hackstead mutations or set timers.
fn handle_ask(ss: &mut SessSend, AskMessage { ask, ask_id }: AskMessage) -> HandledAsk {
    use hcor::wormhole::{Ask::*, AskedNote::*};

    trace!(
        "got ask from {}: ask_id: {} ask: {:#?}",
        ss.hackstead.profile.steader_id,
        ask_id,
        ask
    );

    let kind = match ask {
        KnowledgeSnort { xp } => HandledAskKind::Direct(KnowledgeSnortResult(Ok({
            ss.profile.xp += xp;
            ss.profile.xp
        }))),
        Plant(p) => HandledAskKind::Direct(plant::handle_ask(ss, p)),
        Item(i) => item::handle_ask(ss, i),
        TileSummon {
            tile_redeemable_item_id,
        } => HandledAskKind::Direct(TileSummonResult(strerr(tile::summon(
            ss,
            tile_redeemable_item_id,
        )))),
    };

    if let HandledAskKind::Direct(note) = &kind {
        ss.send_note(Note::Asked {
            ask_id,
            note: note.clone(),
        });
    }

    HandledAsk { ask_id, kind }
}
