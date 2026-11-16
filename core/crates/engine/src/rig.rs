//! rig-core (docs/lanes/realcore-firstrun.testplan.md R2, R5 and the new mutant rows): what the HIDDEN TEST COPY may switch on.
//! Read only when `RustCore::open_production` runs with `rig` true, the same guard as every other `WHOOSH_CORE_CHECK_*` variable
//! (rust_core.rs `open_production`): the shell passes `rig` true only for the copy `scripts/launch-one.sh` makes (CorePlace.rig).
//! The owner's copy reads none of it: `from_env(false, ..)` answers nothing whatever the variables say.
//!
//! - `WHOOSH_CORE_CHECK_PROVIDERS=<port>`: the key providers' hosts are reached in clear HTTP on 127.0.0.1:<port>
//!   (`ai_settings::route_key_providers_to_loopback`), so the product's own path runs against checks/realcore_aifake.py.
//! - the keyring of a rig core keeps its items in `<data dir>/rig-keys.json` (`secrets::RigFileKeychain`, test data only).
//! - nothing leaves this Mac from a rig copy ("nothing leaves this Mac except the relay"): `apply` closes the process with crate netguard
//!   (`close_to_this_mac`, lane engine-act-r6), whose ONE guard and counter every dialer calls; `dial-out` dials 127.0.0.1 only.
//! - `WHOOSH_CORE_MUTANT=<name>`, the names below (the older names stay rust_core.rs `Mutant`'s): each one behaviour change, armed per
//!   store file, read by ONE guarded line where the behaviour lives.
use std::collections::HashMap;
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

pub const PROVIDERS_VARIABLE: &str = "WHOOSH_CORE_CHECK_PROVIDERS";
pub const MUTANT_VARIABLE: &str = "WHOOSH_CORE_MUTANT";
/// `dial-out`'s port on 127.0.0.1: one the first-run check listens on itself and keeps OUT of the walk's allowed ports.
pub const DIAL_PORT_VARIABLE: &str = "WHOOSH_CORE_MUTANT_DIAL_PORT";
/// The rig keyring's file, inside the core's data folder (the run folder's `core/`, deleted with the run).
pub const KEYS_FILE: &str = "rig-keys.json";
/// How long one dial-out attempt waits before the next; the check samples every 500 ms, so an attempt is always open.
const DIAL_OUT_WAIT: Duration = Duration::from_secs(5);
/// `slow-save`'s pause in SaveAgentConfig: above the owner's 50 ms bound with room for the clock.
pub const SLOW_SAVE: Duration = Duration::from_millis(80);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RigMutant {
    /// AddAccount answers Ok and stores nothing (no account row, no Keychain item).
    NoAccountStored,
    /// The body download fetches and keeps nothing.
    BodiesNeverStored,
    /// The word indexer skips its write.
    IndexNeverWrites,
    /// SaveAgentConfig keeps the agent in memory only: it is gone after a relaunch.
    ChoicesInMemory,
    /// The core opens a TCP connection to 127.0.0.1:<WHOOSH_CORE_MUTANT_DIAL_PORT>, a port outside the walk's fakes.
    DialOut,
    /// SaveAgentConfig waits SLOW_SAVE first.
    SlowSave,
}

impl RigMutant {
    pub const ALL: [RigMutant; 6] = [RigMutant::NoAccountStored, RigMutant::BodiesNeverStored, RigMutant::IndexNeverWrites, RigMutant::ChoicesInMemory, RigMutant::DialOut, RigMutant::SlowSave];

    pub fn name(self) -> &'static str {
        match self {
            RigMutant::NoAccountStored => "no-account-stored",
            RigMutant::BodiesNeverStored => "bodies-never-stored",
            RigMutant::IndexNeverWrites => "index-never-writes",
            RigMutant::ChoicesInMemory => "choices-in-memory",
            RigMutant::DialOut => "dial-out",
            RigMutant::SlowSave => "slow-save",
        }
    }

    pub fn from_name(name: &str) -> Option<RigMutant> { RigMutant::ALL.into_iter().find(|m| m.name() == name.trim()) }
}

/// What a rig launch asked for. All empty on the owner's copy.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RigSwitches {
    /// True on the hidden test copy only.
    pub rig: bool,
    pub providers_port: Option<u16>,
    pub dial_port: Option<u16>,
    pub mutant: Option<RigMutant>,
    pub keys_file: Option<PathBuf>,
}

/// The switches of a launch: `var` reads one environment variable. `rig` false = nothing at all.
pub fn from_env(rig: bool, data_dir: &Path, var: &dyn Fn(&str) -> Option<String>) -> RigSwitches {
    if !rig { return RigSwitches::default(); }
    let port = |name: &str| var(name).and_then(|v| v.trim().parse::<u16>().ok()).filter(|p| *p != 0);
    RigSwitches {
        rig: true,
        providers_port: port(PROVIDERS_VARIABLE),
        dial_port: port(DIAL_PORT_VARIABLE),
        mutant: var(MUTANT_VARIABLE).as_deref().and_then(RigMutant::from_name),
        keys_file: Some(data_dir.join(KEYS_FILE)),
    }
}

/// Arms the launch's switches on the store at `store_path` (the providers' loopback, the mutant, a dial-out). Called once, at open.
pub fn apply(switches: &RigSwitches, store_path: &Path) {
    if switches.rig { netguard::close_to_this_mac(); }   // the rig closes the process through netguard only (engine-act-r6's guard, the lead's ruling 2026-11-10)
    if let Some(port) = switches.providers_port { crate::ai_settings::route_key_providers_to_loopback(store_path, port); }
    arm(store_path, switches.mutant);
    if switches.mutant == Some(RigMutant::DialOut) {
        match switches.dial_port {
            Some(port) => { if let Err(e) = dial_out_to(SocketAddr::from(([127, 0, 0, 1], port))) { eprintln!("rig dial-out: {e}"); } }
            None => eprintln!("rig dial-out: {DIAL_PORT_VARIABLE} is not set, so nothing is dialled"),
        }
    }
}


/// True once a rig core opened in this process (`apply` with `rig`): the app process is the hidden test copy, and nothing it opens may
/// reach past this Mac. Never true on the owner's copy.
pub fn is_rig_process() -> bool { netguard::is_closed() }

static ANY_ARMED: AtomicBool = AtomicBool::new(false);

fn armed() -> &'static Mutex<HashMap<PathBuf, RigMutant>> {
    static ARMED: OnceLock<Mutex<HashMap<PathBuf, RigMutant>>> = OnceLock::new();
    ARMED.get_or_init(Default::default)
}

/// Sets (Some) or clears (None) the mutant of the core over the store at `store_path`. Tests call it directly; a rig launch through `apply`.
pub fn arm(store_path: &Path, mutant: Option<RigMutant>) {
    let mut g = armed().lock().unwrap_or_else(|e| e.into_inner());
    match mutant { Some(m) => { g.insert(store_path.to_path_buf(), m); } None => { g.remove(store_path); } }
    ANY_ARMED.store(!g.is_empty(), Ordering::SeqCst);
}

/// True when `mutant` is armed on the store at `store_path`. One atomic read on the owner's copy, which never arms one.
pub fn on(store_path: &Path, mutant: RigMutant) -> bool {
    ANY_ARMED.load(Ordering::Relaxed) && armed().lock().unwrap_or_else(|e| e.into_inner()).get(store_path) == Some(&mutant)
}

static DIAL_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);

/// `dial-out`: a thread that opens a TCP connection to `peer`, again after each one ends, for the life of the process. A peer that is not
/// this Mac is refused before any socket is opened.
pub fn dial_out_to(peer: SocketAddr) -> Result<(), String> {
    if !peer.ip().is_loopback() { return Err(format!("{peer} is not this Mac: a test copy dials 127.0.0.1 only, so nothing was opened")); }
    let started = std::thread::Builder::new().name("whoosh-rig-dial-out".into()).spawn(move || loop {
        DIAL_ATTEMPTS.fetch_add(1, Ordering::SeqCst);
        match TcpStream::connect_timeout(&peer, DIAL_OUT_WAIT) {
            Ok(held) => { std::thread::sleep(DIAL_OUT_WAIT); drop(held); }
            Err(_) => std::thread::sleep(Duration::from_millis(200)),
        }
    });
    started.map(|_| ()).map_err(|e| format!("the thread did not start: {e}"))
}

/// How many dial-out connections this process opened or tried.
pub fn dial_attempts() -> usize { DIAL_ATTEMPTS.load(Ordering::SeqCst) }
