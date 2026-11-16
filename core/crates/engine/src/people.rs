//! search-forms part 3: the people of all stored mail. A person is an address that sent the owner mail, or that the owner sent mail to
//! (the To and Cc of a mail in an account's Sent-role folder, or of a mail whose sender is one of the owner's accounts). Per person:
//! the best display name seen (a sender's own name first, the newest one; else the newest name a recipient field carried; else the
//! address), `from_count` (mails from them), `to_count` (mails the owner sent them), and both per account.
//!
//! Kept in memory, no table and no migration. The first build never runs on the path that opens the core: a thread of its own at
//! background priority reads the message table PEOPLE_BUILD_PAGE_ROWS rows per step, taking the book's lock for one step at a time,
//! and a read before it ends answers from the people built so far (`people/ready` = false; `people/count` counts those). Once it
//! ends, every read first reads only the rows stored since the last one (`rowid > watermark`, the table's own key), so a read costs
//! the new rows plus a pass over the people (thousands, not mails). A delete in the message table (an expunge, an account removed)
//! marks the book stale; the next read starts a full rebuild on its own thread and answers from the current book meanwhile.
//! Known limit: a mail moved between folders after it was read (into a Sent-role folder, say) is not recounted until a rebuild
//! (the store's change feed names the table, not the row, so a move cannot be told from a flag change without re-reading).
//!
//! Reads: `RustCore::contacts()` and `contact(address)` (the contract's `Contact`), and the state paths `people/prefix/<letters>`,
//! `people/address/<address>`, `people/count` (shapes in docs/lanes/search-forms.md "Part 3").
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use contracts::{Contact, SenderDomain};
use rusqlite::{params, Connection};
use store::{Store, TableChange};
use sync_imap::FolderRole;

use crate::imap_source::Roles;
use crate::reads::json_text;

/// Rows a prefix read answers at most.
/// docs/CONSTANTS.md PEOPLE_PREFIX_MAX.
pub(crate) const PEOPLE_PREFIX_MAX: usize = contracts::constants::PEOPLE_PREFIX_MAX as usize;
/// Rows a sender-domain read answers at most (SPEC A153), like PEOPLE_PREFIX_MAX: the suggestion list's height.
/// Ledger row owed (docs/CONSTANTS.md SENDER_DOMAINS_MAX).
pub(crate) const SENDER_DOMAINS_MAX: usize = contracts::constants::SENDER_DOMAINS_MAX as usize;
/// Message rows the first build reads per step: the book's lock is held for one step only, so a read waits at most one step.
/// docs/CONSTANTS.md PEOPLE_BUILD_PAGE_ROWS.
pub(crate) const PEOPLE_BUILD_PAGE_ROWS: usize = contracts::constants::PEOPLE_BUILD_PAGE_ROWS as usize;

/// Tests only (`RustCore::set_people_build_pause`): the first build waits this many ms before each step, so a test can read
/// while it is still building. 0 in production.
static BUILD_PAUSE_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub(crate) fn set_build_pause_ms(ms: u64) { BUILD_PAUSE_MS.store(ms, Ordering::SeqCst); }

/// The calling thread at macOS QOS_CLASS_BACKGROUND (the efficiency cores, throttled disk); nothing elsewhere.
#[cfg(target_os = "macos")]
fn background_priority() {
    extern "C" { fn pthread_set_qos_class_self_np(qos_class: u32, relative_priority: i32) -> i32; }
    const QOS_CLASS_BACKGROUND: u32 = 0x09;
    // SAFETY: sets the calling thread's own QoS class; no pointer is passed.
    let _ = unsafe { pthread_set_qos_class_self_np(QOS_CLASS_BACKGROUND, 0) };
}
#[cfg(not(target_os = "macos"))]
fn background_priority() {}

/// Lower case with the accents of the Latin letters taken off ("Zoë" reads "zoe", "Straße" reads "strasse").
pub(crate) fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(char::to_lowercase) {
        let r: &str = match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
            'æ' => "ae",
            'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
            'ď' | 'đ' => "d",
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
            'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
            'ĥ' | 'ħ' => "h",
            'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
            'ĵ' => "j",
            'ķ' => "k",
            'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
            'ñ' | 'ń' | 'ņ' | 'ň' => "n",
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => "o",
            'œ' => "oe",
            'ŕ' | 'ŗ' | 'ř' => "r",
            'ś' | 'ŝ' | 'ş' | 'š' => "s",
            'ß' => "ss",
            'ţ' | 'ť' | 'ŧ' => "t",
            'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
            'ŵ' => "w",
            'ý' | 'ÿ' | 'ŷ' => "y",
            'ź' | 'ż' | 'ž' => "z",
            _ => { out.push(c); continue; }
        };
        out.push_str(r);
    }
    out
}

/// A recipient field's entry: "Name <a@b>", "\"Name\" <a@b>" or "a@b" as (name, lower-case address).
pub(crate) fn split_address(entry: &str) -> (String, String) {
    match (entry.find('<'), entry.rfind('>')) {
        (Some(l), Some(r)) if l < r => (entry[..l].trim().trim_matches('"').trim().to_string(), entry[l + 1..r].trim().to_ascii_lowercase()),
        _ => (String::new(), entry.trim().trim_matches('"').trim().to_ascii_lowercase()),
    }
}

/// One person as the reads give it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Person {
    pub address: String,
    pub name: String,
    pub from_count: u32,
    pub to_count: u32,
    pub per_account: HashMap<String, u32>,
    /// fix-blockforget: of `per_account`, the mails FROM this person, per account (the rest are mails the owner sent them), so a removed
    /// account's share leaves `from_count` and `to_count` exactly (`forget_account`)
    from_per_account: HashMap<String, u32>,
    /// 2: a sender's own name, 1: a recipient field's name, 0: none (the name is the address)
    name_rank: u8,
    name_at: i64,
    words: Vec<String>,
}

impl Person {
    fn new(address: &str) -> Person {
        Person { address: address.to_string(), name: address.to_string(), from_count: 0, to_count: 0, per_account: HashMap::new(), from_per_account: HashMap::new(), name_rank: 0, name_at: i64::MIN, words: vec![fold(address)] }
    }
    fn total(&self) -> u32 { self.from_count + self.to_count }
    /// A name seen at `at` with `rank`: kept when it ranks higher, or the same and newer.
    fn offer_name(&mut self, name: &str, rank: u8, at: i64) {
        let name = name.trim();
        if name.is_empty() || name.eq_ignore_ascii_case(&self.address) { return; }
        if rank > self.name_rank || (rank == self.name_rank && at >= self.name_at) {
            self.name = name.to_string();
            self.name_rank = rank;
            self.name_at = at;
            let folded = fold(name);
            self.words = folded.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(String::from).collect();
            self.words.push(folded);
        }
    }
    /// The start of the whole name, of one of its words, or of the address.
    fn starts_with(&self, folded: &str) -> bool {
        folded.is_empty() || self.address.starts_with(folded) || self.words.iter().any(|w| w.starts_with(folded))
    }
    pub(crate) fn contact(&self) -> Contact {
        Contact { id: format!("person:{}", self.address), name: self.name.clone(), addresses: vec![self.address.clone()], logo_id: None, mail_count_per_account: self.per_account.clone() }
    }
    fn json(&self) -> String {
        format!("{{\"name\":{},\"address\":{},\"from_count\":{},\"to_count\":{}}}", json_text(&self.name), json_text(&self.address), self.from_count, self.to_count)
    }
}

#[derive(Default)]
struct Book { watermark: i64, people: HashMap<String, Person>, conn: Option<Connection> }

pub(crate) struct PeopleBook { book: Mutex<Book>, stale: AtomicBool, rebuilding: AtomicBool, ready: AtomicBool, store: Arc<Store>, roles: Roles }

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> { m.lock().unwrap_or_else(|e| e.into_inner()) }

impl PeopleBook {
    /// The book of a core: built on a thread of its own now; a delete in the message table marks it stale.
    pub(crate) fn start(store: Arc<Store>, roles: Roles) -> Arc<PeopleBook> {
        let me = Arc::new(PeopleBook { book: Mutex::new(Book::default()), stale: AtomicBool::new(false), rebuilding: AtomicBool::new(false), ready: AtomicBool::new(false), store, roles });
        let weak: Weak<PeopleBook> = Arc::downgrade(&me);
        me.store.subscribe_tables(Arc::new(move |c: &TableChange| {
            if c.table == "message" && c.deletes > 0 { if let Some(b) = weak.upgrade() { b.stale.store(true, Ordering::SeqCst); } }
        }));
        // the first build runs beside the open, at background priority, PEOPLE_BUILD_PAGE_ROWS rows per step, the book's lock taken
        // per step only: a read before it ends answers from the people built so far (`people/ready` says false)
        let weak = Arc::downgrade(&me);
        let spawned = std::thread::Builder::new().name("whoosh-people".into()).spawn(move || {
            background_priority();
            loop {
                let pause = BUILD_PAUSE_MS.load(Ordering::SeqCst);
                if pause > 0 { std::thread::sleep(std::time::Duration::from_millis(pause)); }
                let Some(me) = weak.upgrade() else { return };
                // ready is set under the lock: a read that takes the lock after the last step reads the newer rows itself
                let done = { let mut b = lock(&me.book); let d = me.read_rows(&mut b, Some(PEOPLE_BUILD_PAGE_ROWS)); if d { me.ready.store(true, Ordering::SeqCst); } d };
                if done { return; }
            }
        });
        if let Err(e) = spawned {
            // no thread: the reads build the book themselves, as after the first build
            eprintln!("people: the first build did not start: {e}");
            me.ready.store(true, Ordering::SeqCst);
        }
        me
    }

    /// Whether the first build has read every stored row.
    pub(crate) fn ready(&self) -> bool { self.ready.load(Ordering::SeqCst) }

    /// The book brought up to date with the rows stored since its last read; a stale book starts a rebuild beside it.
    fn fresh(self: &Arc<Self>) -> std::sync::MutexGuard<'_, Book> {
        if self.stale.swap(false, Ordering::SeqCst) && !self.rebuilding.swap(true, Ordering::SeqCst) {
            let me = self.clone();
            let spawned = std::thread::Builder::new().name("whoosh-people-rebuild".into()).spawn(move || {
                let mut new = Book::default();
                me.read_new(&mut new);
                let mut b = lock(&me.book);
                // rows stored during the rebuild are read by the next read (its watermark is the rebuild's)
                *b = new;
                me.rebuilding.store(false, Ordering::SeqCst);
            });
            if spawned.is_err() { self.rebuilding.store(false, Ordering::SeqCst); self.stale.store(true, Ordering::SeqCst); }
        }
        let mut b = lock(&self.book);
        // before the first build ends, a read never reads rows itself: it answers from what is built so far
        if self.ready() { self.read_new(&mut b); }
        b
    }

    /// Reads every message row after the book's watermark into it.
    fn read_new(&self, b: &mut Book) { self.read_rows(b, None); }

    /// Reads the message rows after the book's watermark into it, at most `limit`; true when it reached the last row (or failed).
    fn read_rows(&self, b: &mut Book, limit: Option<usize>) -> bool {
        match self.try_read_rows(b, limit) {
            Ok(done) => done,
            Err(e) => { eprintln!("people: the new mail was not read: {e}"); true }
        }
    }

    fn try_read_rows(&self, b: &mut Book, limit: Option<usize>) -> Result<bool, String> {
        if b.conn.is_none() { b.conn = Some(search_text::source::open_reader(self.store.path()).map_err(|e| e.to_string())?); }
        let accounts = self.store.account_summaries().map_err(|e| e.to_string())?;
        let own: HashSet<String> = accounts.iter().map(|a| a.address.trim().to_ascii_lowercase()).collect();
        let ids: Vec<String> = accounts.into_iter().map(|a| a.id).collect();
        let sent: HashSet<(String, String)> = crate::reads::folders_with_role(&self.store, &self.roles, &ids, FolderRole::Sent).into_iter().collect();
        let after = b.watermark;
        let Some(conn) = b.conn.as_ref() else { return Ok(true) };
        // the rows of this step (SQLite reads LIMIT -1 as no limit), then the To and Cc of exactly those rows
        let wanted: i64 = limit.map_or(-1, |n| n as i64);
        let headers: Vec<(i64, String, String, String, String, i64)> = {
            let mut st = conn.prepare_cached(
                "SELECT rowid, account_id, folder, from_address, sender_name, received_at_ms FROM message WHERE rowid > ?1 ORDER BY rowid LIMIT ?2").map_err(|e| e.to_string())?;
            let rows = st.query_map(params![after, wanted], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, i64>(5)?)))
                .map_err(|e| e.to_string())?;
            rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
        };
        let done = limit.is_none_or(|n| headers.len() < n);
        let Some(hi) = headers.last().map(|h| h.0) else { return Ok(true) };
        let mut recipients: HashMap<i64, Vec<String>> = HashMap::new();
        {
            let mut st = conn.prepare_cached(
                "SELECT m.rowid, j.value FROM message m, json_each(m.to_json) j WHERE m.rowid > ?1 AND m.rowid <= ?2 \
                 UNION ALL SELECT m.rowid, j.value FROM message m, json_each(m.cc_json) j WHERE m.rowid > ?1 AND m.rowid <= ?2").map_err(|e| e.to_string())?;
            let rows = st.query_map(params![after, hi], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))).map_err(|e| e.to_string())?;
            for row in rows { let (rowid, entry) = row.map_err(|e| e.to_string())?; recipients.entry(rowid).or_default().push(entry); }
        }
        let mut top = after;
        for (rowid, account, folder, from, sender_name, at) in headers {
            top = top.max(rowid);
            let from = from.trim().to_ascii_lowercase();
            let by_owner = own.contains(&from) || sent.contains(&(account.clone(), folder));
            if by_owner {
                for entry in recipients.get(&rowid).map(Vec::as_slice).unwrap_or(&[]) {
                    let (name, address) = split_address(entry);
                    if !address.contains('@') || own.contains(&address) { continue; }
                    let p = b.people.entry(address.clone()).or_insert_with(|| Person::new(&address));
                    p.to_count += 1;
                    *p.per_account.entry(account.clone()).or_default() += 1;
                    p.offer_name(&name, 1, at);
                }
            } else if from.contains('@') {
                let p = b.people.entry(from.clone()).or_insert_with(|| Person::new(&from));
                p.from_count += 1;
                *p.per_account.entry(account.clone()).or_default() += 1;
                *p.from_per_account.entry(account.clone()).or_default() += 1;
                p.offer_name(&sender_name, 2, at);
            }
        }
        b.watermark = top;
        Ok(done)
    }

    fn ranked<'a>(people: impl Iterator<Item = &'a Person>) -> Vec<&'a Person> {
        let mut v: Vec<&Person> = people.collect();
        v.sort_by(|a, b| b.total().cmp(&a.total()).then_with(|| fold(&a.name).cmp(&fold(&b.name))).then_with(|| a.address.cmp(&b.address)));
        v
    }

    /// fix-blockforget (docs/lanes/real-accounts.md finding 2): an account was removed from this Mac. Its mail leaves every person's counts
    /// at once, in memory, and a person known only through it leaves the book, so the very next read lists none of it (before, the first
    /// read after a removal answered the old book while the rebuild ran). The rebuild the removal's delete starts (`stale`) then reads the
    /// store as it is. Returns how many people left. One pass over the people (thousands, not mails), under the book's lock.
    pub(crate) fn forget_account(&self, account_id: &str) -> usize {
        let mut b = lock(&self.book);
        let before = b.people.len();
        b.people.retain(|_, p| {
            if let Some(n) = p.per_account.remove(account_id) {
                let from = p.from_per_account.remove(account_id).unwrap_or(0).min(n);
                p.from_count = p.from_count.saturating_sub(from);
                p.to_count = p.to_count.saturating_sub(n - from);
            }
            p.total() > 0
        });
        before - b.people.len()
    }

    /// Every person, the most mail first (the contract's `contacts()`).
    pub(crate) fn contacts(self: &Arc<Self>) -> Vec<Contact> {
        let b = self.fresh();
        Self::ranked(b.people.values()).into_iter().map(Person::contact).collect()
    }

    /// One person by address (any case).
    pub(crate) fn contact(self: &Arc<Self>, address: &str) -> Option<Contact> {
        let b = self.fresh();
        b.people.get(&split_address(address).1).map(Person::contact)
    }

    /// The people whose name, a word of it, or address starts with `letters` (case and accents folded), the most mail first, at most `limit`.
    pub(crate) fn prefix(self: &Arc<Self>, letters: &str, limit: usize) -> Vec<Person> {
        let folded = fold(letters.trim());
        let b = self.fresh();
        Self::ranked(b.people.values().filter(|p| p.starts_with(&folded))).into_iter().take(limit).cloned().collect()
    }

    /// SPEC A153: the domains of the senders of received mail (`from_count` > 0: the owner's own mail and the Sent-role folders are not
    /// in it) that start with `letters` (case and accents folded; "@" and spaces at either end ignored), each with its different sender
    /// addresses and its mails, most mails first, then domain A to Z, at most SENDER_DOMAINS_MAX. No letters: nothing. One pass over the
    /// people in memory (thousands, not mails); no store read beyond the rows stored since the last read.
    pub(crate) fn sender_domains(self: &Arc<Self>, letters: &str) -> Vec<SenderDomain> {
        let folded = fold(letters.trim_matches(|c: char| c == '@' || c.is_whitespace()));
        if folded.is_empty() { return Vec::new(); }
        let b = self.fresh();
        let mut by_domain: HashMap<&str, (u32, u32)> = HashMap::new();
        for p in b.people.values().filter(|p| p.from_count > 0) {
            let Some((_, domain)) = p.address.rsplit_once('@') else { continue };
            if domain.is_empty() || !fold(domain).starts_with(&folded) { continue; }
            let e = by_domain.entry(domain).or_insert((0, 0));
            e.0 += 1;
            e.1 += p.from_count;
        }
        let mut v: Vec<(&str, (u32, u32))> = by_domain.into_iter().collect();
        v.sort_by(|a, b| (b.1).1.cmp(&(a.1).1).then_with(|| a.0.cmp(b.0)));
        // owned rows bound before the function ends: the borrows of the book's guard end here
        let out: Vec<SenderDomain> = v.into_iter().take(SENDER_DOMAINS_MAX).map(|(d, (people, mails))| SenderDomain { domain: d.to_string(), people, mails }).collect();
        out
    }

    /// `people/prefix/<letters>`, `people/address/<address>`, `people/count`.
    pub(crate) fn state(self: &Arc<Self>, path: &str) -> Option<String> {
        if let Some(letters) = path.strip_prefix("people/prefix/") {
            return Some(format!("[{}]", self.prefix(letters, PEOPLE_PREFIX_MAX).iter().map(Person::json).collect::<Vec<_>>().join(",")));
        }
        if let Some(address) = path.strip_prefix("people/address/") {
            let b = self.fresh();
            return Some(b.people.get(&split_address(address).1).map(Person::json).unwrap_or_else(|| "null".into()));
        }
        if path == "people/count" { return Some(self.fresh().people.len().to_string()); }
        if path == "people/ready" { return Some(self.ready().to_string()); }
        None
    }
}
