//! fix-blockforget: the real engine serves BlockSender (it answered "The local engine does not do this yet."). The test engine
//! (Sources/CoreBridge/FixtureLabels.swift `blockSenderLocked`) and the contract (command.rs: "Blocks a sender address: their mail goes to
//! junk on arrival. Idempotent: a second block changes nothing.") are the specification:
//! - the address is trimmed and lower-cased; one that is not a full address (one "@", something before it, a dot after it) is refused in the
//!   test engine's words, and nothing is written;
//! - the block is ONE row of the store's `block` table (`Store::block_add`, the function BlockDomain uses in notes_cmds.rs), kept on this
//!   Mac; the junk filter reads that table for every arrival (junk_filter.rs `context`), so the sender's NEXT mail goes to junk;
//! - the mail already in the inbox is left where it is (the test engine moves none; the contract names arrivals only);
//! - no undo entry (the test engine answers `undoEntryId: nil`; the contract's `UndoAction` has no block kind);
//! - nothing reaches the network: no account of this engine has a server filter to mirror to, and the pending-operation queue has no
//!   filter kind (docs/lanes/fix-blockforget.md).
//! The contract has no UnblockSender command; nothing more to serve here. State paths are notes_cmds.rs's (block/count, block/list,
//! block/<address>/blocked) and the trait's `blocks()`.
use contracts::*;
use store::Store;

use crate::rust_core::now_ms;

const OK: CommandOutcome = CommandOutcome { undo_entry_id: None, created_id: None };

/// The one command this file serves.
pub(crate) fn is_command(c: &Command) -> bool { matches!(c, Command::BlockSender { .. }) }

/// An address as the test engine's `blockSenderLocked` takes it: trimmed, lower case, exactly one "@", a name before it, a dot after it.
pub(crate) fn clean_address(address: &str) -> Result<String, CoreError> {
    let a = address.trim().to_lowercase();
    let parts: Vec<&str> = a.split('@').collect();
    if parts.len() != 2 || parts[0].is_empty() || !parts[1].contains('.') {
        return Err(CoreError::Invalid { field: "address".into(), reason_words: "a sender is a full address like name@example.com".into() });
    }
    Ok(a)
}

/// Runs BlockSender: one row in the store's block table (a second block of the same address changes nothing, its first time stays).
pub(crate) fn run(store: &Store, command: Command) -> Result<CommandOutcome, CoreError> {
    match command {
        Command::BlockSender { address } => {
            let address = clean_address(&address)?;
            store.block_add(BlockTarget::Sender { address }, now_ms())?;
            Ok(OK)
        }
        _ => Err(CoreError::Invalid { field: "command".into(), reason_words: "not a block command".into() }),
    }
}
