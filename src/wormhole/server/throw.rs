use crate::wormhole::session::SessSend;
use actix::{AsyncContext, Context, Handler, MailboxError, Message, ResponseFuture};
use hcor::{id, item, wormhole::RudeNote::ItemThrowReceipt, Item, ItemId, Note, SteaderId};
use std::fmt;

#[derive(Debug)]
pub enum Error {
    NoSuch(id::NoSuch),
    PartyOffline(SteaderId),
    Mailbox(MailboxError),
    MixedOwnership(ItemId),
    SelfGive,
}
use Error::*;

impl From<id::NoSuch> for Error {
    fn from(ns: id::NoSuch) -> Error {
        Error::NoSuch(ns)
    }
}
impl From<MailboxError> for Error {
    fn from(ns: MailboxError) -> Error {
        Error::Mailbox(ns)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "couldn't throw items to another user: ")?;
        match self {
            NoSuch(e) => write!(f, "{}", e),
            PartyOffline(u) => write!(
                f,
                "sender or receiver {} is offline, which makes it impossible to trade (for now)",
                u
            ),
            MixedOwnership(i) => write!(
                f,
                "transfer of item {} requested but sender was not owner",
                i
            ),
            SelfGive => write!(
                f,
                "huh? the sender and receiver are the same, this accomplishes nothing!"
            ),
            Mailbox(e) => write!(
                f,
                "couldn't communicate with a users's session, possibly they are offline: {}",
                e
            ),
        }
    }
}

#[derive(Message)]
#[rtype(result = "Result<Vec<Item>, Error>")]
pub struct ThrowItems {
    pub sender_id: SteaderId,
    pub receiver_id: SteaderId,
    pub item_ids: Vec<ItemId>,
}

/// Sessions must send requests that items are transferred to the Server, because only the Server
/// is capable of handling interactions between different Sessions.
impl Handler<ThrowItems> for super::Server {
    type Result = ResponseFuture<Result<Vec<Item>, Error>>;

    fn handle(&mut self, ti: ThrowItems, ctx: &mut Context<Self>) -> Self::Result {
        let ThrowItems {
            sender_id,
            receiver_id,
            item_ids,
        } = ti;

        // unfortunately we have to clone these even though they contain several Arcs so that we
        // can pass them into the 'static future.
        let ses = |id| self.sessions.get(&id).cloned().ok_or(PartyOffline(id));
        let tx_ses = ses(sender_id);
        let rx_ses = ses(receiver_id);
        let addr = ctx.address();

        Box::pin(async move {
            if sender_id == receiver_id {
                return Err(SelfGive);
            }

            let mut tx_hs = SessSend::lookup_from_addrs(tx_ses?, addr.clone()).await?;
            let mut rx_hs = SessSend::lookup_from_addrs(rx_ses?, addr.clone()).await?;

            // n^2 perf right here D:
            let mut items = item_ids
                .clone()
                .into_iter()
                .map(|i| tx_hs.take_item(i))
                .collect::<Result<Vec<Item>, id::NoSuch>>()?;

            for i in &mut items {
                if i.owner_id != sender_id {
                    return Err(MixedOwnership(i.item_id));
                }
                i.owner_id = receiver_id;

                i.ownership_log.push(item::LoggedOwner {
                    logged_owner_id: receiver_id,
                    acquisition: item::Acquisition::Trade,
                    owner_index: i.ownership_log.len(),
                })
            }

            rx_hs.inventory.append(&mut items.clone());

            rx_hs.send_note(Note::Rude(ItemThrowReceipt {
                from: sender_id,
                items: items.clone(),
            }));

            tx_hs.submit().await?;
            rx_hs.submit().await?;
            Ok(items)
        })
    }
}

#[cfg(all(test, feature = "hcor_client"))]
mod test {
    #[test]
    pub fn throw() {
        use actix::System;
        use hcor::{
            wormhole::{self, Note, RudeNote},
            Hackstead, IdentifiesSteader,
        };
        use log::*;
        use std::time::Duration;
        use tokio::{sync::oneshot, time::timeout};
        const SERVER_WAIT: Duration = Duration::from_millis(100);

        // attempt to establish logging, do nothing if it fails
        // (it probably fails because it's already been established in another test)
        drop(pretty_env_logger::try_init());

        // send bob's name to eve so she knows where to send the items
        let (tx, rx) = oneshot::channel();

        // bob
        let t1 = std::thread::spawn(move || {
            System::new("test").block_on(async move {
                let mut bobstead = Hackstead::register().await.unwrap();
                tx.send(bobstead.profile.steader_id).unwrap();

                let until = wormhole::until_map(|n| match n {
                    Note::Rude(RudeNote::ItemThrowReceipt { from, items }) => Some((from, items)),
                    _ => None,
                });
                let (eve_steader_id, items) =
                    timeout(SERVER_WAIT, until).await.expect("timeout").unwrap();

                // verify the items log eve then bob as the owners
                bobstead = Hackstead::fetch(&bobstead).await.unwrap();
                let evestead = Hackstead::fetch(eve_steader_id).await.unwrap();
                for item in &items {
                    assert!(
                        !evestead.has_item(item),
                        "eve still has an item she gave away: {:#?}",
                        item
                    );
                    assert!(
                        bobstead.has_item(item),
                        "bob doesn't have an item he was transferred: {:#?}",
                        item
                    );
                    assert_eq!(
                        item.ownership_log.len(),
                        2,
                        "spawned then traded item doesn't have two owners: {:#?}",
                        item
                    );
                    assert_eq!(
                        item.ownership_log.first().unwrap().logged_owner_id,
                        evestead.profile.steader_id,
                        "item spawned for eve doesn't log her as the first owner: {:#?}",
                        item
                    );
                    assert_eq!(
                        item.ownership_log.get(1).unwrap().logged_owner_id,
                        bobstead.profile.steader_id,
                        "item given to bob doesn't log him as the second owner: {:#?}",
                        item
                    );
                }

                // give one last item to eve
                items.last().unwrap().throw_at(&evestead).await.unwrap();

                // make sure all gives still fail when the items are of mixed ownership
                // (the items are of mixed ownership now because the last one belongs to eve again)
                match evestead.throw_items(&bobstead, &items).await {
                    Err(e) => info!(
                        "received error as expected trying to give items of mixed ownership: {}",
                        e
                    ),
                    Ok(i) => panic!(
                        "unexpectedly able to give away items of mixed ownership: {:#?}",
                        i
                    ),
                }

                bobstead.slaughter().await.unwrap();
            });
        });

        // eve
        let t2 = std::thread::spawn(move || {
            System::new("test").block_on(async move {
                const ITEM_ARCHETYPE: hcor::config::ArchetypeHandle = 0;
                const ITEM_SPAWN_COUNT: usize = 10;

                let evestead = Hackstead::register().await.unwrap();
                let bobstead = Hackstead::fetch(rx.await.unwrap()).await.unwrap();

                // spawn eve items and verify that they log her as the owner
                let items = bobstead
                    .spawn_items(ITEM_ARCHETYPE, ITEM_SPAWN_COUNT)
                    .await
                    .unwrap();
                for item in &items {
                    assert_eq!(
                        item.ownership_log.len(),
                        1,
                        "freshly spawned item has more than one owner: {:#?}",
                        item
                    );
                    assert_eq!(
                        item.ownership_log.first().unwrap().logged_owner_id,
                        evestead.profile.steader_id,
                        "item spawned for eve doesn't log her as the first owner: {:#?}",
                        item
                    );
                }

                // give items to bob
                evestead.throw_items(&bobstead, &items).await.unwrap();

                // wait until bob gives one item back
                let until = wormhole::until_map(|n| match n {
                    Note::Rude(RudeNote::ItemThrowReceipt { from, items }) => Some((from, items)),
                    _ => None,
                });
                let (_, mut items) = timeout(SERVER_WAIT, until).await.expect("timeout").unwrap();

                let item = items.pop().unwrap();
                assert!(
                    items.is_empty(),
                    "bob only gave one item, which was taken from the list already"
                );

                assert_eq!(
                    vec![
                        evestead.steader_id(),
                        bobstead.steader_id(),
                        evestead.steader_id(),
                    ],
                    item.ownership_log
                        .iter()
                        .map(|o| o.logged_owner_id)
                        .collect::<Vec<_>>(),
                    "malformed ownership log for spawned(eve), given(bob), then given(eve) item",
                );

                match item.throw_at(&evestead).await {
                    Err(e) => info!("received error as expected trying to self-give: {}", e),
                    Ok(i) => panic!("unexpectedly able to give item to self: {:#?}", i),
                }

                evestead.slaughter().await.unwrap();
            });
        });

        t1.join().unwrap();
        t2.join().unwrap();
    }
}
