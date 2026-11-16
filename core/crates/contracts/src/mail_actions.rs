//! Delta 18, parts 2 and 3: mute a thread, leave a mailing list, remind me if no reply. The COMMANDS already exist (delta 10:
//! `Mute`, `Unarchive` = mark not done, `Unsubscribe`, `RemindIfNoReply`); this file adds what they lacked: the state words the
//! two screens read, the one refusal rule per command, the arrival rule for a muted thread, the reminder's life and its
//! `Reminders` view membership. Pure functions: the Rust core, the store and the Swift FixtureCore all call them.
//!
//! WHAT THIS FILE DOES NOT DO: it sends no unsubscribe request and fires no reminder. The adapter posts (RFC 8058) or opens the
//! mail link; the engine fires a reminder when its time comes and no reply arrived.
use crate::{CoreError, UnsubscribeMethod};

/// The `field` of the refusal "this message is not from a mailing list".
pub const UNSUBSCRIBE_NOT_A_LIST_CODE: &str = "unsubscribe_not_a_list";
/// The `field` of the refusal "you already left this list".
pub const UNSUBSCRIBE_ALREADY_DONE_CODE: &str = "unsubscribe_already_done";
/// The `field` of the refusal "that time has already passed".
pub const REMINDER_IN_THE_PAST_CODE: &str = "reminder_in_the_past";
/// The `field` of the refusal "there is no reminder to clear".
pub const REMINDER_NOTHING_TO_CLEAR_CODE: &str = "reminder_nothing_to_clear";

fn refuse(code: &str, words: impl Into<String>) -> CoreError { CoreError::Invalid { field: code.into(), reason_words: words.into() } }

// ---------------------------------------------------------------- unsubscribe

/// What the screen reads at `message/<id>/unsubscribe`: how this message's list can be left, or that it already was. `NoList` = no
/// List-Unsubscribe header (word "none"). `Mail` = the
/// header offers a mailto only (the core sends the mail). `Link` = a page the UI opens after the owner's confirm.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnsubscribeState { NoList, OneClick, Mail, Link, Done }

/// The one derivation: Done wins over any method (once left, the offer is gone); no header = None. Derived at read, stored once: the
/// method comes from the header the sync keeps, the done mark from `unsubscribed_at_ms`.
#[uniffi::export]
pub fn unsubscribe_state(method: Option<UnsubscribeMethod>, done: bool) -> UnsubscribeState {
    if done { return UnsubscribeState::Done; }
    match method {
        None => UnsubscribeState::NoList,
        Some(UnsubscribeMethod::OneClick { .. }) => UnsubscribeState::OneClick,
        Some(UnsubscribeMethod::Mail { .. }) => UnsubscribeState::Mail,
        Some(UnsubscribeMethod::Link { .. }) => UnsubscribeState::Link,
    }
}

/// The state words, once: "none", "one_click", "mail", "link", "done" (underscore, as the other state words: `waiting_for_network`).
#[uniffi::export]
pub fn unsubscribe_state_key(state: UnsubscribeState) -> String {
    match state {
        UnsubscribeState::NoList => "none", UnsubscribeState::OneClick => "one_click", UnsubscribeState::Mail => "mail",
        UnsubscribeState::Link => "link", UnsubscribeState::Done => "done",
    }.to_string()
}

/// Whether `Unsubscribe` may run, and by which method: refused when the message is not from a list, and when the list was already
/// left (a second one-click post helps nobody and the confirm would lie).
#[uniffi::export]
pub fn check_unsubscribe(method: Option<UnsubscribeMethod>, done: bool) -> Result<UnsubscribeMethod, CoreError> {
    if done { return Err(refuse(UNSUBSCRIBE_ALREADY_DONE_CODE, "You already left this list.")); }
    method.ok_or_else(|| refuse(UNSUBSCRIBE_NOT_A_LIST_CODE, "This message is not from a mailing list."))
}

// ---------------------------------------------------------------- mute

/// What the arrival filing does with a new message of a thread: a muted thread's mail arrives READ and stays out of the inbox lists,
/// where the Muted view (`Mailbox::Muted`) holds it; any other thread's mail arrives as it always did. One rule, said once, for the
/// engine-decision lane to call.
#[derive(uniffi::Record, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrivalTreatment { pub mark_read: bool, pub in_inbox: bool }

#[uniffi::export]
pub fn arrival_in_thread(thread_muted: bool) -> ArrivalTreatment {
    if thread_muted { ArrivalTreatment { mark_read: true, in_inbox: false } } else { ArrivalTreatment { mark_read: false, in_inbox: true } }
}

// ---------------------------------------------------------------- remind if no reply

/// Where a reminder is, read at `thread/<id>/reminder`. `NoReminder` = no reminder set (or cleared). `Waiting` = set and its time has not come.
/// `Fired` = its time came and no reply had arrived: the thread is in the Reminders view. `Answered` = a reply arrived at or before
/// its time: it never fires and is not in the view.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyReminderState { NoReminder, Waiting { at_ms: i64 }, Fired { at_ms: i64 }, Answered { at_ms: i64 } }

/// The one derivation. `first_reply_ms` = when the first reply from someone else reached the thread after the owner's message went
/// out (None = none yet). A reply AT the reminder's time counts as in time ("no reply by then" = none by `at_ms` inclusive).
/// A reply that comes after a fired reminder does not unfire it: the owner dismisses it (`ClearReplyReminder`).
#[uniffi::export]
pub fn reply_reminder_state(at_ms: Option<i64>, first_reply_ms: Option<i64>, now_ms: i64) -> ReplyReminderState {
    let Some(at) = at_ms else { return ReplyReminderState::NoReminder };
    if first_reply_ms.is_some_and(|r| r <= at) { return ReplyReminderState::Answered { at_ms: at }; }
    if now_ms >= at { ReplyReminderState::Fired { at_ms: at } } else { ReplyReminderState::Waiting { at_ms: at } }
}

/// The state words, once: "none", "waiting", "fired", "answered".
#[uniffi::export]
pub fn reply_reminder_state_key(state: ReplyReminderState) -> String {
    match state {
        ReplyReminderState::NoReminder => "none", ReplyReminderState::Waiting { .. } => "waiting",
        ReplyReminderState::Fired { .. } => "fired", ReplyReminderState::Answered { .. } => "answered",
    }.to_string()
}

/// Whether a thread is in the Reminders built-in view: only a fired reminder puts it there.
#[uniffi::export]
pub fn reply_reminder_in_view(state: ReplyReminderState) -> bool { matches!(state, ReplyReminderState::Fired { .. }) }

/// Whether `RemindIfNoReply` may set a reminder at `at_ms`: a time in the past is refused (it would fire at once).
#[uniffi::export]
pub fn check_remind_if_no_reply(at_ms: i64, now_ms: i64) -> Result<(), CoreError> {
    if at_ms <= now_ms { Err(refuse(REMINDER_IN_THE_PAST_CODE, "That time has already passed.")) } else { Ok(()) }
}

/// Whether `ClearReplyReminder` has a reminder to clear: any state but NoReminder (a fired or answered one is cleared too: that is how the
/// owner dismisses a row of the Reminders view).
#[uniffi::export]
pub fn check_clear_reply_reminder(state: ReplyReminderState) -> Result<(), CoreError> {
    if state == ReplyReminderState::NoReminder { Err(refuse(REMINDER_NOTHING_TO_CLEAR_CODE, "There is no reminder to clear.")) } else { Ok(()) }
}

