use super::{strerr, HandledAskKind, SessSend};
use crate::wormhole::server;
use hcor::id::ItemId;
use hcor::wormhole::{
    AskedNote::*,
    ItemAsk::{self, *},
};

mod spawn;
use spawn::spawn;

mod hatch;
use hatch::hatch;

mod rename_gotchi;
use rename_gotchi::rename;

pub(super) fn handle_ask(ss: &mut SessSend, ask: ItemAsk) -> HandledAskKind {
    HandledAskKind::Direct(match ask {
        Spawn {
            item_archetype_handle: iah,
            amount,
        } => ItemSpawnResult(strerr(spawn(ss, iah, amount))),
        Throw {
            receiver_id,
            item_ids,
        } => {
            return HandledAskKind::ServerRelinquish(super::NoteEnvelope::new(server::ThrowItems {
                sender_id: ss.hackstead.profile.steader_id,
                receiver_id,
                item_ids,
            }))
        }
        Hatch { hatchable_item_id } => ItemHatchResult(strerr(hatch(ss, hatchable_item_id))),
        GotchiRename { item_id, new_name } => {
            GotchiRenameResult(strerr(rename(ss, item_id, new_name)))
        }
    })
}
