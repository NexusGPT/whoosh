//! gap-moved-slice: the stored threading headers of a mail, as the hidden real-engine check (checks/realcore-check.py --slice-moved) reads
//! them to know the fill-in pass has stored what a reply is threaded from (SPEC A144): `message/<id>/thread_headers` is
//! `{"message_id":"<..>","references":["<..>",..]}`. `<id>` is a message id, or a thread id (then the newest message of that thread that has
//! stored headers answers, the one a reply uses). Nothing is answered while no message has stored headers. A read of the store only.
use store::Store;

use crate::reads::json_text;

pub(crate) fn state(store: &Store, path: &str) -> Option<String> {
    let p: Vec<&str> = path.split('/').collect();
    let ["message", id, "thread_headers"] = p[..] else { return None };
    let h = match store.thread_headers(id).ok().flatten() {
        Some(h) => h,
        None => store.newest_thread_headers(id).ok().flatten().map(|(_, h)| h)?,
    };
    let refs: Vec<String> = h.references.iter().map(|r| json_text(r)).collect();
    Some(format!("{{\"message_id\":{},\"references\":[{}]}}", json_text(&h.message_id), refs.join(",")))
}
