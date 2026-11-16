//! vip-mark: `MarkVip { message_id, vip }` in the real core. The ADDRESS of the message's sender joins (true) or leaves (false) the
//! owner's VIP list (SQLite table `vip`, docs/DATA-MAP.md row vip-list: "Mark VIP" on a sender) through smart-inbox's `EditVips`, which
//! also moves that sender's inbox mail to or from the VIPs tab in the same commit. The store announces the counts on a change of the
//! `vip` table (store/src/writer.rs COUNTS_TABLES), so the VIPs view (`Mailbox::Vips`) and `Counts.mailboxes["vips"]` follow.
use contracts::{CommandOutcome, CoreError};
use smart_inbox::{EditVips, VipEdit};
use store::Store;

use crate::reminders::store_failed;

/// An id that names no message is refused (`NotFound`), as `mute` refuses one; nothing is written then. No undo entry.
pub(crate) fn run(store: &Store, message_id: &str, vip: bool) -> Result<CommandOutcome, CoreError> {
    let message = store
        .message(message_id)
        .map_err(|e| store_failed("the message could not be read", e))?
        .ok_or_else(|| CoreError::NotFound { kind: "message".into(), id: message_id.to_string() })?;
    let address = message.from_address.trim().to_string();
    if address.is_empty() {
        return Err(CoreError::Invalid { field: "sender".into(), reason_words: "this message names no sender address to mark as a VIP".into() });
    }
    let edit = if vip { VipEdit::MarkAddress(address) } else { VipEdit::UnmarkAddress(address) };
    EditVips { store }
        .execute(edit, crate::rust_core::now_ms())
        .map_err(|e| CoreError::Unexpected { reason_words: format!("the VIP list could not be stored: {e}") })?;
    Ok(CommandOutcome { undo_entry_id: None, created_id: None })
}
