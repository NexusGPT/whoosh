//! Delta 18, part 1 (PRD A83, the first-start import step and Settings > Accounts "Import"): bring mail in from a local store. The
//! import crate (core/crates/import) reads Apple Mail's message files, an Outlook for Mac `.olm` export and a Thunderbird or Unix
//! mbox, and writes the same local database the sync writes to. This file is the CONTRACT between that crate (through the engine)
//! and the two screens that show it: what a scan finds, what a run is told, what a run reports, and the refusals in words.
//!
//! Three commands (`ImportScan`, `ImportStart`, `ImportCancel`, in command.rs) and three state reads, no new `WhooshCore` method:
//! the established route is `core state <path>`, so `RustCore` and `SpyCore` need no stub. The state values are JSON; the key
//! spelling is written once in docs/lanes/contracts.md "Delta 18" (snake_case, one object per record below).
//!
//! WHAT THIS FILE DOES NOT DO: it reads no folder and decides nothing about a message. A permission refusal from macOS arrives as
//! `ImportAccess::NeedsFullDiskAccess` on a source or as `ImportRefusal` on a run; the words are `import_access_words`.
use crate::CoreError;

/// What a source is. Closed: a fourth reader is a contract change with its own decision.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImportSourceKind { AppleMail, Olm, Mbox }

/// Whether the core could open the source when it scanned it. A source that is not `Readable` carries its words
/// (`import_access_words`) and cannot be started.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImportAccess { Readable, NeedsFullDiskAccess, NotFound, Unreadable }

/// One source a scan found. State `import/sources` is the array of these, newest scan, Apple Mail first, then the picked files in the
/// order they were picked. `message_count` is None when counting is not cheap (an mbox, until it is read): the UI then shows no
/// total and an indeterminate bar. `imported_count` and `last_run_ms` are what earlier runs of this same source stored (0 and None
/// for a source never imported), so Settings > Accounts can say "imported 12,418 messages on 8 November" and offer a re-run.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ImportSource {
    /// `import_source_id(kind, location)`: stable across scans, so a re-run is the same row.
    pub id: String,
    pub kind: ImportSourceKind,
    /// what the row is called: "Apple Mail", or the file's name ("Outlook archive.olm")
    pub name: String,
    /// where it is on this Mac, as the owner reads it ("~/Library/Mail", "~/Documents/Outlook archive.olm")
    pub location: String,
    pub message_count: Option<u64>,
    pub imported_count: u64,
    pub last_run_ms: Option<i64>,
    pub access: ImportAccess,
}

/// Where the messages of a run go. `Imported` = each message to the account whose address received it, else to the local
/// "Imported" account (what the import crate does today). `Account` = every message of the run to that account.
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum ImportInto { Imported, Account { account_id: String } }

/// Where a run is. `Refused` = the source could not be opened (the refusal is on the progress and the result); `Failed` = it opened
/// and stopped (the local database refused a write, say). `Cancelled` keeps every message stored before the cancel.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImportRunState { Running, Finished, Cancelled, Refused, Failed }

/// Why a run did not run to the end, as a code and a sentence. `code` is one of the `IMPORT_*_CODE` constants.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ImportRefusal { pub code: String, pub words: String }

/// What a run reports while it goes (named `ImportRunProgress`: the import crate has its own `ImportProgress` and the engine imports both). State `import/<run id>/progress`. `done` counts messages looked at; `total` is the source's count
/// when known (an estimate: `done` may pass it, `import_progress_fraction` clamps). `current` is the mailbox or file being read.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ImportRunProgress {
    pub run_id: String,
    pub source_id: String,
    pub state: ImportRunState,
    pub done: u64,
    pub total: Option<u64>,
    pub current: String,
    pub stored: u64,
    pub skipped: u64,
    /// set exactly when `state` is Refused or Failed
    pub refusal: Option<ImportRefusal>,
}

/// A message or file the run left out, named, with the reason in words.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ImportSkipped { pub origin: String, pub words: String }

/// Messages given to one account.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ImportPerAccount { pub account_id: String, pub stored: u64 }

/// What a run ended with. State `import/<run id>/result`: absent (None) while the run is Running. `seen` = stored + already_there +
/// skipped.len(): the three never overlap, so the screen's sentence adds up (`import_result_adds_up`).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ImportResult {
    pub run_id: String,
    pub source_id: String,
    pub state: ImportRunState,
    pub seen: u64,
    pub stored: u64,
    /// readable and already in the database (an earlier run, or a copy under another mailbox): changes nothing
    pub already_there: u64,
    pub skipped: Vec<ImportSkipped>,
    pub per_account: Vec<ImportPerAccount>,
    pub refusal: Option<ImportRefusal>,
}

// ---------------------------------------------------------------- refusal codes (the `field` of CoreError::Invalid / ImportRefusal.code)

/// macOS refused to list ~/Library/Mail.
pub const IMPORT_FULL_DISK_ACCESS_CODE: &str = "import_needs_full_disk_access";
/// The file or folder is not there any more.
pub const IMPORT_NOT_FOUND_CODE: &str = "import_source_not_found";
/// The source opened as a path but cannot be read (not an archive, say).
pub const IMPORT_UNREADABLE_CODE: &str = "import_source_unreadable";
/// The id names no source of the last scan.
pub const IMPORT_UNKNOWN_SOURCE_CODE: &str = "import_unknown_source";
/// The account of `ImportInto::Account` is not on this Mac.
pub const IMPORT_BAD_TARGET_CODE: &str = "import_bad_target";
/// One run at a time (decision of this delta: two writers over one database help nobody).
pub const IMPORT_ALREADY_RUNNING_CODE: &str = "import_already_running";
/// The run named by `ImportCancel` is not running.
pub const IMPORT_NOT_RUNNING_CODE: &str = "import_not_running";
/// A picked path is neither an `.olm` file nor an mbox the reader accepts.
pub const IMPORT_BAD_PICK_CODE: &str = "import_bad_pick";
/// The write after the source opened failed: the local database refused it.
pub const IMPORT_STORE_FAILED_CODE: &str = "import_store_failed";

fn refuse(code: &str, words: impl Into<String>) -> CoreError { CoreError::Invalid { field: code.into(), reason_words: words.into() } }

// ---------------------------------------------------------------- pure functions

/// "apple-mail" / "olm" / "mbox": the key in ids and in the JSON `kind`.
#[uniffi::export]
pub fn import_source_kind_key(kind: ImportSourceKind) -> String {
    match kind { ImportSourceKind::AppleMail => "apple-mail", ImportSourceKind::Olm => "olm", ImportSourceKind::Mbox => "mbox" }.to_string()
}

/// The stable id of a source: its kind key and its location, so a re-scan finds the same row and a re-run is recognised.
#[uniffi::export]
pub fn import_source_id(kind: ImportSourceKind, location: String) -> String { format!("{}:{}", import_source_kind_key(kind), location.trim()) }

/// The sentence for a source the core could not open; empty for a readable one. The Full Disk Access sentence is the one the first-start
/// step shows: it says how to grant it AND that skipping is fine (the step has a Skip). The import crate's `FULL_DISK_ACCESS_WORDS`
/// must equal it (a contract test pins this one; the core-import lane aligns its constant at the build word).
#[uniffi::export]
pub fn import_access_words(access: ImportAccess) -> String {
    match access {
        ImportAccess::Readable => "",
        ImportAccess::NeedsFullDiskAccess => "Whoosh needs Full Disk Access to read Apple Mail. Open System Settings, choose Privacy & Security, then Full Disk Access, add Whoosh to the list and switch it on. You can also skip this step and import later from Settings, Accounts.",
        ImportAccess::NotFound => "That file or folder is not there any more. Pick it again.",
        ImportAccess::Unreadable => "That file or folder could not be read. Check that it is a mail archive.",
    }.to_string()
}

/// The refusal code a not-readable access answers with; None for Readable.
#[uniffi::export]
pub fn import_access_code(access: ImportAccess) -> Option<String> {
    match access {
        ImportAccess::Readable => None,
        ImportAccess::NeedsFullDiskAccess => Some(IMPORT_FULL_DISK_ACCESS_CODE.to_string()),
        ImportAccess::NotFound => Some(IMPORT_NOT_FOUND_CODE.to_string()),
        ImportAccess::Unreadable => Some(IMPORT_UNREADABLE_CODE.to_string()),
    }
}

/// The refusal a run carries when its source would not open: the code and the words, from one place.
#[uniffi::export]
pub fn import_access_refusal(access: ImportAccess) -> Option<ImportRefusal> {
    import_access_code(access).map(|code| ImportRefusal { code, words: import_access_words(access) })
}

/// The refusal of a run that opened its source and then could not write: the local database refused the mail.
#[uniffi::export]
pub fn import_store_failed_refusal() -> ImportRefusal {
    ImportRefusal { code: IMPORT_STORE_FAILED_CODE.to_string(), words: "The local database refused the imported mail. What was stored stays; try again.".to_string() }
}

/// Whether a state is final: nothing more will be reported for the run.
#[uniffi::export]
pub fn import_run_is_over(state: ImportRunState) -> bool { state != ImportRunState::Running }

/// The state words of a run in the JSON `state`.
#[uniffi::export]
pub fn import_run_state_key(state: ImportRunState) -> String {
    match state {
        ImportRunState::Running => "running", ImportRunState::Finished => "finished", ImportRunState::Cancelled => "cancelled",
        ImportRunState::Refused => "refused", ImportRunState::Failed => "failed",
    }.to_string()
}

/// The access words in the JSON `access`.
#[uniffi::export]
pub fn import_access_key(access: ImportAccess) -> String {
    match access {
        ImportAccess::Readable => "readable", ImportAccess::NeedsFullDiskAccess => "needs_full_disk_access",
        ImportAccess::NotFound => "not_found", ImportAccess::Unreadable => "unreadable",
    }.to_string()
}

/// Where the bar stands, 0.0 to 1.0, derived once: None when the total is unknown or zero (an indeterminate bar, never a guessed
/// one). A total is an estimate, so `done` past it reads 1.0, never more.
#[uniffi::export]
pub fn import_progress_fraction(done: u64, total: Option<u64>) -> Option<f64> {
    match total { Some(t) if t > 0 => Some((done.min(t) as f64) / (t as f64)), _ => None }
}

/// Whether `ImportStart` may begin: the source must be one the last scan found and readable, the target an account on this Mac (or
/// `Imported`), and no other run may be going. The first failing rule answers, in this order.
#[uniffi::export]
pub fn check_import_start(source: Option<ImportSource>, into: ImportInto, account_ids: Vec<String>, running_run_id: Option<String>) -> Result<(), CoreError> {
    let Some(source) = source else {
        return Err(refuse(IMPORT_UNKNOWN_SOURCE_CODE, "That source is not in the list. Scan again."));
    };
    if let Some(r) = import_access_refusal(source.access) { return Err(refuse(&r.code, r.words)); }
    if let ImportInto::Account { account_id } = &into {
        if !account_ids.iter().any(|a| a == account_id) { return Err(refuse(IMPORT_BAD_TARGET_CODE, "That account is not on this Mac. Pick an account from the list, or Imported.")); }
    }
    if running_run_id.is_some() { return Err(refuse(IMPORT_ALREADY_RUNNING_CODE, "An import is already running. Wait for it to end or cancel it.")); }
    Ok(())
}

/// Whether `ImportCancel` has anything to cancel: the run must exist and be Running.
#[uniffi::export]
pub fn check_import_cancel(progress: Option<ImportRunProgress>) -> Result<(), CoreError> {
    match progress {
        Some(p) if p.state == ImportRunState::Running => Ok(()),
        _ => Err(refuse(IMPORT_NOT_RUNNING_CODE, "That import is not running.")),
    }
}

/// Whether a picked path may join the scan. The shell hands the core paths the owner picked in the file dialog; the extension (case
/// ignored) decides the kind: `.olm` = Outlook, `.mbox` and `.mbx` = mbox. Anything else is refused in words.
#[uniffi::export]
pub fn import_kind_of_pick(path: String) -> Result<ImportSourceKind, CoreError> {
    let lower = path.trim().to_lowercase();
    if lower.ends_with(".olm") { Ok(ImportSourceKind::Olm) }
    else if lower.ends_with(".mbox") || lower.ends_with(".mbx") { Ok(ImportSourceKind::Mbox) }
    else { Err(refuse(IMPORT_BAD_PICK_CODE, "Pick an Outlook archive (.olm) or a Thunderbird or Unix mailbox file (.mbox).")) }
}

/// Whether a result's three counts add up: seen = stored + already there + skipped. A run that was refused has seen nothing.
#[uniffi::export]
pub fn import_result_adds_up(result: ImportResult) -> bool {
    result.seen == result.stored + result.already_there + result.skipped.len() as u64
        && result.per_account.iter().map(|a| a.stored).sum::<u64>() == result.stored
}

/// Whether a progress record is well formed: a refusal exactly when the run was Refused or Failed, `stored + skipped <= done` (a
/// message is counted in at most one), a running run has no refusal. The engine builds progress through this check, the fixture
/// core asserts it in its own tests.
#[uniffi::export]
pub fn import_progress_is_legal(progress: ImportRunProgress) -> bool {
    let needs_refusal = matches!(progress.state, ImportRunState::Refused | ImportRunState::Failed);
    progress.refusal.is_some() == needs_refusal && progress.stored + progress.skipped <= progress.done
}

/// The stored-row rule of imported mail, repeated by the migration's two triggers: an imported row names its source and the time the run
/// stored it; a synced row names neither. `imported` is `message.source == 'imported'`.
#[uniffi::export]
pub fn imported_row_is_legal(imported: bool, import_source_id: Option<String>, imported_at_ms: Option<i64>) -> bool {
    if imported { import_source_id.is_some_and(|i| !i.trim().is_empty()) && imported_at_ms.is_some() } else { import_source_id.is_none() && imported_at_ms.is_none() }
}
