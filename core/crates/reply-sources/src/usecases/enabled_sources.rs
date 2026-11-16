//! "The enabled sources for account X", in the order the Sources pane shows them: X's card first, then (with "Tous les comptes") the
//! other accounts' cards in the pane's order, then the folders of this Mac, then the calendars. Folders of this Mac and calendars are
//! not an account's: they count for every account.
use contracts::{ReplySourceRef, ReplySourcesRead, RetrievalScope};

pub fn enabled_sources(read: &ReplySourcesRead, account_id: &str) -> Vec<ReplySourceRef> {
    let card = |a: &contracts::ReplyAccountSources| a.folders.iter().filter(|s| s.enabled).map(|s| s.source.clone()).collect::<Vec<_>>();
    let mut out: Vec<ReplySourceRef> = read.accounts.iter().filter(|a| a.account_id == account_id).flat_map(card).collect();
    if read.scope == RetrievalScope::AllAccounts {
        out.extend(read.accounts.iter().filter(|a| a.account_id != account_id).flat_map(card));
    }
    out.extend(read.local_folders.iter().filter(|s| s.enabled).map(|s| s.source.clone()));
    out.extend(read.calendars.iter().filter(|c| c.enabled).map(|c| ReplySourceRef::Calendar { calendar_id: c.calendar_id.clone() }));
    out
}
