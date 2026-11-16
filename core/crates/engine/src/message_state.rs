//! engine-act-r6 round 6: the per-message marks the screens read as state paths (docs/STATE-PATHS.json), from the store, as the fixture core
//! answers them. `message/<id>/starred` (the Starred filter's marks, ListFilter.swift), `message/<id>/archived` (⇧E outside the Archive view,
//! TriageKeys.swift: true for a message archived with E AND for one the sync stored from an Archive-role folder, the two kinds `Unarchive`
//! brings back: the sync stores those with the archived mark too, sync-imap envelope.rs), `message/<id>/snoozed`, `snooze/<id>/return_ms` and `snooze/<id>/return_local_time` ("HH:mm" in this Mac's time; ListProbe.swift).
//! An id the store does not hold answers nothing (the real core's rule, as `message/<id>/muted`); a message that is not snoozed answers
//! nothing on the two `snooze/` paths (the fixture's rule). The same snooze facts are also on `snoozed_messages()`.
use store::Store;

use crate::reads::json_text;

/// "HH:mm" of `ms` in this Mac's time (`offset_s` from UTC), as the fixture's DateFormatter writes it.
pub fn local_time(ms: i64, offset_s: i32) -> String {
    let secs = (ms.div_euclid(1000) + offset_s as i64).rem_euclid(86_400);
    format!("{:02}:{:02}", secs / 3600, (secs % 3600) / 60)
}

pub(crate) fn state(store: &Store, path: &str, offset_s: i32) -> Option<String> {
    let p: Vec<&str> = path.split('/').collect();
    match p.as_slice() {
        ["message", id, "starred"] => store.message_marks(id).ok().flatten().map(|m| m.starred.to_string()),
        ["message", id, "archived"] => store.message_marks(id).ok().flatten().map(|m| m.archived.to_string()),
        ["message", id, "snoozed"] => store.message_marks(id).ok().flatten().map(|m| m.snoozed_until_ms.is_some().to_string()),
        ["snooze", id, "return_ms"] => store.message_marks(id).ok().flatten().and_then(|m| m.snoozed_until_ms).map(|ms| ms.to_string()),
        ["snooze", id, "return_local_time"] => store.message_marks(id).ok().flatten().and_then(|m| m.snoozed_until_ms).map(|ms| json_text(&local_time(ms, offset_s))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn local_time_is_hours_and_minutes_in_this_macs_time() {
        // 2026-11-07 10:00 UTC
        assert_eq!(super::local_time(1_794_045_600_000, 0), "10:00");
        assert_eq!(super::local_time(1_794_045_600_000, 3600), "11:00");
        assert_eq!(super::local_time(1_794_045_600_000, -11 * 3600), "23:00");
    }
}
