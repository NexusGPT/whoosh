#![allow(dead_code)]
//! Lane labels-r5: a 12-message IMAP account, stored the way the real IMAP sync stores it (sync-imap `envelope::build`): the id is
//! "<account>:<folder>:<uidvalidity>:<uid>", the thread is "<account>:<root message-id>" (the first of References, else In-Reply-To,
//! else the message's own Message-ID, so a reply in Sent shares the thread of the message it answers), `is_outgoing` is not stored
//! (the store derives it from the From address), and the Sent folder is a folder like another. Shared by `answered_r5.rs`
//! (smart-inbox) and `counts_r5.rs` (decision) by path.
//!
//! The nine inbound messages and the three Sent copies, by thread (times are epoch ms, small on purpose):
//! ```text
//! thread a1  r1  ann  1000 unread  "Quote for the villa?"        s1 owner 2000 (Sent)   -> unread, already answered
//! thread b1  r2  bob  1100 read    "Invoice 42"                  s2 owner 1500 (Sent)   -> read, answered; r3 came after the reply
//!            r3  bob  3000 unread  "Re: Invoice 42"                                      -> unread, waiting (the reply is OLDER)
//! thread c1  r4  carl 1200 unread  "Lunch?"                      r4b carl 1250 unread   -> r4 has a later message, none of mine
//! thread d1  r5  news 1300 unread  "Weekly digest" (list header)
//! thread f1  r7  ann  1600 unread  "Contract renewal"            s3 owner 2500 (Sent)   -> unread, already answered
//! thread g1  r8  gus  1700 read    "FYI budget" (owner in Cc)
//! thread h1  r9  hal  1800 unread  "Signed form" (one attachment)
//! ```
use contracts::Message;
use store::{NewMessage, Store};

pub const ACCOUNT: &str = "acc";
pub const OWNER: &str = "owner@home-example.com";
pub const SENT_FOLDER: &str = "Sent Messages";

#[derive(Clone)]
pub struct Mail {
    pub key: &'static str,
    pub folder: &'static str,
    pub uid: u32,
    pub root: &'static str,
    pub at: i64,
    pub from: &'static str,
    pub subject: &'static str,
    pub body: &'static str,
    pub read: bool,
    pub list: bool,
    pub cc_me: bool,
    pub attachment: bool,
}

fn inbound(key: &'static str, uid: u32, root: &'static str, at: i64, from: &'static str, subject: &'static str, body: &'static str) -> Mail {
    Mail { key, folder: "INBOX", uid, root, at, from, subject, body, read: false, list: false, cc_me: false, attachment: false }
}
fn sent(key: &'static str, uid: u32, root: &'static str, at: i64, subject: &'static str, body: &'static str) -> Mail {
    Mail { key, folder: SENT_FOLDER, uid, root, at, from: OWNER, subject, body, read: true, list: false, cc_me: false, attachment: false }
}

/// The twelve messages, in no particular time order.
pub fn account_mail() -> Vec<Mail> {
    vec![
        inbound("r1", 1, "a1@x", 1_000, "ann@clients-example.com", "Quote for the villa?", "Can you send the quote for the villa?"),
        sent("s1", 1, "a1@x", 2_000, "Re: Quote for the villa?", "Sent, see the attachment.\n\nOn Mon, 9 Nov ann wrote:\n> Can you send the quote for the villa?"),
        Mail { read: true, ..inbound("r2", 2, "b1@x", 1_100, "bob@vendors-example.com", "Invoice 42", "Please pay invoice 42.") },
        sent("s2", 2, "b1@x", 1_500, "Re: Invoice 42", "Paid this morning."),
        inbound("r3", 3, "b1@x", 3_000, "bob@vendors-example.com", "Re: Invoice 42", "Thanks. One more thing about invoice 43."),
        inbound("r4", 4, "c1@x", 1_200, "carl@friends-example.com", "Lunch?", "Lunch on Friday?"),
        inbound("r4b", 5, "c1@x", 1_250, "carl@friends-example.com", "Re: Lunch?", "Ping, Friday?"),
        Mail { list: true, ..inbound("r5", 6, "d1@x", 1_300, "news@digest-example.com", "Weekly digest", "Ten links for your week.") },
        inbound("r7", 7, "f1@x", 1_600, "ann@clients-example.com", "Contract renewal", "The contract renewal is attached."),
        sent("s3", 3, "f1@x", 2_500, "Re: Contract renewal", "Signed and returned."),
        Mail { read: true, cc_me: true, ..inbound("r8", 8, "g1@x", 1_700, "gus@team-example.com", "FYI budget", "For your information, the budget.") },
        Mail { attachment: true, ..inbound("r9", 9, "h1@x", 1_800, "hal@legal-example.com", "Signed form", "The signed form is attached.") },
    ]
}

/// The id the IMAP sync gives the message.
pub fn id_of(m: &Mail) -> String {
    format!("{ACCOUNT}:{}:1:{}", m.folder, m.uid)
}

/// The id of the fixture message with this key ("r1", "s2"...).
pub fn id(key: &str) -> String {
    id_of(&account_mail().into_iter().find(|m| m.key == key).unwrap_or_else(|| panic!("no fixture message {key}")))
}

/// The thread the IMAP sync gives the message.
pub fn thread_of(m: &Mail) -> String {
    format!("{ACCOUNT}:{}", m.root)
}

fn new_message(m: &Mail) -> NewMessage {
    let inbound = m.from != OWNER;
    let (to, cc) = if !inbound {
        (vec!["ann@clients-example.com".to_string()], vec![])
    } else if m.cc_me {
        (vec!["team@team-example.com".to_string()], vec![OWNER.to_string()])
    } else {
        (vec![OWNER.to_string()], vec![])
    };
    NewMessage {
        message: Message {
            list_unsubscribe: vec![], list_unsubscribe_one_click: false, id: id_of(m), thread_id: thread_of(m), account_id: ACCOUNT.into(),
            sender_name: m.from.split('@').next().unwrap_or("").to_string(), from_address: m.from.into(), to, subject: m.subject.into(),
            preview: m.body.chars().take(60).collect(), received_at_ms: m.at, read: m.read, archived: false, folder: m.folder.into(),
            attachment_names: vec![], login_code: None, trackers_blocked: 0, reply_to_name: None, cc, bcc: vec![],
            // the real IMAP sync leaves it false on every message, the Sent copies included (the store derives it from the address)
            is_outgoing: false,
        },
        remote_id: Some(format!("{}", m.uid)),
        body_text: Some(m.body.into()),
        body_html: None,
        has_list_header: m.list,
        size_bytes: None,
        tab_id: None,
    }
}

/// An empty store holding the account, the nine inbound messages and the three Sent copies; one attachment on r9.
pub fn open_imap_account(db: &std::path::Path) -> Store {
    let store = Store::open(db).unwrap();
    store
        .write(|ctx| {
            ctx.tx.execute(
                "INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) \
                 VALUES (?1, ?2, 'imap', 'Owner', '#336699', 0, 'manual', 'svc' || char(31) || ?1, 0)",
                rusqlite::params![ACCOUNT, OWNER],
            )?;
            Ok(())
        })
        .unwrap();
    let mail = account_mail();
    store.insert_messages(mail.iter().map(new_message).collect()).unwrap();
    let r9 = id("r9");
    store
        .write(move |ctx| {
            ctx.tx.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes) VALUES ('att-1', ?1, 'form.pdf', 'application/pdf', 1024)", [&r9])?;
            Ok(())
        })
        .unwrap();
    store
}
