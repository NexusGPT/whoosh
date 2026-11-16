//! What a placement reads about one message. Built from the store (a re-sort) or from a message as sync hands it over
//! (arrival, where the raw headers and X-GM-LABELS are still at hand).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GmailCategory {
    Primary,
    Promotions,
    Social,
    Updates,
}

/// What the owner's mailbox says about the sender.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SenderHistory {
    /// Messages from this address received earlier.
    pub received_before: u32,
    /// The owner has written to this address before.
    pub owner_wrote_to: bool,
}

/// One earlier message of a message's thread, as a label's AI reads it when its card ticks the rest of the thread (lane labels-r5).
/// The text is without the quoted messages it repeats; the decision crate cuts the list to each model's budget, oldest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadPart {
    pub from: String,
    pub at_ms: i64,
    /// Sent from one of the owner's addresses (the Sent folder's copy of a reply).
    pub from_owner: bool,
    pub text: String,
}

/// The word that stands for a list header whose List-Id was not kept (so an empty-words list rule still matches).
pub const LIST_HEADER_WORD: &str = "list";

#[derive(Debug, Clone, Default)]
pub struct MessageFacts {
    pub message_id: String,
    pub thread_id: String,
    pub account_id: String,
    pub from_address: String,
    /// The sender's display name (contracts delta 12): a From rule on a NAME matches at arrival too, as the contract's
    /// label-rule facts (`contracts::MessageFacts.from_name`) expect.
    pub from_name: String,
    /// Not read yet (contracts delta 12): the contract's label-rule facts carry it for the live count's unread figure.
    pub unread: bool,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub subject: String,
    pub body: String,
    /// The List-Id header value, when the raw headers were at hand.
    pub list_id: Option<String>,
    pub has_list_unsubscribe: bool,
    /// List-Id or List-Unsubscribe is present (the store keeps this one bit).
    pub has_list_header: bool,
    /// Gmail IMAP X-GM-LABELS values as given.
    pub gmail_labels: Vec<String>,
    pub label_ids: Vec<String>,
    /// For each label the message carries, the chain of label ids from the root to that label (a label path such as
    /// client > Provanta > Orbit is three ids). Built from the label table's parent_id; depth is not limited here.
    pub label_chains: Vec<Vec<String>>,
    pub label_names: Vec<String>,
    pub has_attachment: bool,
    pub received_at_ms: i64,
    pub from_owner: bool,
    pub from_vip: bool,
    pub history: SenderHistory,
    /// Delta 17, the facts of a label's grid that are not above (unread, attachment, VIP and the list header already are). All four
    /// come from columns already synced (`load::complete`), never a second fetch: starred (`message.starred`); unanswered = inbound
    /// and nothing newer in its thread (the waiting definition); cc_me = one of the owner's addresses in Cc and none in To;
    /// replied_by_me = the owner wrote EARLIER in the thread: a stored message from one of the owner's addresses dated before this one
    /// (owner ruling 2026-11-10; it replaces "a later message of mine").
    pub starred: bool,
    pub unanswered: bool,
    pub cc_me: bool,
    pub replied_by_me: bool,
    /// The earlier messages of the thread, oldest first; empty unless `thread::attach_thread` filled it (a label ticked the rest of the
    /// thread). Never loaded by `facts_for`: reading it is opt-in, so the default path pays nothing.
    pub thread_before: Vec<ThreadPart>,
}

impl MessageFacts {
    /// The facts a label rule reads (`contracts::MessageFacts`) from these facts, field for field. The list header is the List-Id when
    /// it was at hand, else a marker word when any list header was present. One text for the filing (`decision::labels::rule_facts`)
    /// and for the live count (`corpus::rule_set_count`), so a rule is judged one way.
    pub fn rule_facts(&self) -> contracts::MessageFacts {
        contracts::MessageFacts {
            message_id: self.message_id.clone(),
            account_id: self.account_id.clone(),
            from_name: self.from_name.clone(),
            from_address: self.from_address.clone(),
            to: self.to.clone(),
            cc: self.cc.clone(),
            subject: self.subject.clone(),
            body: self.body.clone(),
            has_attachment: self.has_attachment,
            list_header: match (&self.list_id, self.has_list_header) {
                (Some(id), _) => id.clone(),
                (None, true) => LIST_HEADER_WORD.to_string(),
                (None, false) => String::new(),
            },
            unread: self.unread,
            received_at_ms: self.received_at_ms,
            unanswered: self.unanswered,
            starred: self.starred,
            vip_sender: self.from_vip,
            cc_me: self.cc_me,
            replied_by_me: self.replied_by_me,
        }
    }

    pub fn domain(&self) -> String {
        self.from_address.rsplit_once('@').map(|(_, d)| d.trim().to_ascii_lowercase()).unwrap_or_default()
    }

    /// The Gmail category an X-GM-LABELS value names. Only labels that start with "category" count, so an owner's own
    /// label called "Social" does not place mail (the spelling Gmail IMAP uses is read from the Gmail help and the API
    /// label ids, not from a live account: unverified on real mail).
    pub fn gmail_category(&self) -> Option<GmailCategory> {
        for l in &self.gmail_labels {
            let n: String = l.to_ascii_lowercase().chars().filter(|c| c.is_ascii_alphabetic()).collect();
            let Some(rest) = n.strip_prefix("category") else { continue };
            return match rest {
                "promotions" => Some(GmailCategory::Promotions),
                "social" => Some(GmailCategory::Social),
                "updates" | "forums" => Some(GmailCategory::Updates),
                "personal" => Some(GmailCategory::Primary),
                _ => continue,
            };
        }
        None
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeaderFacts {
    pub list_id: Option<String>,
    pub has_list_unsubscribe: bool,
    pub has_list_header: bool,
}

/// List-Id and List-Unsubscribe from raw message headers (unfolded; stops at the first blank line).
pub fn header_facts(raw: &[u8]) -> HeaderFacts {
    let text = String::from_utf8_lossy(raw);
    let mut out = HeaderFacts::default();
    let mut current: Option<(String, String)> = None;
    let flush = |cur: &mut Option<(String, String)>, out: &mut HeaderFacts| {
        if let Some((name, value)) = cur.take() {
            match name.as_str() {
                "list-id" => out.list_id = Some(value.trim().to_string()),
                "list-unsubscribe" => out.has_list_unsubscribe = !value.trim().is_empty(),
                _ => {}
            }
        }
    };
    for line in text.lines() {
        if line.trim().is_empty() {
            break;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some((_, v)) = current.as_mut() {
                v.push(' ');
                v.push_str(line.trim());
            }
            continue;
        }
        flush(&mut current, &mut out);
        if let Some((n, v)) = line.split_once(':') {
            current = Some((n.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
    }
    flush(&mut current, &mut out);
    out.has_list_header = out.list_id.is_some() || out.has_list_unsubscribe;
    out
}
