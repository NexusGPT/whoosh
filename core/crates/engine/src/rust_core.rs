use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use contracts::*;
use secrets::{DeleteSecret, Namespace, ReadSecret, Secret, SecretKind, StoreSecret};
use store::Store;
use tokio::sync::Notify;

use crate::filing_worker::Filing;
use crate::import_run::{ImportBook, ImportPace};
use crate::imap_source::Roles;
use crate::map;
use crate::reads;
use crate::sign_in::{authority::AuthorityHttp, codes, SignIn, SignInParts};
use crate::sync::{self, emit, Sinks, SyncDeps, SyncPhase};
use crate::actions_bridge::{self as act, Act, SystemClock};
use crate::outbox_tick::{CancelViaOutbox, MailOut};
use crate::writeback::{ImapWriteBack, Ledger, Worker};
// gap-engine-state: the three state paths (mcp_grant, split_tab name, smart_folder count); declared here so lib.rs stays untouched
#[path = "state_extra.rs"]
mod state_extra;
// gap-moved-slice: message/<id>/thread_headers, the stored threading headers (state_headers.rs)
#[path = "state_headers.rs"]
mod state_headers;

// search-forms (SPEC A141): the owner's search on both indexes (declared here: lib.rs is not this lane's file)
#[path = "search_cmd.rs"]
pub(crate) mod search_cmd;
// search-forms part 3: the people of all stored mail (contacts, the prefix read)
#[path = "people.rs"]
pub(crate) mod people;
// real-accounts: the account commands (rename, pause, remove, icon, colour, order, the AddAccount refusal) and the account state paths; a child of this module so it reads the core's private fields
#[path = "account_cmds.rs"]
pub(crate) mod account_cmds;
// gap-engine-tests (f2dd23de): the two rule previews, read from the store (declared here: lib.rs is not this lane's file)
#[path = "rule_counts.rs"]
mod rule_counts;
// real-folders: the smart folder commands, list and state paths (smart_folder_cmds.rs); declared here: lib.rs is not this lane's file
#[path = "smart_folder_cmds.rs"]
mod smart_folder_cmds;
// real-labels: the label record commands (pin, create, update, rename, move, reorder, delete, remove from messages) and the label state paths
#[path = "label_cmds.rs"]
mod label_cmds;
#[path = "block_cmds.rs"] mod block_cmds;   // fix-blockforget: BlockSender (block_cmds.rs)

/// The credential store as the core uses it: a password written at Add account, read back to sync at the next launch.
pub trait Keyring: StoreSecret + ReadSecret + DeleteSecret {}
impl<T: StoreSecret + ReadSecret + DeleteSecret> Keyring for T {}

/// Provider detection as the core runs it: the address in, one callback per step as it starts, the result out.
pub trait Detect: Send + Sync {
    fn detect(&self, email: &str, on_step: &mut dyn FnMut(autoconfig::Step)) -> autoconfig::DetectionResult;
}
impl<H: autoconfig::HttpPort + Send + Sync, D: autoconfig::DnsPort + Send + Sync, P: autoconfig::ProbePort + Send + Sync> Detect for autoconfig::Detector<H, D, P> {
    fn detect(&self, email: &str, on_step: &mut dyn FnMut(autoconfig::Step)) -> autoconfig::DetectionResult { self.detect_with_progress(email, on_step) }
}

/// How validation reaches the servers. `Real` is the only value production builds; `PlainLoopback` exists for the in-process
/// fakes (IMAP and SMTP in clear text on 127.0.0.1), is reachable only from Rust, and the sign-in check refuses any other host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestTransport { Real, PlainLoopback }

/// The hidden test copy's runtime mutants, read only when `rig` is true (`WHOOSH_CORE_MUTANT`): `no-mail-record` answers no
/// MX record; `no-microsoft` also refuses every Autodiscover request, so neither Microsoft signal reaches the detector.
/// nexus-example.com is found by Autodiscover first on the real network, so checks/realcore-check.py --mutant uses `no-microsoft`.
struct NoMailRecord<D>(D);
struct NoAutodiscover<H>(H);
impl<H: autoconfig::HttpPort> autoconfig::HttpPort for NoAutodiscover<H> {
    fn execute(&self, request: &autoconfig::HttpRequest) -> Result<autoconfig::HttpResponse, autoconfig::NetError> {
        if request.url.to_ascii_lowercase().contains("autodiscover") { Err(autoconfig::NetError::Refused) } else { self.0.execute(request) }
    }
}
impl<D: autoconfig::DnsPort> autoconfig::DnsPort for NoMailRecord<D> {
    fn execute(&self, query: &autoconfig::DnsQuery) -> Result<autoconfig::DnsAnswer, autoconfig::NetError> {
        match query { autoconfig::DnsQuery::Mx(_) => Ok(autoconfig::DnsAnswer::Mx(vec![])), q => self.0.execute(q) }
    }
}

/// The hidden check's local servers (slice 2, rig only, `WHOOSH_CORE_CHECK_SERVERS=<imap port>:<smtp port>`): an address
/// on the reserved domain `whoosh-check.invalid` (RFC 2606, never a real domain) is found on 127.0.0.1 at those ports, so
/// the check adds its account through the sheet without typing a port. Every other address runs the real detector.
pub const CHECK_DOMAIN: &str = "whoosh-check.invalid";
/// With `WHOOSH_CORE_CHECK_AUTHORITY` too (slice 2b), the address is Microsoft-shaped: OAuth2, its sign-in at that authority.
struct CheckServers { imap_port: u16, smtp_port: u16, microsoft: bool }
impl CheckServers {
    fn from_env() -> Option<Self> {
        let v = std::env::var("WHOOSH_CORE_CHECK_SERVERS").ok()?;
        let (i, s) = v.split_once(':')?;
        Some(CheckServers { imap_port: i.trim().parse().ok()?, smtp_port: s.trim().parse().ok()?, microsoft: crate::sign_in::check_authority_port().is_some() })
    }
    fn answer(&self, email: &str, on_step: &mut dyn FnMut(autoconfig::Step)) -> Option<autoconfig::DetectionResult> {
        let domain = email.rsplit_once('@')?.1.trim().to_ascii_lowercase();
        if domain != CHECK_DOMAIN { return None; }
        on_step(autoconfig::Step::Guess);
        let config = autoconfig::MailConfig {
            imap_host: "127.0.0.1".into(), imap_port: self.imap_port, imap_security: autoconfig::Security::Ssl,
            smtp_host: "127.0.0.1".into(), smtp_port: self.smtp_port, smtp_security: autoconfig::Security::Starttls,
            username: email.trim().to_string(), smtp_username: email.trim().to_string(),
            auth_method: if self.microsoft { autoconfig::AuthMethod::Oauth2 } else { autoconfig::AuthMethod::Password }, source: autoconfig::Source::Guess,
        };
        let provider = self.microsoft.then_some(autoconfig::Provider::Microsoft365);
        Some(autoconfig::DetectionResult { outcome: autoconfig::Outcome::Found(config), steps_tried: vec![autoconfig::Step::Guess], microsoft_365_tenant: self.microsoft, provider, sign_in_needed: self.microsoft })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mutant { None, NoMailRecord, NoMicrosoft, SyncNeverStores, FlowNeverFiles, ImportNeverStores, ActNeverWritesBack, BodyFetchSync, SearchNeverAnswers, ReplyIgnoresStore, CrawlNeverRuns, RuleCountsZero, BodyStoredRaw }

enum DetectorSlot { Ready(Box<dyn Detect>), Production { mutant: Mutant, check: Option<CheckServers>, built: OnceLock<Result<Box<dyn Detect>, String>> } }

pub struct RustCore {
    pub(crate) store: Arc<Store>,
    pub(crate) keychain: Arc<dyn Keyring>,
    detector: DetectorSlot,
    transport: TestTransport,
    /// The core's own events (detection progress, the sync engine's SyncStateChanged); the store's streams (Inbox,
    /// Counts, Thread) are subscribed on the store itself, under the same subscription id (`store_subs`).
    sinks: Sinks,
    store_subs: Mutex<HashMap<u64, u64>>,
    /// The sync (slice 2): one task per account on the core's own runtime, stopped by its notify.
    runtime: Option<tokio::runtime::Runtime>,
    deps: Arc<SyncDeps>,
    stops: Mutex<HashMap<String, Arc<Notify>>>,
    /// Slice 2b: the browser sign-ins of addresses being added, and the token endpoint their accounts refresh through.
    pub(crate) sign_in: SignIn,
    /// Delta 18: the sources of the last scan and every import run of this launch (one at a time).
    import: Arc<ImportBook>,
    /// engine-act: the mail actions (actions crate, the write-back worker) and the outbox with its timer.
    act: Act,
    /// search-forms: the owner's searches (the newest one's number, the last list sent).
    search: Arc<search_cmd::SearchBook>,
    /// search-forms part 3: every address that sent the owner mail or that the owner sent mail to, with its counts (people.rs).
    people: Arc<people::PeopleBook>,
    /// gap-decisions-real: the rows of the last Settings > Decisions preview and what Apply would write (decision_preview.rs); one per core.
    decision_preview: crate::decision_preview::PreviewBook,
}

impl Drop for RustCore {
    fn drop(&mut self) {
        for stop in self.stops.lock().map(|g| g.values().cloned().collect::<Vec<_>>()).unwrap_or_default() { stop.notify_waiters(); stop.notify_one(); }
        self.act.worker.stop();
        self.act.mail.stop();
        // the runtime is not waited on: a sync in flight is dropped where it stands, the store keeps every committed page
        if let Some(rt) = self.runtime.take() { rt.shutdown_background(); }
    }
}

const NOT_YET: &str = "The local engine does not do this yet.";

pub(crate) fn refused(words: &str) -> CoreError {
    CoreError::Action { failure: ActionFailed { account_name: String::new(), reason_words: words.to_string(), retryable: false, code: None } }
}
pub(crate) fn now_ms() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0) }

/// The provider shown on the account row, read from the incoming server's name.
pub fn provider_for_host(imap_host: &str) -> MailProvider {
    let h = imap_host.to_ascii_lowercase();
    if h.contains("gmail") || h.contains("google") { MailProvider::Google }
    else if h.contains("office365") || h.contains("outlook") { MailProvider::Microsoft }
    else if h.contains("icloud") || h.contains("me.com") { MailProvider::Icloud }
    else if h.contains("fastmail") { MailProvider::Fastmail }
    else { MailProvider::Imap }
}

impl RustCore {
    /// Delta 19: of the messages `MarkNotSpam` named, the ones still stored under an account's Junk-role folder (the server's own name for
    /// it, as the sync stored them) go back to the inbox with their server move queued (`junk_filter::mark_not_spam`); the ones that carried
    /// the local Spam word are already back (the actions crate moved them).
    fn leave_server_junk(&self, ids: Vec<String>) -> Result<(), CoreError> {
        let accounts: Vec<String> = self.store.account_summaries().map(|v| v.into_iter().map(|a| a.id).collect()).unwrap_or_default();
        let junk = crate::reads::folders_with_role(&self.store, &self.deps.roles, &accounts, sync_imap::FolderRole::Junk);
        let still: Vec<String> = ids.into_iter().filter(|id| self.store.message(id).ok().flatten().is_some_and(|m| m.folder != actions::constants::SPAM_FOLDER && junk.iter().any(|(a, f)| *a == m.account_id && *f == m.folder))).collect();
        if still.is_empty() { return Ok(()); }
        crate::junk_filter::mark_not_spam(&self.deps, still)?;
        self.act.worker.kick();
        Ok(())
    }

    /// The core the app opens: the store at `<data_dir>/whoosh.sqlite`, the macOS Keychain, the real detector built on the
    /// first lookup (so a missing DNS configuration refuses one lookup in words instead of the whole core).
    pub fn open_production(data_dir: &str, rig: bool) -> Result<Self, CoreError> {
        let mutant = if !rig { Mutant::None } else {
            match std::env::var("WHOOSH_CORE_MUTANT").as_deref() {
                Ok("no-mail-record") => Mutant::NoMailRecord, Ok("no-microsoft") => Mutant::NoMicrosoft, Ok("sync-never-stores") => Mutant::SyncNeverStores, Ok("flow-never-files") => Mutant::FlowNeverFiles, Ok("import-never-stores") => Mutant::ImportNeverStores, Ok("act-never-writes-back") => Mutant::ActNeverWritesBack,
                // body-r1: the reader waits for a body download on its own thread again (the perf check's red case)
                Ok("body-fetch-sync") => Mutant::BodyFetchSync,
                // search-forms: Search returns Ok and sends nothing (the hidden check's red case for the owner's search)
                Ok("search-never-answers") => Mutant::SearchNeverAnswers,
                // gap-moved-slice: a reply never reads the headers the store holds (outbox StoreReplyContext), so it asks the server for a mail that moved away
                Ok("reply-ignores-store") => Mutant::ReplyIgnoresStore,
                // fix-core (f4e1fbe8): the background download of every body never runs (the crawl is held for good), so the hidden `--slice-crawl` sees no body stored
                Ok("crawl-never-runs") => Mutant::CrawlNeverRuns,
                // fix-core (f2dd23de): the label rule count (`rule_set_count`, the labels screen's "N matches") answers 0 again
                Ok("rule-counts-zero") => Mutant::RuleCountsZero,
                // fix-core (d99929d0): a fetched body is stored as the raw message (no decoding of parts), so the hidden `--slice-body` sees boundaries and soft breaks
                Ok("body-stored-raw") => Mutant::BodyStoredRaw,
                _ => Mutant::None,
            }
        };
        // rig-core: WHOOSH_CORE_CHECK_PROVIDERS, the rig keyring file and the new mutant names, read under the same `rig` guard (rig.rs)
        let switches = crate::rig::from_env(rig, Path::new(data_dir), &|k| std::env::var(k).ok());
        // the hidden check's local IMAP and SMTP fakes speak clear text on 127.0.0.1; only the rig copy may ask for that
        let transport = if rig && std::env::var("WHOOSH_CORE_PLAIN_LOOPBACK").as_deref() == Ok("1") { TestTransport::PlainLoopback } else { TestTransport::Real };
        std::fs::create_dir_all(data_dir).map_err(|e| refused(&format!("The app cannot create its data folder {data_dir}: {e}")))?;
        let store = Store::open(&Path::new(data_dir).join("whoosh.sqlite")).map_err(|e| refused(&format!("The app cannot open its mail database: {e}")))?;
        // fix-realcore round 10, rig only: the owner's main106 state (filings with no tab row) on the asked open, before the seed's backfill (check_tabs.rs)
        crate::check_tabs::on_open(rig, Path::new(data_dir), &store);
        crate::rig::apply(&switches, store.path());
        // delta 17, the hidden check only: two labels seeded through the store's own calls (check_flow.rs)
        if rig && std::env::var("WHOOSH_CORE_CHECK_FLOW").as_deref() == Ok("1") {
            crate::check_flow::seed(&store).map_err(|e| refused(&e))?;
        }
        let ns = if rig { Namespace::rig() } else { Namespace::production() };
        // The hidden test copy keeps its secrets in memory: every rig copy is re-signed ad hoc, so an item one copy wrote
        // in the real Keychain is another app's to macOS, and the next copy's read raised a permission prompt on the
        // owner's screen (2026-11-09 22:46, "a lot of these"); a test copy never needs a secret to outlive its run.
        #[cfg(target_os = "macos")]
        let keychain: Arc<dyn Keyring> = if rig {
            Arc::new(secrets::RigFileKeychain::open(ns, switches.keys_file.clone().unwrap_or_default()).map_err(|e| refused(&format!("The test copy's keyring file could not be opened: {e}")))?)
        } else {
            // Local items only until the app is signed with a Developer ID and the iCloud Keychain entitlement (the owner's
            // outside-accounts item): an ad-hoc signed app asking for a synchronizable item is refused with errSecMissingEntitlement
            // (-34018), which the owner met on main47 as "L'un des droits requis n'est pas présent".
            Arc::new(secrets::Keychain::with_sync(ns, secrets::SyncMode::LocalOnly))
        };
        #[cfg(not(target_os = "macos"))]
        let keychain: Arc<dyn Keyring> = Arc::new(secrets::MemoryKeychain::new(ns));
        let check = if rig { CheckServers::from_env() } else { None };
        let http = Arc::new(AuthorityHttp::new());
        let parts = SignInParts::production(rig, &http);
        let core = RustCore::assemble(store, keychain, DetectorSlot::Production { mutant, check, built: OnceLock::new() }, transport, mutant == Mutant::SyncNeverStores, mutant == Mutant::FlowNeverFiles, http, parts)?;
        core.act.never_write_back(mutant == Mutant::ActNeverWritesBack);
        core.deps.bodies.set_fetch_sync(mutant == Mutant::BodyFetchSync);
        // only the rig copy reads WHOOSH_CORE_MUTANT (`mutant` is None when `rig` is false), so the owner's copy always answers
        core.search.set_never_answers(mutant == Mutant::SearchNeverAnswers);
        outbox::adapters::reply::ignore_stored_headers(mutant == Mutant::ReplyIgnoresStore);
        if mutant == Mutant::CrawlNeverRuns { core.deps.bodies.hold_crawl(true); }
        rule_counts::set_zero(mutant == Mutant::RuleCountsZero);
        crate::bodies::set_store_raw(mutant == Mutant::BodyStoredRaw);
        // fix-core (f4e1fbe8), the hidden check only: `WHOOSH_CORE_CHECK_CONDITIONS=battery:<percent>` and/or `heat` (comma between) put this core in the state the shell
        // reports on a Mac on battery or under heat, so a bundle can show the crawl's pause without a battery of its own (crawl.rs `conditions_from_check`)
        if let Some(c) = rig.then(|| std::env::var("WHOOSH_CORE_CHECK_CONDITIONS").ok()).flatten().and_then(|v| crate::bodies::crawl::conditions_from_check(&v)) {
            core.deps.bodies.pin_conditions(c);   // fix-realcore round 12: the shell's later reading of the Mac no longer replaces it
        }
        core.start_stored_accounts();
        // delta 18, the hidden check only: the import the first-start step will start, run once on a fixture folder (check_import.rs)
        core.import.set_never_store(mutant == Mutant::ImportNeverStores);
        if let Some(home) = rig.then(|| std::env::var("WHOOSH_CORE_CHECK_IMPORT").ok()).flatten() {
            crate::check_import::run(core.import.clone(), core.store.clone(), home.into());
        }
        // search-forms, the hidden check only: the "consultant"/"consultants" mail pair (search_cmd.rs CHECK_PAIR)
        if rig && std::env::var("WHOOSH_CORE_CHECK_SEARCH").as_deref() == Ok("1") {
            search_cmd::check_seed(core.store.clone(), core.deps.bodies.clone());
        }
        Ok(core)
    }

    /// The core over given parts (tests and future callers): a store, a keychain, a detector and the validation transport.
    /// Nothing syncs until an account is added or `start_stored_accounts` is called.
    pub fn with_parts<K: Keyring + 'static>(store: Store, keychain: Arc<K>, detector: Box<dyn Detect>, transport: TestTransport) -> Self {
        RustCore::assemble(store, keychain, DetectorSlot::Ready(detector), transport, false, false, Arc::new(AuthorityHttp::new()), SignInParts::quiet()).expect("the sync runtime starts")
    }

    /// As `with_parts`, with the browser sign-in built from `parts` (slice 2b: a local authority, a test browser, a mutant).
    pub fn with_sign_in<K: Keyring + 'static>(store: Store, keychain: Arc<K>, detector: Box<dyn Detect>, transport: TestTransport, parts: SignInParts) -> Self {
        RustCore::assemble(store, keychain, DetectorSlot::Ready(detector), transport, false, false, Arc::new(AuthorityHttp::new()), parts).expect("the sync runtime starts")
    }

    /// As `with_parts`, with the sync's mutant: every page comes back empty (the proof that the tests see a sync that stores nothing).
    pub fn with_parts_never_storing<K: Keyring + 'static>(store: Store, keychain: Arc<K>, detector: Box<dyn Detect>, transport: TestTransport) -> Self {
        RustCore::assemble(store, keychain, DetectorSlot::Ready(detector), transport, true, false, Arc::new(AuthorityHttp::new()), SignInParts::quiet()).expect("the sync runtime starts")
    }

    fn assemble(store: Store, keychain: Arc<dyn Keyring>, detector: DetectorSlot, transport: TestTransport, never_store: bool, never_files: bool, http: Arc<AuthorityHttp>, parts: SignInParts) -> Result<Self, CoreError> {
        let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).thread_name("whoosh-sync").enable_all().build()
            .map_err(|e| refused(&format!("The app could not start its mail sync: {e}")))?;
        let store = Arc::new(store);
        // labels-r5 round 6: the built-in labels are empty shells on a new installation; fill each once (what the owner wrote is never touched; stamped 0 like the migration's own seeds, so any edit wins)
        if let Err(e) = store.seed_builtin_labels(0) { eprintln!("labels: the built-in labels could not be seeded: {e}"); }
        let sinks: Sinks = Arc::new(Mutex::new((0, HashMap::new())));
        let roles: Roles = Arc::new(Mutex::new(HashMap::new()));
        let label_model = Arc::new(crate::decision_model::LabelModel::new(store.clone(), keychain.clone()));
        let filing = Arc::new(Filing::start(store.clone(), roles.clone(), never_files, label_model));
        // fix-aisort: Laya's "no" rows kept before an unreadable answer stopped counting as one are cleared once; their mails are asked again
        crate::label_ai_failure::clear_old_nos_once(&store);
        // fix-aisort round 4: a changed question (its form or its trust level) clears the model's kept "no" (the marker beside the database)
        crate::label_ai_failure::clear_on_question_change(&store);
        // fix-aisort round 9: no judge mark yet = the kept "no" were never judged by the judge in force: they go once, now, not at a first answer
        crate::label_ai_failure::clear_without_judge_mark(&store);
        filing.retry_waiting();
        let bodies = Arc::new(crate::bodies::Bodies::new(store.clone(), sinks.clone(), runtime.handle().clone()));
        // the shell's word on the meaning model reaches this core from now on, not only after an account's first crawl kick
        bodies.register();
        let deps = Arc::new(SyncDeps {
            store: store.clone(), sinks: sinks.clone(), roles, filing, phases: Arc::new(Mutex::new(HashMap::new())), adapters: Arc::new(Mutex::new(HashMap::new())),
            bodies, plain_loopback: transport == TestTransport::PlainLoopback, never_store,
        });
        let sign_in = SignIn::new(keychain.clone(), http, parts);
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
        // engine-act: the actions commit locally and record their server write-back in the same transaction; the worker applies it
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let imap = Arc::new(ImapWriteBack::new(deps.clone(), runtime.handle().clone()));
        let worker = Worker::new(store.clone(), imap.clone(), clock.clone());
        worker.start();
        let mail = MailOut::open(store.clone(), deps.clone(), runtime.handle().clone(), keychain.clone(), sign_in.endpoint.clone(), transport, clock.clone())?;
        mail.start();
        let env = actions::Env::new(store.clone(), imap.clone(), clock).with_send_canceller(Arc::new(CancelViaOutbox(mail.outbox.clone()))).with_ledger(Arc::new(Ledger));
        crate::replies_work::install(&store, &sinks, &deps.bodies, keychain.clone(), env.clone()); // reply-engine: the draft engine behind ReplyWork
        let act = Act { actions: actions::Actions::new(env), worker, mail, imap };
        let people_book = people::PeopleBook::start(store.clone(), deps.roles.clone());
        Ok(RustCore { store, keychain, detector, transport, sinks, store_subs: Mutex::new(HashMap::new()), runtime: Some(runtime), deps, stops: Mutex::new(HashMap::new()), sign_in, import: Arc::new(ImportBook::new(home)), act, search: Arc::new(search_cmd::SearchBook::default()), people: people_book, decision_preview: Default::default() })
    }

    /// Starts the sync of every stored account: a password account with its password read back from the Keychain, an OAuth2
    /// account (slice 2b) with its token rebuilt from the Keychain. An account whose credential cannot be read says so on its
    /// sync state and is not synced; a refresh the provider refuses lands there too, in its words, when the sync signs in.
    pub fn start_stored_accounts(&self) {
        for a in self.store.sync_accounts().unwrap_or_default() { self.start_stored_account(a); }
    }

    /// One stored account's sync, as `start_stored_accounts` starts it (also how a resumed account starts again, account_cmds.rs).
    /// real-accounts: THE one place the sync reads `paused`: a paused account is not started, so nothing is fetched for it until it is resumed.
    pub(crate) fn start_stored_account(&self, a: store::SyncAccount) {
        if a.account.paused { return; }
        let Some(config) = a.config.clone() else { return };
        if a.oauth || config.auth_method == AuthMethod::Oauth2 { self.start_stored_oauth(a.account, &a.keychain_ref, &config); return; }
        let password = a.keychain_ref.split_once('\u{1f}')
            .map(|(service, account)| KeychainRef { service: service.to_string(), account: account.to_string() })
            .ok_or_else(|| "the stored credential reference has no separator".to_string())
            .and_then(|r| ReadSecret::execute(&*self.keychain, &r).map_err(|e| e.to_string()))
            .and_then(|s| String::from_utf8(s.expose().to_vec()).map_err(|e| e.to_string()));
        match password {
            Ok(p) => self.start_sync(a.account, &config, sync_imap::Auth::Password(p)),
            Err(why) => self.record_failure(&a.account, format!("The app could not read this account's password from the Keychain: {why}")),
        }
    }

    pub(crate) fn record_failure(&self, account: &Account, words: String) {
        let failure = ActionFailed { account_name: account.display_name.clone(), reason_words: words, retryable: false, code: None };
        let patch = sync_engine::StatePatch { last_error: Some(Some(failure)), ..Default::default() };
        if let Ok(state) = sync_engine::state::write_state(&self.store, &account.id, patch) {
            emit(&self.sinks, StreamKind::SyncStatus, CoreEvent::SyncStateChanged { state });
        }
    }

    /// Starts (or restarts) one account's sync on the core's runtime: LOGIN with a password, or XOAUTH2 with a token source.
    pub(crate) fn start_sync(&self, account: Account, config: &AccountConfig, auth: sync_imap::Auth) {
        let Some(rt) = self.runtime.as_ref() else { return };
        let adapter = match sync::adapter_for(&account, config, auth, self.deps.plain_loopback) {
            Ok(a) => Arc::new(a),
            Err(why) => return self.record_failure(&account, why),
        };
        let stop = Arc::new(Notify::new());
        if let Some(old) = self.stops.lock().unwrap_or_else(|e| e.into_inner()).insert(account.id.clone(), stop.clone()) { old.notify_waiters(); old.notify_one(); }
        rt.spawn(sync::run_account(self.deps.clone(), account, adapter, stop));
    }

    /// Delta 17: sets (or clears) the engine behind the Labels page's model entry, the decision crate's own `DecisionPort` (the local
    /// model or Jev). None set = the entry rules and the exit filters still run; the model step is absent and `Filed::ai` says so.
    /// ai-settings-core: the model a job uses now, with this core's keychain (`ai_settings::model_for`); the refusal in words when none is set.
    pub fn model_for(&self, job: crate::ai_settings::AiJob) -> Result<crate::ai_settings::ModelChoice, CoreError> { crate::ai_settings::model_for(&self.store, &self.keychain, job) }

    pub fn set_decision_port(&self, port: Option<Arc<dyn DecisionPort>>) { self.deps.filing.set_port(port); }

    /// engine-act-r6 round 7: what the label AI can do now, for the filing record (`message/<id>/filing`): it can answer (a port the app or a check
    /// set, or the saved model with nothing refusing it), it is chosen but refuses (its words: Laya without its weights, Jev without its key or
    /// consent), or no model is chosen.
    pub(crate) fn filing_ai(&self) -> crate::label_flow::FilingAi {
        use crate::label_flow::FilingAi;
        if self.deps.filing.ai_can_answer() { return FilingAi::Configured; }
        if self.deps.filing.port().is_none() { return FilingAi::NoProvider; }
        FilingAi::Refused(self.deps.filing.label_model().refusal().map(|(_, w)| w).unwrap_or_else(|| "the decision model cannot answer now".into()))
    }

    /// Delta 18: where `ImportScan` looks for Apple Mail (`<home>/Library/Mail`). The app's own home folder unless a test or the hidden
    /// check points it at a fixture tree.
    pub fn set_import_home(&self, home: std::path::PathBuf) { self.import.set_home(home); }

    /// Delta 18, tests only: called after every progress report of an import run, on the run's thread, so a test can stop a run between
    /// two batches. Nothing in production sets it.
    pub fn set_import_pace(&self, pace: Option<ImportPace>) { self.import.set_pace(pace); }

    /// Delta 18: marks every reply reminder whose time came with no reply by then as fired (once each) and returns the sent messages it
    /// marked, so a notification goes out once per reminder. The engine calls it after each arrival; no timer exists yet.
    pub fn fire_due_reminders(&self) -> Vec<String> { crate::reminders::fire_due_and_return(&self.store, &self.deps.roles, now_ms()) }

    /// engine-act: runs the server write-backs that are due now, on the caller's thread (the worker does it by itself after every
    /// action; the tests and the hidden check call it to read the result without waiting for the worker's wake).
    pub fn run_write_back(&self) -> crate::writeback::WorkReport { self.act.worker.run_once() }

    /// engine-act: ticks the outbox now (the timer does it every OUTBOX_TICK_MS while an entry is open).
    pub fn tick_outbox(&self) { self.act.mail.tick() }

    /// The store the core reads and writes, for a caller that stores mail beside the sync (the tests; a future send path storing the
    /// owner's reply). Tell the label flow about such messages with `note_stored`.
    pub fn store(&self) -> Arc<Store> { self.store.clone() }

    /// Delta 17: messages were stored by someone other than the sync (the owner's reply from the send path, a test): they run the
    /// labelling order as arrivals, and the older messages of their threads are re-evaluated (a reply flips "unanswered"). Returns at once;
    /// `flush_filing` waits for the worker.
    pub fn note_stored(&self, ids: Vec<String>) { self.deps.filing.expect(ids); self.deps.filing.kick(); }

    /// Waits until the label flow has filed everything the sync handed it so far (the tests and the hidden check read after it).
    pub fn flush_filing(&self) { self.deps.filing.flush(); }

    /// Runs the labelling order now on stored messages, as arrivals (the engine does it by itself for every message the sync stores;
    /// this is the same use case for a caller that wants the report). INBOX messages only.
    pub fn file_stored_messages(&self, ids: &[String]) -> Result<Vec<crate::Filed>, String> {
        crate::file_arrivals::run(&self.store, self.deps.filing.port(), &self.deps.roles, ids, now_ms())
    }

    /// Re-evaluates stored messages on their current facts (a reply, a star, a read): the entry cards, then the exit filter of the
    /// label the AI chose. Never asks the model. The engine does it by itself when the sync stores a flipped fact.
    pub fn refile_flipped(&self, ids: &[String]) -> Result<Vec<crate::Moved>, String> {
        crate::refile_flipped::run(&self.store, self.deps.filing.port(), &self.deps.roles, ids, now_ms())
    }

    /// Where one account's sync stands (None: it never started in this process).
    pub fn sync_phase(&self, account_id: &str) -> Option<SyncPhase> { self.deps.phases.lock().ok().and_then(|g| g.get(account_id).cloned()) }

    /// body-r1, tests and the hidden check: where one account's bodies are downloaded from (the sync registers the account's own
    /// adapter when it starts; this replaces it, for a fake that answers late or refuses).
    pub fn set_body_source(&self, account_id: &str, source: Arc<dyn crate::bodies::BodySource>) { self.deps.bodies.set_source(account_id, source); }

    /// body-r1: downloads the newest bodies of one folder now, as the sync does after each fetch (BODY_PREFETCH_COUNT newest,
    /// BODY_PREFETCH_BATCH per server call). Returns at once; `settle_bodies` waits.
    pub fn prefetch_bodies(&self, account_id: &str, folder: &str) { self.deps.bodies.folder_synced(account_id, folder); self.deps.bodies.kick(account_id); }

    /// body-r1: waits until no body download runs (true), or `within` passed (false).
    pub fn settle_bodies(&self, within: std::time::Duration) -> bool { self.deps.bodies.settle(within) }

    /// sync-full-r1, tests and the hidden check: the background bodies, crawl and indexer (bodies.rs, crawl.rs, indexer.rs). The shell
    /// reaches them through the free functions `set_background_conditions`, `note_owner_activity` and `set_background_embedder`.
    pub fn background(&self) -> Arc<crate::bodies::Bodies> { self.deps.bodies.clone() }

    /// A stored body as the reader shows it: the text (made from the sanitised html when the message has no text part).
    fn stored_body(parts: &store::BodyParts) -> Option<MessageBody> {
        if parts.body_text.is_none() && parts.body_html.is_none() { return None; }
        let text = parts.body_text.clone().unwrap_or_else(|| parts.body_html.as_deref().map(parse::safe_html_to_text).unwrap_or_default());
        Some(reads::body_from_stored(&parts.message_id, text, parts.body_html.clone(), parts.trackers_blocked, parts.prepared.as_deref()))
    }

    fn run_detect(&self, email: &str, on_step: &mut dyn FnMut(autoconfig::Step)) -> Result<autoconfig::DetectionResult, String> {
        match &self.detector {
            DetectorSlot::Ready(d) => Ok(d.detect(email, on_step)),
            DetectorSlot::Production { mutant, check, built } => {
                if let Some(found) = check.as_ref().and_then(|c| c.answer(email, on_step)) { return Ok(found); }
                let d = built.get_or_init(|| {
                    let http = autoconfig::adapters::http_reqwest::ReqwestHttp::new()?;
                    let dns = autoconfig::adapters::dns_hickory::HickoryDns::new()?;
                    let probe = autoconfig::adapters::tls_probe::TlsProbe::new()?;
                    Ok(match mutant {
                        Mutant::None | Mutant::SyncNeverStores | Mutant::FlowNeverFiles | Mutant::ImportNeverStores | Mutant::ActNeverWritesBack | Mutant::BodyFetchSync | Mutant::SearchNeverAnswers | Mutant::ReplyIgnoresStore | Mutant::CrawlNeverRuns | Mutant::RuleCountsZero | Mutant::BodyStoredRaw => Box::new(autoconfig::Detector::new(http, dns, probe)) as Box<dyn Detect>,
                        Mutant::NoMailRecord => Box::new(autoconfig::Detector::new(http, NoMailRecord(dns), probe)),
                        Mutant::NoMicrosoft => Box::new(autoconfig::Detector::new(NoAutodiscover(http), NoMailRecord(dns), probe)),
                    })
                });
                match d { Ok(d) => Ok(d.detect(email, on_step)), Err(e) => Err(e.clone()) }
            }
        }
    }

    fn emit(&self, stream: StreamKind, event: CoreEvent) { emit(&self.sinks, stream, event) }

    /// The contract's account config as the sign-in check's config.
    pub fn validation_config(&self, config: &AccountConfig) -> sync_imap::ValidationConfig {
        // clear text only toward this Mac's own address: a plain-loopback core asked for any other host signs in for real
        let plain = self.transport == TestTransport::PlainLoopback && sync::is_loopback(&config.imap_host);
        let imap_security = if plain { sync_imap::Security::PlainLoopbackOnly } else {
            match config.imap_security { ConnectionSecurity::Ssl => sync_imap::Security::Tls, ConnectionSecurity::Starttls => sync_imap::Security::StartTls }
        };
        let smtp_security = if plain { send::Security::Plaintext } else {
            match config.smtp_security { ConnectionSecurity::Ssl => send::Security::Implicit, ConnectionSecurity::Starttls => send::Security::StartTls }
        };
        let smtp = if config.smtp_host.trim().is_empty() { None } else {
            Some(sync_imap::SmtpEndpoint { host: config.smtp_host.clone(), port: config.smtp_port, security: smtp_security })
        };
        sync_imap::ValidationConfig { username: config.username.clone(), imap: sync_imap::ImapEndpoint { host: config.imap_host.clone(), port: config.imap_port, security: imap_security }, smtp, extra_roots_der: vec![] }
    }

    pub(crate) fn validate(&self, config: &AccountConfig, password: &SecretText) -> ValidationReport {
        if config.auth_method == AuthMethod::Oauth2 {
            let step = ValidationStep { kind: ValidationStepKind::ImapLogin, ok: false, words: "This account signs in with its provider's page. Use Sign in with your organisation's app ID.".into() };
            return ValidationReport::from_steps(vec![step], vec![], 0, Some(config.imap_host.clone()));
        }
        if password.value.is_empty() {
            let step = ValidationStep { kind: ValidationStepKind::ImapLogin, ok: false, words: "Type the password (or an app password) for this account.".into() };
            return ValidationReport::from_steps(vec![step], vec![], 0, Some(config.imap_host.clone()));
        }
        self.validate_with(config, &sync_imap::Credential::Password(password.value.clone()))
    }

    /// The sign-in check with a credential: a password (LOGIN), or an OAuth2 access token (XOAUTH2 on IMAP and SMTP).
    pub(crate) fn validate_with(&self, config: &AccountConfig, cred: &sync_imap::Credential) -> ValidationReport {
        let vc = self.validation_config(config);
        let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                let step = ValidationStep { kind: ValidationStepKind::ImapConnect, ok: false, words: format!("The app could not start its network worker: {e}") };
                return ValidationReport::from_steps(vec![step], vec![], 0, Some(config.imap_host.clone()));
            }
        };
        // A sign-in check started from inside an async caller runs on its own thread, so the blocking wait never nests.
        let report = std::thread::scope(|s| s.spawn(|| rt.block_on(sync_imap::validate(&vc, cred))).join());
        match report {
            Ok(r) => map::validation(r, config),
            Err(_) => {
                let step = ValidationStep { kind: ValidationStepKind::ImapConnect, ok: false, words: "The sign-in check stopped unexpectedly. Try again.".into() };
                ValidationReport::from_steps(vec![step], vec![], 0, Some(config.imap_host.clone()))
            }
        }
    }

    fn add_account(&self, address: String, display_name: Option<String>, config: AccountConfig, password: SecretText) -> Result<CommandOutcome, CoreError> {
        let address = address.trim().to_string();
        let fail = |words: String, retryable: bool| CoreError::Action { failure: ActionFailed { account_name: address.clone(), reason_words: words, retryable, code: None } };
        if !address.contains('@') { return Err(fail("Type the whole address, with its @.".into(), false)); }
        if config.auth_method == AuthMethod::Oauth2 { return self.add_oauth_account(address, display_name, config); }
        if self.store.account_summaries().map_err(|e| fail(e.to_string(), true))?.iter().any(|a| a.address.eq_ignore_ascii_case(&address)) {
            return Err(fail(format!("{address} is already an account on this Mac."), false));
        }
        let report = self.validate(&config, &password);
        if !report.ok { return Err(fail(report.summary_words.clone(), true)); }
        if crate::rig::on(self.store.path(), crate::rig::RigMutant::NoAccountStored) { return Ok(CommandOutcome { undo_entry_id: None, created_id: None }); }
        let r = StoreSecret::execute(&*self.keychain, SecretKind::AccountCredential, &address.to_ascii_lowercase(), &Secret::new(password.value.as_bytes().to_vec()))?;
        let keychain_ref = format!("{}\u{1f}{}", r.service, r.account);
        let name = display_name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| address.clone());
        let provider = provider_for_host(&config.imap_host);
        let id = self.store.add_account(&address, &name, provider, &keychain_ref, false, config.clone(), now_ms()).map_err(|e| fail(e.to_string(), false))?;
        // slice 2: the new account starts syncing at once (its first screen, then its history)
        match self.store.sync_accounts().ok().and_then(|all| all.into_iter().find(|a| a.account.id == id)) {
            Some(a) => self.start_sync(a.account, &config, sync_imap::Auth::Password(password.value)),
            None => eprintln!("add {id}: the new account was not read back, so its sync did not start"),
        }
        Ok(CommandOutcome { undo_entry_id: None, created_id: Some(id) })
    }
}

impl RustCore {
    /// labels-r5 round 6: the shell hands the core its on-device decision model (Laya: a `DecisionPort` over the typed-decisions runtime, loaded on
    /// its first question and unloaded when idle by the shell); the label AI uses it when the decision model in force is Local. None takes it back.
    /// TEST SEAM (labels-r5 round 7): what a Jev request goes out over; the default refuses every send until the provider connection is shared.
    #[doc(hidden)]
    pub fn set_decision_transport(&self, t: std::sync::Arc<dyn ai_providers::transport::Transport>) { self.deps.filing.label_model().set_jev_transport(t); }

    /// TEST SEAM (labels-r5 round 8): Jev's REAL transport, with its address swapped for a fake provider on 127.0.0.1:`port`.
    #[doc(hidden)]
    pub fn set_decision_loopback(&self, port: u16) { self.deps.filing.label_model().set_jev_loopback(port); }

    pub fn set_laya_port(&self, port: Option<std::sync::Arc<dyn contracts::DecisionPort>>) {
        let handed = port.is_some();
        self.deps.filing.label_model().set_laya(port);
        // fix-aisort round 9: the start's retry may run before the shell hands its model over: when the start cleared the kept "no", a model
        // handed over asks the waiting mails now (only then: a store with nothing cleared keeps today's moments, the sync's settle)
        if handed && crate::label_ai_failure::cleared_pending(self.store.path()) { self.deps.filing.retry_waiting(); }
    }

    /// fix-aisort round 5: the hidden check `laya-sanity.py` (rig only, its two variables set) asks the shell's Laya through the label flow's port.
    pub(crate) fn start_laya_check(&self, rig: bool) {
        // round 6: the meaning model as the shell hands it to the index, read only (nothing is indexed by the check)
        let indexer = self.deps.bodies.indexer.clone();
        crate::check_laya::start(rig, self.deps.filing.label_model().on_device().map(|(port, _)| port), Box::new(move || indexer.embedder()));
    }

    /// TEST SEAM (search-forms part 3): the people list's first build waits `ms` before each of its steps (0: no wait, production),
    /// process-wide, so a test can read while the list is still being built.
    #[doc(hidden)]
    pub fn set_people_build_pause(ms: u64) { people::set_build_pause_ms(ms); }

    /// Whether a model can answer the label AI now: one is in force (key, consent and model for Jev; the shell's port for Laya) and the last on-device
    /// judgement did not fail. The re-run, its candidates and its preview all read this one answer.
    fn ai_can_answer(&self) -> bool { self.deps.filing.ai_can_answer() }

    /// `label/<id>/rerun/<model_pending|state|total|done|filed|refiled|unfiled|candidates|beyond|preview>` for a label that exists. `state` is "running" while the
    /// worker judges, else "done" after a run, else "idle"; `candidates` is how many messages a re-run would look at now (the Save offer's number).
    fn rerun_state(&self, path: &str) -> Option<String> {
        let parts: Vec<&str> = path.split('/').collect();
        let ["label", id, "rerun", leaf] = parts.as_slice() else { return None };
        self.store.labels_full().ok()?.iter().any(|l| l.id == *id).then_some(())?;
        let st = self.deps.filing.rerun_status(id);
        Some(match *leaf {
            "model_pending" => st.pending.to_string(),
            "state" => if st.running { "\"running\"" } else if st.total > 0 || st.filed > 0 { "\"done\"" } else { "\"idle\"" }.to_string(),
            "total" => st.total.to_string(), "done" => st.done.to_string(), "filed" => st.filed.to_string(),
            "refiled" => st.refiled.to_string(), "unfiled" => st.unfiled.to_string(),
            // `beyond`: older inbound messages the next step will judge (0 when this one reaches the oldest): "500 of N sorted, press Re-run to continue"
            "candidates" => crate::flow_ctx::rerun_candidate_count(&self.store, &self.deps.filing.rerun_book(), id, self.ai_can_answer())?.0.to_string(),
            "beyond" => crate::flow_ctx::rerun_beyond(&self.store, &self.deps.filing.rerun_book(), id, self.ai_can_answer())?.to_string(),
            // what a re-run would do, in the foot's terms, from the same candidates the run uses; `ai_words` is the one line when the AI cannot answer
            "preview" => crate::flow_ctx::rerun_preview(&self.store, &self.deps.filing.rerun_book(), id, self.ai_can_answer(), self.deps.filing.label_model().refusal())?,
            _ => return None,
        })
    }
}

impl WhooshCore for RustCore {
    fn run(&self, command: Command) -> Result<CommandOutcome, CoreError> {
        // delta 16: Primary and All are views; a command that treats one as a label is refused in words (VIEW_REFUSAL_CODE)
        check_view_command(command.clone())?;
        match command {
            Command::AddAccountConfigured { address, display_name, config, password } => self.add_account(address, display_name, config, password),
            Command::SetOAuthClient { account_draft_id, provider, client_id } => self.sign_in.set_client(&account_draft_id, provider, &client_id),
            Command::StartOAuthSignIn { account_draft_id } => self.sign_in.start(&account_draft_id),
            // delta 17: the label flow's six commands (cards, AI inputs, fallback, chains), on the store
            c if crate::label_flow::is_flow_command(&c) => {
                // round 11: a changed step 1 or AI step judges the label's waiting mails now, in the background
                let label = match &c {
                    Command::SetLabelCard { label_id, .. } | Command::SetLabelAiReads { label_id, .. } | Command::AddLabelChain { label_id, .. }
                    | Command::UpdateLabelChain { label_id, .. } | Command::RemoveLabelChain { label_id, .. } => Some(label_id.clone()),
                    _ => None,
                };
                let out = crate::label_flow::run(&self.store, c);
                if let (Ok(_), Some(l)) = (&out, label) { self.deps.filing.judge_label(self.store.path(), &l); }
                out
            }
            // delta 18: the mail import (scan, start, cancel) and dismissing a reply reminder
            Command::ImportScan { picked_paths } => self.import.scan(&self.store, &picked_paths).map(|_| CommandOutcome { undo_entry_id: None, created_id: None }),
            Command::ImportStart { source_id, into } => self.import.start(&self.store, &source_id, into),
            Command::ImportCancel { run_id } => self.import.cancel(&run_id),
            Command::ClearReplyReminder { draft_id } => crate::reminders::clear(&self.store, &draft_id, now_ms()),
            // engine-actions: mute and mark not done, leave a mailing list, remind if no reply (docs/lanes/engine-actions.md)
            Command::Mute { message_ids, muted } => crate::mute::run(&self.store, message_ids, muted),
            // vip-mark: the sender of a message joins or leaves the VIP list (vip_mark.rs, smart-inbox EditVips)
            Command::MarkVip { message_id, vip } => crate::vip_mark::run(&self.store, &message_id, vip),
            // delta 19: the server's copy is moved by the write-back ledger's rows, so the worker is woken once the local change is committed
            Command::Unarchive { message_ids } => {
                if let Err(e) = self.store.forget_server_copies(&message_ids) { eprintln!("unarchive: the server copies stayed listed: {e}"); }   // engine-act-r6, before the worker can drop the place
                let out = crate::unarchive::run(&self.store, &self.deps.roles, message_ids)?;
                self.act.worker.kick();
                Ok(out)
            }
            Command::Unsubscribe { message_id } => crate::unsubscribe::run(&self.store, &message_id, now_ms(), self.transport == TestTransport::PlainLoopback),
            Command::RemindIfNoReply { draft_id, at_ms } => crate::reminders::set(&self.store, &draft_id, at_ms, now_ms()),
            // engine-act: the mail actions through the actions crate, the sends through the outbox (docs/lanes/engine-act.md)
            // junk-r2: the owner's junk lesson is learned before the actions crate moves the message (its text is read where it is)
            Command::MarkSpam { message_ids } => {
                if let Err(e) = crate::junk_filter::learn(&self.store, &message_ids, store::JunkLesson::Junk) { eprintln!("junk: the lesson was not kept: {e}"); }
                self.act.run(Command::MarkSpam { message_ids })
            }
            // delta 19: the not-junk action. The lesson is learned BEFORE the actions crate moves the message (its text is read where it is),
            // then the messages leave Spam for the inbox; a row the sync stored under the server's own Junk folder name is moved back by
            // `junk_filter::mark_not_spam` (it knows the account's roles and queues the server move)
            Command::MarkNotSpam { message_ids } => {
                if let Err(e) = crate::junk_filter::learn(&self.store, &message_ids, store::JunkLesson::NotJunk) { eprintln!("junk: the lesson was not kept: {e}"); }
                if let Err(e) = self.store.forget_server_copies(&message_ids) { eprintln!("not spam: the server copies stayed listed: {e}"); }   // engine-act-r6, before the worker can drop the place
                let out = self.act.run(Command::MarkNotSpam { message_ids: message_ids.clone() })?;
                self.leave_server_junk(message_ids)?;
                Ok(out)
            }
            // engine-act-r6 round 3: Z of a core archive drops the stored server copy too (the same way back as Unarchive), before the worker runs
            // real-notes: Z when the newest entry is a note's (the mail actions' Undo cannot reverse a note's kind and would stay stuck on it)
            Command::Undo if crate::notes_cmds::newest_is_note(&self.store) => crate::notes_cmds::undo(&self.store),
            Command::Undo => { if let Some(e) = self.act.newest_undo() { if let Err(w) = self.store.forget_server_copies_of_undo(&e) { eprintln!("undo: the server copies stayed listed: {w}"); } } self.act.run(Command::Undo) }
            // engine-act-r4: Enter on a row opens its thread; every message of it is marked read, no undo entry (actions_bridge.rs `open_thread`)
            Command::OpenThread { thread_id } => self.act.open_thread(&thread_id),
            // engine-act-r6 round 2 (contracts delta 22): the files of a draft, and Forward carrying the original's files (draft_files.rs)
            Command::AddDraftAttachment { draft_id, source } => crate::draft_files::add(&self.store, &draft_id, source),
            Command::RemoveDraftAttachment { draft_id, attachment_id } => crate::draft_files::remove(&self.store, &draft_id, &attachment_id),
            // attach-engine (delta 23): answers at once, the fetch runs in the background (attachment_download.rs)
            Command::DownloadAttachment { attachment_id } => crate::draft_files::download::run(&self.store, &self.deps.bodies, &self.sinks, self.runtime.as_ref().map(|r| r.handle().clone()), &attachment_id),
            // engine-act-r6 round 6: Enter on a search keeps its words (recent_search.rs; state search/recent)
            Command::RecordSearch { query } => crate::recent_search::record(&self.store, &query, now_ms()),
            // search-forms (SPEC A141): answers at once; the ranked hits arrive on the Search stream, twice when the meaning comes later (search_cmd.rs)
            Command::Search { query } => {
                let parts = search_cmd::Parts { store: self.store.clone(), indexer: self.deps.bodies.indexer.clone(), roles: self.deps.roles.clone(), sinks: self.sinks.clone(), utc_offset_s: SystemClock.utc_offset_s() };
                self.search.start(parts, query)
            }
            Command::Forward { message_id } => crate::draft_files::forward(&self.store, &message_id, now_ms(), SystemClock.utc_offset_s()),
            // labels-r5 round 5: the decision model row (it answered not-built-yet)
            Command::SaveDecisionModel { model } => {
                // round 16: a saved decision model (the owner's choice, and the shell's re-save when Laya's weights arrive) asks afresh: the
                // remembered Laya failure goes and the mails that wait for the AI are asked again in the background
                let out = crate::decision_model::save(&self.store, model, now_ms());
                if out.is_ok() {
                    self.deps.filing.label_model().clear_laya_failure();
                    crate::label_ai_failure::ask_waiting_again(self.store.path());
                    self.deps.filing.retry_waiting();
                }
                out
            }
            // gap-decisions: the decision list of the Labels page, the rules the owner files under a node of the tree (decision_list.rs); each node's re-run files what its rules catch
            Command::SaveDecisionList { page_id, node_ids, rules } => {
                for id in crate::decision_list::save(&self.store, &page_id, &node_ids, &rules)? {
                    self.deps.filing.rerun(&id);
                }
                Ok(CommandOutcome { undo_entry_id: None, created_id: None })
            }
            // gap-preview-core: one page's list as the page holds it, run on the recent mail; the rows wait for `decision_preview()` (decision_preview.rs)
            Command::PreviewDecisionList { page_id, entries } => {
                crate::decision_preview::run(&self.decision_preview, &self.store, self.deps.filing.port(), &SystemClock, &page_id, &entries)?;
                Ok(CommandOutcome { undo_entry_id: None, created_id: None })
            }
            // gap-decisions-real: writes the last preview's placements for its page; stale or missing preview refused in words (decision_preview.rs `apply`); no undo entry, as the arrival path's writes have none
            Command::ApplyDecisionPreview => {
                crate::decision_preview::apply(&self.decision_preview, &self.store, now_ms())?;
                Ok(CommandOutcome { undo_entry_id: None, created_id: None })
            }
            // labels-r5 round 7 (delta 23): the owner's yes or no to sending the label AI's mail text to a hosted decision model
            Command::GiveDecisionConsent { provider, given } => crate::decision_model::give_consent(&self.store, provider, given, now_ms()),
            // labels-r5: "Re-run on N filed messages" (flow_ctx::rerun_label)
            // the re-run runs on the filing worker and answers at once (a model takes its own time): progress and result are state label/<id>/rerun/*
            Command::RerunLabel { label_id } => {
                if !self.store.labels_full().map_err(|e| CoreError::Unexpected { reason_words: e.to_string() })?.iter().any(|l| l.id == label_id) {
                    return Err(CoreError::NotFound { kind: "label".into(), id: label_id });
                }
                self.deps.filing.rerun(&label_id);
                Ok(CommandOutcome { undo_entry_id: None, created_id: None })
            }
            c if act::is_action(&c) => self.act.run(c),
            c if act::is_send(&c) => self.act.send(c),
            // delta 20: the AI replies settings and the one port for its working commands (replies.rs)
            c if crate::replies::is_reply_command(&c) => crate::replies::run(&self.store, c),
            // ai-settings-core: the provider key (checked in the background), the agents, the speech engine (ai_settings.rs)
            c if crate::ai_settings::is_command(&c) => crate::ai_settings::run(&self.store, &self.keychain, c),
            // real-folders: SaveSmartFolder, RenameSmartFolder, UpdateSmartFolder, DeleteSmartFolder (smart_folder_cmds.rs)
            c if smart_folder_cmds::is_command(&c) => smart_folder_cmds::run(&self.store, c),
            // real-compose: Reply all (a threaded draft answering everyone) and the account's signature (compose_cmds.rs)
            c if crate::compose_cmds::is_command(&c) => crate::compose_cmds::run(&self.store, c, SystemClock.utc_offset_s()),
            // real-settings: the notch, the notifications, the key preset and the split tabs (settings_cmds.rs); [dispatch line of real-settings]
            c if crate::settings_cmds::is_command(&c) => crate::settings_cmds::run(&self.store, c),
            // real-accounts: RenameAccount, PauseAccount, RemoveAccount, SetAccountIcon, SetAccountColour, ReorderAccounts, and the AddAccount refusal (account_cmds.rs)
            c if account_cmds::is_command(&c) => account_cmds::run(self, c),
            // real-notes: the notes, the outside apps' grants, the suggestions, MergeContacts, BlockDomain (notes_cmds.rs)
            c if crate::notes_cmds::is_command(&c) => crate::notes_cmds::run(&self.store, self.transport == TestTransport::PlainLoopback, c),
            c if label_cmds::is_command(&c) => {
                // round 11: a Save of the label sheet (rules, description) judges the label's waiting mails now, in the background
                let saved = match &c { Command::UpdateLabel { label_id, .. } => Some(label_id.clone()), _ => None };
                label_cmds::run(&self.store, c).map(|out| {
                    self.act.worker.kick();
                    if let Some(l) = saved.clone().or_else(|| out.created_id.clone()) { self.deps.filing.judge_label(self.store.path(), &l); }
                    out
                })
            }   // real-labels (label_cmds.rs): the worker carries a removal's server rows
            c if block_cmds::is_command(&c) => block_cmds::run(&self.store, c),   // fix-blockforget: BlockSender, one row of the block table (block_cmds.rs)
            c if crate::decision_teaching::is_command(&c) => crate::decision_teaching::run(&self.store, c),   // fix-teachcore: RefitDecisionModel (decision_teaching.rs)
            _ => Err(codes::refusal(NOT_YET, codes::NOT_BUILT_YET, "")),
        }
    }
    fn inbox(&self, query: InboxQuery) -> Vec<InboxRow> { self.inbox_rows(query, store::INBOX_PAGE_ROWS as u32) }
    fn inbox_rows(&self, query: InboxQuery, rows: u32) -> Vec<InboxRow> {
        // sync-words: opening the list of an account (its filter, or an account set) is the owner's action on it: a failed account
        // is synced again at once instead of at the end of its SYNC_BACKOFF_MAX_MS wait. Nothing happens for a healthy account.
        if let Some(named) = query_accounts(query.clone()) { for a in &named { sync::retry_now(a); } }
        // engine-act: the Outbox mailbox lists the open outbox entries (the fixture's entryRows)
        if query.mailbox == Some(Mailbox::Outbox) { return reads::outbox_rows(&self.store, &self.act.mail.open_entries(), &query); }
        // real-folders: a smart folder's list is its saved search (smart_folder_cmds.rs); reads::inbox answers none for it
        if query.smart_folder_id.is_some() { return smart_folder_cmds::inbox(self, &query); }
        reads::inbox_rows(&self.store, &self.deps.roles, &query, rows.max(1) as usize)
    }
    fn unsubscribe_method(&self, message_id: String) -> Option<UnsubscribeMethod> { crate::unsubscribe::method(&self.store, &message_id, self.transport == TestTransport::PlainLoopback) }
    fn attachments(&self, message_id: String) -> Vec<AttachmentFile> { self.store.attachments(&message_id).unwrap_or_default() }
    fn drafts(&self, thread_id: String) -> Vec<DraftSummary> { self.store.draft_summaries(&thread_id).unwrap_or_default() }
    // real-accounts: what RemoveAccount would take off this Mac, counted from the store
    fn account_removal(&self, account_id: String) -> Option<AccountRemoval> { self.store.account_removal(&account_id).ok().flatten() }
    fn counts(&self) -> Counts { self.store.counts().unwrap_or(Counts { tabs: vec![], mailboxes: HashMap::new() }) }
    fn thread(&self, thread_id: String) -> Option<MailThread> { self.store.thread(&thread_id).ok().flatten() }
    fn message(&self, message_id: String) -> Option<Message> { self.store.message(&message_id).ok().flatten() }
    // search-forms part 3: every person of the stored mail, the most mail first (people.rs)
    // real-notes: a card MergeContacts made shows once (notes_cmds.rs `fold_merged`)
    fn contacts(&self) -> Vec<Contact> { crate::notes_cmds::fold_merged(&self.store, self.people.contacts()) }
    // SPEC A153: the sender domains of received mail for the letters typed, from the people list in memory (people.rs)
    fn sender_domains(&self, prefix: String) -> Vec<SenderDomain> { self.people.sender_domains(&prefix) }
    fn contact(&self, address: String) -> Option<Contact> { crate::notes_cmds::merged_contact(&self.store, &address, self.people.contact(&address), &|| self.people.contacts()) }
    fn contact_messages(&self, contact_id: String) -> Vec<InboxRow> { self.store.contact_mail_ids(&contact_id).and_then(|ids| self.store.rows_for_messages(&ids)).unwrap_or_default() }   // fix-blockforget: a card's or a person's mail (contacts.rs)
    // rows-engine (SPEC A141): the search screen's rows, any folder, read from the local store only. The store's row is the row
    // `inbox` serves (reads::inbox filters and orders a list, it changes no field of a row), so nothing is added here.
    fn rows_for_messages(&self, message_ids: Vec<String>) -> Vec<InboxRow> { self.store.rows_for_messages(&message_ids).unwrap_or_default() }
    // gap-engine-tests (f2dd23de): the smart-folder editor's "N matches": the exact group of the search for the rule's words, from the store and the word index
    fn rule_match_count(&self, rule_words: String) -> u32 {
        let parts = search_cmd::Parts { store: self.store.clone(), indexer: self.deps.bodies.indexer.clone(), roles: self.deps.roles.clone(), sinks: self.sinks.clone(), utc_offset_s: SystemClock.utc_offset_s() };
        rule_counts::words_count(&parts, &rule_words)
    }
    // delta 16: a store seeded before the views keeps a built-in "primary" label row; it is a view, never served as a label
    fn labels(&self) -> Vec<MailLabel> { labels_without_views(self.store.labels_full().unwrap_or_default()) }
    fn pinned_labels(&self) -> Vec<MailLabel> { labels_without_views(self.store.pinned_labels().unwrap_or_default()) }
    // labels-r5: the real count over the stored inbox (it was a constant zero) and the real number of messages filed under the label
    fn rule_set_count(&self, rule_set: RuleSet) -> RuleSetCount {
        if rule_counts::zeroed() { return RuleSetCount { count: 0, unread: 0, newest_at_ms: None }; }   // the hidden check's mutant `rule-counts-zero`
        smart_inbox::corpus::rule_set_count(&self.store, rule_set)
    }
    // delta 20 + labels-r5: the matching message ids, newest first, from the corpus facts
    fn rule_set_matches(&self, rule_set: RuleSet, limit: u32) -> Vec<String> { smart_inbox::corpus::rule_set_matching_ids(&self.store, rule_set, limit as usize) }
    fn label_rerun_count(&self, label_id: String) -> u32 { self.store.label_filed_count(&label_id).unwrap_or(0) }
    fn suggestions(&self) -> Vec<Suggestion> { crate::notes_cmds::suggestions(&self.store) }
    fn blocks(&self) -> Vec<Block> { crate::notes_cmds::blocks(&self.store) }
    fn accounts(&self) -> Vec<AccountSummary> {
        // health = the account's sync failure, in the server's words
        let failures: HashMap<String, ActionFailed> = self.sync_states().into_iter().filter_map(|s| s.last_error.map(|f| (s.account_id, f))).collect();
        let mut out = self.store.account_summaries().unwrap_or_default();
        for a in &mut out { a.health = failures.get(&a.id).cloned(); }
        // real-compose: the saved signature of each account (SetSignature; compose_cmds.rs)
        let signatures = self.store.signatures_by_account().unwrap_or_default();
        for a in &mut out { a.signature = signatures.get(&a.id).cloned().unwrap_or_default(); }
        out
    }
    // real-folders: the saved folders, parents before children (smart_folder_cmds.rs)
    fn smart_folders(&self) -> Vec<SmartFolder> { smart_folder_cmds::folders(&self.store) }
    /// body-r1: answers at once. The stored body when there is one; else the decoded preview while the body downloads on the core's
    /// runtime (once per message, a failure not tried again before BODY_FETCH_RETRY_S), its landing announced by `ThreadChanged`.
    fn body(&self, message_id: String) -> Option<MessageBody> {
        let parts = self.store.body_parts(&message_id).ok().flatten()?;
        if let Some(b) = Self::stored_body(&parts) { return Some(b); }
        let bodies = &self.deps.bodies;
        if bodies.fetch_sync() {
            bodies.request_and_wait(&parts);
            if let Some(b) = self.store.body_parts(&message_id).ok().flatten().as_ref().and_then(Self::stored_body) { return Some(b); }
        } else {
            bodies.request(&parts);
        }
        Some(reads::body_from(&message_id, reads::shown_preview(&parts.preview), None, 0))
    }
    fn answer(&self, _conversation_id: String) -> Option<AskAnswer> { None }
    fn proposals(&self, _conversation_id: String) -> Vec<AgentProposal> { vec![] }
    // real-settings: the owner's saved settings, the documented defaults before the first save (settings_cmds.rs)
    fn notch_setting(&self) -> NotchSetting { crate::settings_cmds::notch(&self.store) }
    fn notch_alerts(&self) -> contracts::NotchAlerts { crate::settings_cmds::notch_alerts(&self.store) }
    fn notch_kind(&self, message_id: String) -> contracts::NotchKind { crate::settings_cmds::notch_kind(&self.store, &message_id) }
    fn notification_settings(&self) -> Vec<NotificationSetting> { crate::settings_cmds::notifications(&self.store) }
    fn ai_providers(&self) -> Vec<AiProvider> { crate::ai_settings::providers(&self.store) }
    fn agent_configs(&self) -> Vec<AgentConfig> { crate::ai_settings::agents(&self.store) }
    // labels-r5 round 5: the owner's saved choice (it was None); `SaveDecisionModel` is served in `run`
    // labels-r5 round 7 (delta 23): the stored consent (delta 23)
    fn decision_consent(&self, provider: DecisionProviderKind) -> bool { crate::decision_model::consent_given(&self.store, provider) }
    fn decision_model(&self) -> Option<DecisionModel> { crate::decision_model::read(&self.store) }
    fn decision_preview(&self) -> Vec<DecisionVerdict> { crate::decision_preview::rows(&self.decision_preview) }
    fn speech_engine(&self) -> SpeechEngine { crate::ai_settings::speech(&self.store) }
    fn mcp_grants(&self) -> Vec<McpGrant> { crate::notes_cmds::grants(&self.store) }
    fn split_tabs(&self) -> Vec<SplitTab> { crate::settings_cmds::split_tabs(&self.store) }
    fn snoozed_messages(&self) -> Vec<Snooze> {
        self.store.snoozed().unwrap_or_default().into_iter().map(|s| Snooze { message_id: s.row.message_id, return_at_ms: s.return_at_ms }).collect()
    }
    // gap-engine-tests (f2dd23de): the inbox mail the tab's rule would hold, read from the store (rule_counts.rs)
    fn split_tab_rule_match_count(&self, rule: SplitTabRule) -> u32 { rule_counts::split_tab_count(&self.store, rule) }
    fn notes(&self, target: NoteTarget) -> Vec<Note> { crate::notes_cmds::notes(&self.store, target) }
    fn signature(&self, address: String) -> Option<Signature> { crate::compose_cmds::signature(&self.store, &address) }
    fn shortcut_preset(&self) -> ShortcutPreset { crate::settings_cmds::shortcut_preset(&self.store) }
    fn detect_provider(&self, email: String) -> DetectionResult {
        let mut number = 0u32;
        let address = email.trim().to_string();
        let mut on_step = |s: autoconfig::Step| {
            number += 1;
            let step = map::step(s);
            self.emit(StreamKind::SyncStatus, CoreEvent::DetectionProgress { address: address.clone(), number, progress: DetectionProgress { step, found: false, words: step.words().to_string() } });
        };
        match self.run_detect(&address, &mut on_step) {
            Ok(r) => map::detection(r),
            Err(e) => DetectionResult::new(DetectionOutcome::Refused { words: format!("The app could not start looking up mail settings ({e}). Use Manual setup.") }, vec![], false, None),
        }
    }
    fn oauth_sign_in_state(&self, account_draft_id: String) -> OAuthSignInState { self.sign_in.state(&account_draft_id) }
    fn validate_account(&self, config: AccountConfig, password: SecretText) -> ValidationReport { self.validate(&config, &password) }
    // delta 16 views (reads::view_state) and delta 17 label-flow reads (label_flow::state): the views answer first, the flow next
    fn state(&self, path: String) -> Option<String> {
        reads::view_state(&self.store, &path)
            .or_else(|| (path == "provider_request/count").then(|| crate::ai_settings::provider_request_count().to_string()))   // fix-aicore F2: the off-device model requests (ai_settings.rs)
            .or_else(|| crate::thread_summary::state(&self.store, &self.keychain, &|| self.deps.filing.label_model().on_device().map(|(_, g)| g), &path))   // fix-aicore A: thread/<id>/summary (thread_summary.rs)
            // delta 22: draft/<id>/attachments, outbox/<id>/attachments; engine-act-r6 round 6: draft/<id>/body
            .or_else(|| crate::draft_files::state(&self.store, &path))
            .or_else(|| crate::check_tabs::state(&path))   // fix-realcore round 10: rig/builtin_tabs_dropped
            .or_else(|| (path == "background/pause").then(|| self.deps.bodies.pause_state()))   // fix-realcore round 13: the engine's own pause
            // fix-realcore round 15: one outbox entry's sending state (queued, sending, retrying with the next try, failed with its step, sent)
            .or_else(|| match path.split('/').collect::<Vec<_>>().as_slice() { ["outbox", id, "send_state"] => self.act.mail.send_state(id), _ => None })
            .or_else(|| (path == "outbox/open").then(|| self.act.mail.open_states()))
            // fix-realcore round 9 (item 7e1fc235): why the reader has no text for a mail the server lists but never sends (identifiers; null: nothing failed)
            .or_else(|| match path.split('/').collect::<Vec<_>>().as_slice() {
                ["message", id, "body_unavailable"] => Some(self.deps.bodies.body_unavailable(id).unwrap_or_else(|| "null".to_string())),
                _ => None,
            })
            // real-compose: draft/<id>/to, cc, subject, from_account_id, reply_to_thread_id; draft/count (compose_cmds.rs)
            .or_else(|| crate::compose_cmds::state(&self.store, &path))
            // engine-act-r6 round 5: rig/refused-outbound, the outbound attempts the rig copy refused (count, last host); the owner's copy serves nothing
            .or_else(|| (path == netguard::REFUSED_STATE_PATH && crate::rig::is_rig_process()).then(netguard::refused_json))
            // engine-act-r6 round 6: message/<id>/starred, /archived, /snoozed, snooze/<id>/return_ms, /return_local_time (message_state.rs); search/recent
            .or_else(|| crate::message_state::state(&self.store, &path, SystemClock.utc_offset_s()))
            .or_else(|| crate::recent_search::state(&self.store, &path))
            // search-forms: search/last/* (the last list the owner's search sent)
            .or_else(|| self.search.state(&path))
            // search-forms part 3: people/prefix/<letters>, people/address/<address>, people/count
            .or_else(|| self.people.state(&path))
            // engine-act-r6 round 6: the real app runs no MCP server (crate mcp is not in its graph), so the server is off; the lane that links one serves its state
            // real-notes: note/*, mcp_server/on, mcp_grant/<app>, mcp_request/*, suggestion/*, block/*, contact/*, contacts/count (notes_cmds.rs); an app with no
            // grant row falls through to state_extra's `false`, and the server reads false until the owner switches it on
            .or_else(|| crate::notes_cmds::state(&self.store, &path, &|| self.contacts().len()))
            // real-folders: smart_folder/count, smart_folder/<id>/{count,unread,threads,rule,parent}, smart_folder/<id or name>/{rule,exists}; asked before state_extra's `0`
            .or_else(|| smart_folder_cmds::state(self, &path))
            // real-settings: notch_setting/*, notification_setting/*, shortcut_preset/name, agent_consent/<provider>, split_tab/count|<tab>/order|enabled|rule and the name of an edited tab
            // (before state_extra: an edited tab's name wins over the label's)
            .or_else(|| crate::settings_cmds::state(&self.store, &path))
            // gap-engine-state: mcp_grant/<app>, split_tab/<tab>/name, smart_folder/<folder>/count (state_extra.rs)
            .or_else(|| state_extra::state(&self.store, &path))
            .or_else(|| state_headers::state(&self.store, &path))
            .or_else(|| crate::label_flow::state(&self.store, &path, &|| self.filing_ai()).flatten())
            .or_else(|| label_cmds::state(&self.store, &path))   // real-labels: labels/tree, labels/pinned, labels/count, label/<id>/<field>, label_assignment/<m>, label_removed/<m>/<l>
            // delta 18: import/sources, import/<run>/progress and /result; thread/<id>/reminder
            .or_else(|| self.import.state(&path))
            // engine-actions: message/<id>/muted, message/<id>/unsubscribe, /unsubscribed, /unsubscribe_method
            .or_else(|| crate::mute::state(&self.store, &path))
            // junk-r3: message/<id>/junk, why the message is where it is (score, mark, lesson, block, correspondent)
            .or_else(|| crate::junk_filter::state(&self.store, &path))
            .or_else(|| crate::unsubscribe::state(&self.store, self.transport == TestTransport::PlainLoopback, &path))
            .or_else(|| crate::replies::state(&self.store, &path))
            .or_else(|| crate::ai_settings::state(&self.store, &path))
            // labels-r5: label/<id>/rerun/model_pending, "true" while the last re-run left a message to the model that could not answer
            .or_else(|| self.rerun_state(&path))
            // labels-r5 round 5: label/<id>/flow/ai_refusal
            .or_else(|| self.deps.filing.label_model().state(&path))
            .or_else(|| crate::decision_model::consent_state(&self.store, &path))
            .or_else(|| crate::decision_teaching::state(&self.store, &path))   // fix-teachcore: decision_teaching/<page>/corrections|shared, /examples, /refits
            .or_else(|| crate::decision_preview::state(&self.decision_preview, &path))
            // real-accounts: account/count, account/order, account/<id>/*, account_icon/<id>, account_credential/<id>/stored, sync_state/<id>/*
            .or_else(|| account_cmds::state(self, &path))
            .or_else(|| match path.split('/').collect::<Vec<_>>().as_slice() { ["thread", id, "reminder"] => crate::reminders::state(&self.store, id, now_ms()), _ => None })
            // ownercopy-pieces (item c1436f49): index/pieces/<after rowid>/<limit>, counts only (indexer.rs piece_counts); the limit is at most INDEX_SCAN_ROWS
            .or_else(|| match path.split('/').collect::<Vec<_>>().as_slice() {
                ["index", "pieces", after, limit] => match (after.parse::<i64>(), limit.parse::<usize>()) {
                    (Ok(a), Ok(n)) if n > 0 => Some(self.deps.bodies.indexer.piece_counts(a, n.min(crate::bodies::indexer::INDEX_SCAN_ROWS))),
                    _ => None,
                },
                _ => None,
            })
    }
    fn outbox(&self) -> Vec<OutboxEntry> { self.act.mail.open_entries() }
    fn sync_states(&self) -> Vec<SyncState> {
        self.store.sync_state_rows().unwrap_or_default().into_iter().map(|r| SyncState {
            last_error: r.last_error_json.as_deref().and_then(sync_engine::parse_failure),
            account_id: r.account_id, history_fetched: r.history_fetched, messages_indexed: r.messages_indexed,
            provider_status_words: r.provider_status_words, last_synced_ms: r.last_synced_ms,
        }).collect()
    }
    fn subscribe(&self, stream: StreamKind, sink: Arc<dyn EventSink>) -> u64 {
        let id = {
            let mut g = match self.sinks.lock() { Ok(g) => g, Err(p) => p.into_inner() };
            g.0 += 1; let id = g.0; g.1.insert(id, (stream, sink.clone())); id
        };
        // the store produces the Inbox, Counts and Thread events of every write (the sync's pages included)
        let on_store = self.store.subscribe(stream, sink);
        self.store_subs.lock().unwrap_or_else(|e| e.into_inner()).insert(id, on_store);
        id
    }
    fn unsubscribe(&self, subscription: u64) {
        { let mut g = match self.sinks.lock() { Ok(g) => g, Err(p) => p.into_inner() }; g.1.remove(&subscription); }
        if let Some(s) = self.store_subs.lock().unwrap_or_else(|e| e.into_inner()).remove(&subscription) { self.store.unsubscribe(s); }
    }
}
