//! The shapes of a search: what the owner typed, the chips read from it, and the threads that come back.
pub mod dates;

/// A filter the owner typed in words. Each recognised filter is exactly one chip (C-J2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChipKind {
    From,
    Before,
    After,
    Attachment,
    Unread,
    Account,
}

impl ChipKind {
    /// The word the fixtures and the UI use for the kind.
    pub fn word(self) -> &'static str {
        match self {
            ChipKind::From => "from",
            ChipKind::Before => "before",
            ChipKind::After => "after",
            ChipKind::Attachment => "attachment",
            ChipKind::Unread => "unread",
            ChipKind::Account => "account",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chip {
    pub kind: ChipKind,
    /// The value as typed ("Linea Supply", "June"); None for has attachment and unread.
    pub value: Option<String>,
    /// The words the chip was read from, as typed ("from Linea Supply"); removing the chip removes these.
    pub words: String,
}

/// The filters the chips stand for, ready to test a message against.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filters {
    /// Lower-cased; a message matches when its sender name or address contains any of them.
    pub from: Vec<String>,
    /// Received strictly before this instant (ms since the epoch).
    pub before_ms: Option<i64>,
    /// Received at or after this instant.
    pub after_ms: Option<i64>,
    pub has_attachment: bool,
    pub unread: bool,
    /// Account ids; a message matches when it is in any of them.
    pub accounts: Vec<String>,
}

impl Filters {
    pub fn is_empty(&self) -> bool {
        *self == Filters::default()
    }

    pub fn admits(&self, m: &contracts::Message) -> bool {
        let sender = format!("{} {}", m.sender_name, m.from_address).to_lowercase();
        (self.from.is_empty() || self.from.iter().any(|f| sender.contains(f.as_str())))
            && self.before_ms.is_none_or(|b| m.received_at_ms < b)
            && self.after_ms.is_none_or(|a| m.received_at_ms >= a)
            && (!self.has_attachment || !m.attachment_names.is_empty())
            && (!self.unread || !m.read)
            && (self.accounts.is_empty() || self.accounts.contains(&m.account_id))
    }
}

/// One typed line, read.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParsedQuery {
    pub chips: Vec<Chip>,
    pub filters: Filters,
    /// The words left to search, in the order typed, quoted phrases kept with their quotes.
    pub residual: String,
    /// The exact phrases among them, without quotes.
    pub phrases: Vec<String>,
    /// Filter words whose value was not understood ("before Junetember"); they stay in `residual` and the bar says so.
    pub not_understood: Vec<String>,
}

impl ParsedQuery {
    /// The line again, chips first then the words: what a smart folder stores.
    pub fn canonical_text(&self) -> String {
        let mut parts: Vec<&str> = self.chips.iter().map(|c| c.words.as_str()).collect();
        if !self.residual.is_empty() {
            parts.push(&self.residual);
        }
        parts.join(" ")
    }

    /// The line with chip `index` removed (the chip's x).
    pub fn without_chip(&self, index: usize) -> String {
        let mut parts: Vec<&str> = self.chips.iter().enumerate().filter(|(i, _)| *i != index).map(|(_, c)| c.words.as_str()).collect();
        if !self.residual.is_empty() {
            parts.push(&self.residual);
        }
        parts.join(" ")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccountState {
    pub id: String,
    /// The display name the owner gave the account ("Nordlicht").
    pub name: String,
    /// True once the account's mail is in the text and vector indexes.
    pub indexed: bool,
}

/// A run of snippet text, marked when it is one of the searched words.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub text: String,
    pub marked: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MessageMatch {
    pub message_id: String,
    pub account_id: String,
    pub sender_name: String,
    pub received_at_ms: i64,
    /// Fused reciprocal-rank score.
    pub score: f64,
    /// True when the message holds a quoted phrase exactly (C-J2.3: exact outranks meaning).
    pub exact: bool,
    pub snippet: Vec<Segment>,
    /// The attachment that matched, when the match is inside one or in its name (contracts delta 10): the row names it.
    pub attachment: Option<contracts::SearchHit>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThreadResult {
    pub thread_id: String,
    /// The best match: Enter opens the thread here (C-J2.4).
    pub best_message_id: String,
    pub account_id: String,
    pub subject: String,
    /// The best member's score; threads are ordered by it.
    pub score: f64,
    /// Every matching message of the thread, best first.
    pub matches: Vec<MessageMatch>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SearchOutcome {
    pub query: ParsedQuery,
    pub threads: Vec<ThreadResult>,
    /// A quoted phrase was asked for and no message holds it; meaning results are kept (C-J2.3).
    pub no_exact_match: bool,
    /// Display names of the accounts whose mail is not indexed yet (C-J2.1); non-empty means partial results.
    pub accounts_still_loading: Vec<String>,
    /// Offer "search the servers too" (C-J2.1): true while results are partial.
    pub offer_server_search: bool,
    /// The meaning half failed; text results stand alone and this says why.
    pub meaning_unavailable: Option<String>,
}
