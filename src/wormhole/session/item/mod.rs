use super::{strerr, SessSend};
use crate::wormhole::server;
use hcor::wormhole::{
    AskedNote::{self, *},
    ItemAsk::{self, *},
};

mod spawn;
use spawn::spawn;

mod hatch;
use hatch::hatch;

pub async fn handle_ask(ss: &mut SessSend, ask: ItemAsk) -> AskedNote {
    match ask {
        Spawn {
            item_archetype_handle: iah,
            amount,
        } => ItemSpawnResult(strerr(spawn(ss, iah, amount))),
        Throw {
            receiver_id,
            item_ids,
        } => ItemThrowResult(
            match ss
                .server
                .send(server::ThrowItems {
                    sender_id: ss.hackstead.profile.steader_id,
                    receiver_id,
                    item_ids,
                })
                .await
            {
                Ok(r) => strerr(r),
                Err(e) => Err(format!("couldn't reach server mailbox: {}", e)),
            },
        ),
        Hatch { hatchable_item_id } => ItemHatchResult(strerr(hatch(ss, hatchable_item_id))),
    }
}
