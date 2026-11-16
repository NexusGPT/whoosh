//! fix-realcore (main92 s2-007.log, s1-014.log `--slice-crawl`: "FAIL one-connection at most 3 IMAP sessions were open at once (limit 2: the
//! sync's IDLE session and one for the bodies)"): the engine handed the adapter the PROVIDER's cap (IMAP_MAX_CONNECTIONS_DEFAULT 5, Gmail 15)
//! as its pool, so the sync's workers, the background body download and the IDLE session (idle.rs leases a permit of the same pool) opened
//! three sessions and more. docs/CONSTANTS.md: "Whoosh itself opens IMAP_CONN_PER_ACCOUNT, which this cap bounds". The account's pool is
//! now IMAP_CONN_PER_ACCOUNT (2) under the provider's cap, for every host: the IDLE session and one more.
//! Broken copies (one edit, red first):
//!   sync.rs `limits`: `max_default: IMAP_MAX_CONNECTIONS_DEFAULT.min(WHOOSH_CONNECTIONS),` -> `max_default: IMAP_MAX_CONNECTIONS_DEFAULT,` (the old code: red)
//!   sync.rs `limits`: `max_gmail: IMAP_MAX_CONNECTIONS_GMAIL.min(WHOOSH_CONNECTIONS),` -> `max_gmail: IMAP_MAX_CONNECTIONS_GMAIL,` (Gmail: red)
use contracts::constants::{IMAP_CONN_PER_ACCOUNT, IMAP_IDLE_CONNECTIONS_PER_ACCOUNT, IMAP_MAX_CONNECTIONS_DEFAULT, IMAP_MAX_CONNECTIONS_GMAIL};

#[test]
fn an_account_holds_at_most_whoosh_own_number_of_imap_sessions_the_idle_one_included() {
    let limits = engine::sync::limits();
    for host in ["127.0.0.1", "imap.example.test", "imap.gmail.com"] {
        let table = limits.table_for(host);
        let provider_cap = if host == "imap.gmail.com" { IMAP_MAX_CONNECTIONS_GMAIL } else { IMAP_MAX_CONNECTIONS_DEFAULT };
        let want = (IMAP_CONN_PER_ACCOUNT.min(provider_cap)) as usize;
        assert_eq!(table.max_connections, want, "{host}: the pool (the sessions the account may hold open, IDLE included) is Whoosh's own number under the provider's cap");
        assert_eq!(table.idle_connections, IMAP_IDLE_CONNECTIONS_PER_ACCOUNT as usize, "{host}: one IDLE session");
        assert!(table.max_connections > table.idle_connections, "{host}: one session is left beside the IDLE one for the sync, the bodies and the acts");
        println!("PASS {host}: at most {} sessions, {} of them IDLE", table.max_connections, table.idle_connections);
    }
}
