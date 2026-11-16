//! Where retrieval may look: the ENABLED sources of the Sources pane, behind the account wall. "This account" (the default the owner confirmed)
//! = the enabled mail folders of the account the reply goes out from, plus the folders of this Mac and the calendars the owner switched on
//! (each was allowed explicitly, so no account owns them); "all accounts" = every enabled source.
use contracts::{ReplySourceRef, ReplySourcesRead, RetrievalScope};

/// The sources a search may read, in the pane's order, each once.
pub fn sources_in_scope(sources: &ReplySourcesRead, scope: RetrievalScope, account_id: &str) -> Vec<ReplySourceRef> {
    let mut out: Vec<ReplySourceRef> = Vec::new();
    let mut add = |r: ReplySourceRef| {
        if !out.contains(&r) {
            out.push(r);
        }
    };
    for folder in sources.accounts.iter().flat_map(|a| a.folders.iter()).filter(|s| s.enabled) {
        let mine = matches!(&folder.source, ReplySourceRef::MailFolder { account_id: a, .. } if a == account_id);
        if scope == RetrievalScope::AllAccounts || mine {
            add(folder.source.clone());
        }
    }
    for local in sources.local_folders.iter().filter(|s| s.enabled) {
        add(local.source.clone());
    }
    for cal in sources.calendars.iter().filter(|c| c.enabled) {
        add(ReplySourceRef::Calendar { calendar_id: cal.calendar_id.clone() });
    }
    out
}
