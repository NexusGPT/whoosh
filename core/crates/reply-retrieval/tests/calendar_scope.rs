//! gap-replies (item 7604cdeb): the calendars as a source of the reply writer, behind the account wall. The index here IGNORES the sources it is given
//! (as `common::Index` documents), so what the writer receives is decided by the crate alone.
//!   a selected calendar's events reach the writer, an unselected calendar's never do (the same retrieval, one calendar on, one off)
//!   the wall: with "this account" another account's mail never reaches the writer, the selected calendar's events still do (no account owns a calendar);
//!   with "all accounts" the other account's mail joins and the calendar is asked once
//! Red-first broken copies (core/crates/reply-retrieval/src/usecases/search_scope.rs):
//!   `for cal in sources.calendars.iter().filter(|c| c.enabled)` -> `.filter(|c| true || c.enabled)`   (an_unselected_calendars_event_*: red)
//!   `if scope == RetrievalScope::AllAccounts || mine {` -> `if true {`                                   (the_wall_*: red)
mod common;
use common::*;
use contracts::{ReplySourceRef, RetrievalScope};
use reply_retrieval::*;

fn cal(id: &str) -> ReplySourceRef { ReplySourceRef::Calendar { calendar_id: id.into() } }

#[test]
fn a_selected_calendars_event_reaches_the_writer_and_an_unselected_calendars_event_never_does() {
    let req = request();   // cal-1 on, cal-2 off
    let i = index(vec![
        ex_in(cal("cal-1"), "ev-on", 0.9, Some(NOV_09), "Board meeting", "Board meeting with Claire, Tuesday 10:00-11:00."),
        ex_in(cal("cal-2"), "ev-off", 0.95, Some(NOV_09), "Dentist", "Dentist appointment, Tuesday 15:00-16:00."),
    ]);
    let o = run(&req, &writer("Quand ?"), &i, &judge());
    assert_eq!(kept_ids(&o), vec!["ev-on"], "only the selected calendar's event is kept");
    assert_eq!(dropped_with(&o, &DropReason::OutsideScope), vec!["ev-off"]);
    assert!(o.context_text.contains("Board meeting") && !o.context_text.contains("Dentist"), "the writer's context:\n{}", o.context_text);
    let asked = i.queries.lock().unwrap()[0].sources.clone();
    assert!(asked.contains(&cal("cal-1")) && !asked.contains(&cal("cal-2")), "the index is asked for the selected calendar only: {asked:?}");
}

#[test]
fn the_wall_keeps_another_accounts_mail_out_while_the_selected_calendar_stays_in() {
    let this = |scope| {
        let mut req = request();   // message of account a1; a2's INBOX is on; cal-1 is on
        req.settings.scope = scope;
        let i = index(vec![
            ex_in(mail("a2", "INBOX"), "other-mail", 0.99, Some(NOV_09), "", "Prix chez l'autre compte."),
            ex_in(cal("cal-1"), "ev-on", 0.5, Some(NOV_09), "Board meeting", "Board meeting with Claire."),
        ]);
        let o = run(&req, &writer("Quand ?"), &i, &judge());
        let asked = i.queries.lock().unwrap().iter().map(|q| q.sources.iter().filter(|s| **s == cal("cal-1")).count()).collect::<Vec<_>>();
        (kept_ids(&o), dropped_with(&o, &DropReason::OutsideScope), asked)
    };
    let (kept, dropped, asked) = this(RetrievalScope::ThisAccount);
    assert_eq!(kept, vec!["ev-on"], "this account: the calendar's event stays");
    assert_eq!(dropped, vec!["other-mail"], "this account: the other account's mail is behind the wall");
    assert!(asked.iter().all(|n| *n == 1), "the calendar is asked once: {asked:?}");
    let (kept, dropped, asked) = this(RetrievalScope::AllAccounts);
    assert_eq!(kept, vec!["other-mail", "ev-on"], "all accounts: the other account's mail joins, the event stays");
    assert!(dropped.is_empty() && asked.iter().all(|n| *n == 1), "{dropped:?} {asked:?}");
}
