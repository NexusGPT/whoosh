//! A time limit of our own on every mailbox command (provider-limits lane, item 2). Before it, a server that took a command and
//! never answered held the call, its connection permit and the sync worker for ever: the only ceiling was the IDLE wait's
//! (idle.rs LIBRARY_CEILING, 24 h), and async-imap 0.12.0 has no timeout outside its IDLE handle (extensions/idle.rs:127-157).
//! The limit is IMAP_COMMAND_TIMEOUT_MS (docs/CONSTANTS.md). It runs on SILENCE: a read of the server's reply is cut only when no
//! byte has arrived for that long (`Activity`, stamped by `Conn::poll_read`), so a long FETCH that is still receiving bytes is never
//! cut, however long the whole takes (async-imap's `read_response` returns a whole response, literal included, so a limit on the
//! call itself would cut a slow big body). The sending of a command, the whole open (connect, TLS, greeting, sign-in, capabilities),
//! and IDLE's start and end are short exchanges and run under the plain limit (`within`). IDLE's own wait is not touched.
//! The answer is a retryable refusal in words, the same kind a lost connection gives, so the sync engine's backoff handles it.
use std::future::Future;
use std::time::Duration;

use crate::adapter::E;
use crate::consts::IMAP_COMMAND_TIMEOUT_MS;

/// Test-only override of the limit, in milliseconds (0: the ledger's). Set by this crate's own tests, which cannot wait 30 seconds.
#[cfg(test)]
pub(crate) static TEST_LIMIT_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(crate) fn limit() -> Duration {
    #[cfg(test)]
    {
        let t = TEST_LIMIT_MS.load(std::sync::atomic::Ordering::SeqCst);
        if t > 0 { return Duration::from_millis(t); }
    }
    Duration::from_millis(IMAP_COMMAND_TIMEOUT_MS)
}

/// The words of a refusal for a wait that ran out: what was waited for, and how long.
pub(crate) fn timed_out_words(what: &str) -> String {
    let l = limit();
    let span = if l.as_millis() >= 1000 && l.as_millis() % 1000 == 0 { format!("{} seconds", l.as_secs()) } else { format!("{} ms", l.as_millis()) };
    format!("the server did not answer {what} within {span}")
}

/// `timed_out_words` as the error a command returns: retryable, like a lost connection.
pub(crate) fn timed_out(what: &str) -> E {
    E::Words { words: timed_out_words(what), retryable: true }
}

/// When the connection last received a byte, in milliseconds from its creation.
pub(crate) struct Activity { base: tokio::time::Instant, last_ms: std::sync::atomic::AtomicU64 }

impl Activity {
    pub(crate) fn new() -> Self { Activity { base: tokio::time::Instant::now(), last_ms: std::sync::atomic::AtomicU64::new(0) } }
    pub(crate) fn touch(&self) {
        let ms = u64::try_from(self.base.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.last_ms.fetch_max(ms, std::sync::atomic::Ordering::SeqCst);
    }
    fn deadline(&self) -> tokio::time::Instant { self.base + Duration::from_millis(self.last_ms.load(std::sync::atomic::Ordering::SeqCst)) + limit() }
}

/// Awaits `f` until it finishes or the connection has been silent for the limit. The wait starts now (a quiet period before the call
/// does not count).
pub(crate) async fn on_silence<F: Future>(act: &Activity, f: F) -> Result<F::Output, ()> {
    act.touch();
    tokio::pin!(f);
    loop {
        let deadline = act.deadline();
        tokio::select! {
            r = &mut f => return Ok(r),
            _ = tokio::time::sleep_until(deadline) => { if act.deadline() <= tokio::time::Instant::now() { return Err(()); } }
        }
    }
}

/// Runs `f` under the limit; a wait that runs out is a retryable refusal in words. The future is dropped then: the caller's
/// session is in an unknown state and is dropped with it (every caller returns the error, so it never goes back to the pool).
pub(crate) async fn within<T, Er: Into<E>>(what: &str, f: impl Future<Output = Result<T, Er>>) -> Result<T, E> {
    match tokio::time::timeout(limit(), f).await {
        Ok(r) => r.map_err(Into::into),
        Err(_) => Err(timed_out(what)),
    }
}

#[cfg(test)]
mod tests {
    //! Item 2's tests: a fake server on 127.0.0.1 (plain, this Mac only) that takes a command and goes quiet. Each call runs under an
    //! outer 8-second guard, so a missing limit is a red test (the guard fires), never a hung test run. The limit is 300 ms here
    //! (TEST_LIMIT_MS); in the product it is IMAP_COMMAND_TIMEOUT_MS. Mutants are named in the comments; the lane note lists the run.
    use super::*;
    use crate::adapter::{Auth, ImapAdapter, ImapConfig, Security};
    use crate::connections::ImapLimits;
    use crate::idle::{IdleEvent, TokioSleeper};
    use crate::port::ImapMailAdapter;
    use contracts::{ActionFailed, CoreError};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;
    use tokio::sync::Notify;

    const TEST_MS: u64 = 300;
    const GUARD: Duration = Duration::from_secs(8);

    /// What the fake does: the first `times` commands whose verb is `verb` are taken and never answered (`partial`: LIST first sends
    /// two untagged lines). `greet: false` accepts the connection and says nothing. `drip_ms`: LIST sends six lines this far apart.
    #[derive(Clone)]
    struct Fake { verb: &'static str, times: usize, partial: bool, greet: bool, drip_ms: u64, slow_body: bool, no_read: bool, slow_read: bool, idles: Arc<AtomicUsize> }
    impl Fake {
        fn normal() -> Self { Fake { verb: "", times: 0, partial: false, greet: true, drip_ms: 0, slow_body: false, no_read: false, slow_read: false, idles: Arc::new(AtomicUsize::new(0)) } }
        fn hang(verb: &'static str) -> Self { Fake { verb, times: usize::MAX, ..Self::normal() } }
    }

    async fn serve(fake: Fake) -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = l.local_addr().expect("addr").port();
        let taken = Arc::new(AtomicUsize::new(0));
        tokio::spawn(async move {
            loop {
                let Ok((sock, _)) = l.accept().await else { return };
                let (fake, taken) = (fake.clone(), taken.clone());
                tokio::spawn(async move {
                    let (r, mut w) = sock.into_split();
                    let mut r = BufReader::new(r);
                    if !fake.greet { std::future::pending::<()>().await; }
                    let _ = w.write_all(b"* OK ready\r\n").await;
                    let mut line = String::new();
                    let mut idle_tag = String::new();
                    loop {
                        line.clear();
                        if r.read_line(&mut line).await.unwrap_or(0) == 0 { return; }
                        let mut parts = line.trim_end().splitn(3, ' ');
                        let (tag, verb) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_ascii_uppercase());
                        let rest = parts.next().unwrap_or("").to_string();
                        let verb = if tag.eq_ignore_ascii_case("DONE") { "DONE".to_string() } else { verb }; // IDLE's untagged end line
                        if verb == fake.verb && taken.fetch_add(1, Ordering::SeqCst) < fake.times {
                            if verb == "LIST" && fake.partial { let _ = w.write_all(b"* LIST (\\HasNoChildren) \"/\" \"INBOX\"\r\n* LIST (\\HasNoChildren) \"/\" \"Sent\"\r\n").await; }
                            if verb == "APPEND" && fake.no_read {
                                let _ = w.write_all(b"+ go\r\n").await; // then reads nothing more: the client's literal fills the socket buffers
                            } else if verb == "APPEND" {
                                let _ = w.write_all(b"+ go\r\n").await;
                                let n: usize = rest.rsplit('{').next().and_then(|s| s.trim_end_matches('}').parse().ok()).unwrap_or(0);
                                let mut lit = vec![0u8; n + 2];
                                let _ = r.read_exact(&mut lit).await;
                            }
                            std::future::pending::<()>().await; // taken, never answered, the socket stays open
                        }
                        let out = match verb.as_str() {
                            "CAPABILITY" => format!("* CAPABILITY IMAP4rev1 IDLE UIDPLUS\r\n{tag} OK done\r\n"),
                            "LOGIN" => format!("{tag} OK logged in\r\n"),
                            "SELECT" => format!("* 0 EXISTS\r\n* OK [UIDVALIDITY 7] ok\r\n{tag} OK [READ-WRITE] done\r\n"),
                            "IDLE" => { idle_tag = tag.clone(); fake.idles.fetch_add(1, Ordering::SeqCst); "+ idling\r\n".to_string() }
                            "DONE" => format!("{idle_tag} OK idle done\r\n"),
                            "APPEND" => {
                                let _ = w.write_all(b"+ go\r\n").await;
                                let n: usize = rest.rsplit('{').next().and_then(|s| s.trim_end_matches('}').parse().ok()).unwrap_or(0);
                                if fake.slow_read {
                                    // takes the literal 16 MiB per 100 ms (a read empties the socket buffers, so the wait for the reply after the last byte is short): the client's write waits on the socket buffers, but the bytes keep moving
                                    let mut left = n + 2;
                                    let mut chunk = vec![0u8; 16 * 1024 * 1024];
                                    while left > 0 {
                                        let got = r.read(&mut chunk[..left.min(16 * 1024 * 1024)]).await.unwrap_or(0);
                                        if got == 0 { break; }
                                        left -= got;
                                        if left > 0 { tokio::time::sleep(Duration::from_millis(100)).await; } // none after the last byte: the reply follows at once
                                    }
                                }
                                let mut lit = vec![0u8; if fake.slow_read { 0 } else { n + 2 }];
                                let _ = r.read_exact(&mut lit).await;
                                format!("{tag} OK appended\r\n")
                            }
                            "UID" if fake.slow_body => {
                                // one FETCH answer whose 2000-byte literal arrives 100 bytes every 100 ms: 2 s in all, every gap under the limit
                                let _ = w.write_all(b"* 1 FETCH (UID 5 FLAGS () INTERNALDATE \"01-Jan-2024 00:00:00 +0000\" RFC822.SIZE 2000 BODY[] {2000}\r\n").await;
                                for _ in 0..20 {
                                    let _ = w.write_all(&[b'x'; 100]).await;
                                    let _ = w.flush().await;
                                    tokio::time::sleep(Duration::from_millis(100)).await;
                                }
                                format!(")\r\n{tag} OK done\r\n")
                            }
                            "LIST" if fake.drip_ms > 0 => {
                                for i in 0..6 {
                                    let _ = w.write_all(format!("* LIST (\\HasNoChildren) \"/\" \"F{i}\"\r\n").as_bytes()).await;
                                    let _ = w.flush().await;
                                    tokio::time::sleep(Duration::from_millis(fake.drip_ms)).await;
                                }
                                format!("{tag} OK done\r\n")
                            }
                            "LIST" => format!("* LIST (\\HasNoChildren) \"/\" \"INBOX\"\r\n{tag} OK done\r\n"),
                            _ => format!("{tag} OK done\r\n"),
                        };
                        let _ = w.write_all(out.as_bytes()).await;
                    }
                });
            }
        });
        port
    }

    fn adapter(port: u16) -> ImapAdapter {
        TEST_LIMIT_MS.store(TEST_MS, Ordering::SeqCst);
        ImapAdapter::new(ImapConfig {
            account_id: "t".into(), account_name: "t".into(), host: "127.0.0.1".into(), port, security: Security::PlainLoopbackOnly,
            username: "owner@example.test".into(), auth: Auth::Password("app-password".into()), extra_roots_der: Vec::new(),
            // one connection: a permit that a timed-out call kept would hang the next call
            limits: ImapLimits { max_default: 1, max_gmail: 1, idle_per_account: 1, backoff_start_ms: 10, backoff_max_ms: 100, transcript_lines: 50 },
        })
    }

    async fn failure<T: std::fmt::Debug>(f: impl Future<Output = Result<T, CoreError>>) -> (ActionFailed, Duration) {
        let t = std::time::Instant::now();
        let r = tokio::time::timeout(GUARD, f).await.expect("the call hung past the 8 s guard: no limit of our own stopped it");
        match r { Err(CoreError::Action { failure }) => (failure, t.elapsed()), other => panic!("expected an Action failure, got {other:?}") }
    }

    fn assert_timed_out(f: &ActionFailed, took: Duration, what: &str) {
        assert!(f.retryable, "a wait that ran out is retried: {f:?}");
        assert!(f.reason_words.contains("did not answer") && f.reason_words.contains(what) && f.reason_words.contains("300 ms"), "{}", f.reason_words);
        assert!(took >= Duration::from_millis(TEST_MS) && took < Duration::from_secs(5), "ended after {took:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_server_that_accepts_the_connection_and_never_greets_ends_the_open_in_a_retryable_refusal() {
        // mutant open-unbounded: adapter.rs `tokio::time::timeout(command_limit::limit(), open_session_unbounded(d, tap)).await` -> `Ok::<_, ()>(open_session_unbounded(d, tap).await)`
        let a = adapter(serve(Fake { greet: false, ..Fake::normal() }).await);
        let (f, took) = failure(a.list_folders()).await;
        assert_timed_out(&f, took, "while it was being opened");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_server_that_goes_quiet_at_the_sign_in_ends_the_open_the_same_way() {
        let a = adapter(serve(Fake::hang("LOGIN")).await);
        let (f, took) = failure(a.list_folders()).await;
        assert_timed_out(&f, took, "while it was being opened");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_list_the_server_never_answers_a_select_it_never_answers_and_a_stall_mid_reply_each_end_retryable() {
        // mutants read-unbounded (checked.rs `command_limit::on_silence(&act, s.s.read_response()).await` -> `Ok::<_, ()>(s.s.read_response().await)`),
        //         not-retryable (command_limit.rs `retryable: true` -> `retryable: false`: assert_timed_out's retryable check)
        let a = adapter(serve(Fake::hang("LIST")).await);
        let (f, took) = failure(a.list_folders()).await;
        assert_timed_out(&f, took, "LIST");
        let a = adapter(serve(Fake { partial: true, ..Fake::hang("LIST") }).await);
        let (f, took) = failure(a.list_folders()).await;
        assert_timed_out(&f, took, "LIST");
        let a = adapter(serve(Fake::hang("SELECT")).await);
        let (f, took) = failure(a.folder_state("INBOX")).await;
        assert_timed_out(&f, took, "SELECT");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_timed_out_call_gives_its_connection_back_and_the_next_call_is_served() {
        // one connection in the table: a permit kept by the timed-out call would hang the second call (the guard fires)
        let a = adapter(serve(Fake { times: 1, ..Fake::hang("LIST") }).await);
        let (f, _) = failure(a.list_folders()).await;
        assert!(f.retryable);
        let folders = tokio::time::timeout(GUARD, a.list_folders()).await.expect("the second call hung").expect("the second call is served");
        assert_eq!(folders.len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_append_the_server_takes_and_never_confirms_ends_retryable() {
        // mutant append-read-unbounded: adapter.rs `command_limit::on_silence(&act, s.s.read_response()).await.map_err(|_| command_limit::timed_out("APPEND"))??` -> `s.s.read_response().await?`
        let a = adapter(serve(Fake::hang("APPEND")).await);
        let (f, took) = failure(a.append("INBOX", b"Subject: x\r\n\r\nbody\r\n", &[], None)).await;
        assert_timed_out(&f, took, "APPEND");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_idle_the_server_never_accepts_ends_retryable() {
        // mutant idle-init-unbounded: idle.rs `crate::command_limit::within("IDLE", h.init()).await.map_err(|e| self.core_err(e))?` -> `h.init().await.map_err(|e| self.err_imap(e))?`
        let a = adapter(serve(Fake::hang("IDLE")).await);
        let stop = Notify::new();
        let mut events: Vec<IdleEvent> = Vec::new();
        let (f, took) = failure(async { a.idle("INBOX", &TokioSleeper, &stop, &mut |e| events.push(e)).await.map(|_| ()) }).await;
        assert_timed_out(&f, took, "IDLE");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_idle_the_server_never_lets_go_of_when_it_is_ended_ends_retryable() {
        // mutant idle-done-unbounded: idle.rs `crate::command_limit::within("the end of IDLE", h.done()).await.map_err(|e| self.core_err(e))?` -> `h.done().await.map_err(|e| self.err_imap(e))?`
        let a = adapter(serve(Fake::hang("DONE")).await);
        let stop = Notify::new();
        stop.notify_one(); // IDLE is taken, then ended at once: the server never answers the DONE
        let mut events: Vec<IdleEvent> = Vec::new();
        let (f, took) = failure(async { a.idle("INBOX", &TokioSleeper, &stop, &mut |e| events.push(e)).await.map(|_| ()) }).await;
        assert_timed_out(&f, took, "the end of IDLE");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_server_that_stops_reading_a_message_being_appended_ends_the_send_retryable() {
        // the sending side: the server takes `APPEND`, answers `+ go` and reads no more; 48 MiB fill the socket buffers and the write blocks.
        // mutant append-write-unbounded: adapter.rs `command_limit::on_silence(&act, async {` ... `.await.map_err(|_| command_limit::timed_out("APPEND (taking the message)"))??` -> the write awaited plainly (the guard fires)
        use crate::port::ImapMailAdapter as _;
        let a = adapter(serve(Fake { no_read: true, ..Fake::hang("APPEND") }).await);
        let big = vec![b'x'; 48 * 1024 * 1024];
        let (f, took) = failure(a.append("INBOX", &big, &[], None)).await;
        assert_timed_out(&f, took, "APPEND (taking the message)");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_message_sent_slowly_but_steadily_is_not_cut_off() {
        // 160 MiB taken 16 MiB per 100 ms (about a second in all, limit 300 ms): the write keeps moving, so it is not silence.
        // mutant write-progress-not-stamped: transport.rs `if matches!(r, Poll::Ready(Ok(n)) if n > 0) { self.1.touch(); }` removed (red here)
        use crate::port::ImapMailAdapter as _;
        let a = adapter(serve(Fake { slow_read: true, ..Fake::normal() }).await);
        let big = vec![b'x'; 160 * 1024 * 1024];
        tokio::time::timeout(Duration::from_secs(30), a.append("INBOX", &big, &[], None)).await.expect("hung").expect("a slow but steady send is not cut");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_quiet_idle_is_not_cut_by_the_limit_it_runs_for_three_limits_on_one_idle_command() {
        // IDLE is quiet by design (its own 29-minute re-issue stands). A quiet server, 900 ms = three limits: one IDLE command, no re-issue, Ok at the stop.
        // mutant idle-cut-by-silence: idle.rs `_ = &mut reissue => break Out::Reissue,` + `_ = tokio::time::sleep(crate::command_limit::limit()) => break Out::Reissue,` (the IDLE is re-issued every 300 ms: idles > 1)
        let fake = Fake::normal();
        let idles = fake.idles.clone();
        let a = adapter(serve(fake).await);
        let stop = Arc::new(Notify::new());
        let s2 = stop.clone();
        tokio::spawn(async move { tokio::time::sleep(Duration::from_millis(3 * TEST_MS)).await; s2.notify_one(); });
        let t = std::time::Instant::now();
        let mut events: Vec<IdleEvent> = Vec::new();
        tokio::time::timeout(GUARD, a.idle("INBOX", &TokioSleeper, &stop, &mut |e| events.push(e))).await.expect("hung").expect("a quiet IDLE ends Ok at the stop");
        assert!(t.elapsed() >= Duration::from_millis(3 * TEST_MS), "ended after {:?}", t.elapsed());
        assert_eq!(idles.load(Ordering::SeqCst), 1, "the IDLE command was sent once");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_reply_that_keeps_coming_is_not_cut_off_though_the_whole_takes_longer_than_the_limit() {
        // six lines 150 ms apart: 900 ms in all, each gap under the 300 ms limit. A limit on the whole command (not on each wait) is red here.
        let a = adapter(serve(Fake { drip_ms: 150, ..Fake::normal() }).await);
        let folders = tokio::time::timeout(GUARD, a.list_folders()).await.expect("hung").expect("a slow but steady reply is read to its end");
        assert_eq!(folders.len(), 6);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_large_body_that_arrives_slowly_but_steadily_is_not_cut_off() {
        // the literal is one response that takes 2 s to arrive, 100 ms between pieces, limit 300 ms: only silence counts.
        // mutant limit-per-response: checked.rs `command_limit::on_silence(&act, s.s.read_response()).await` -> `tokio::time::timeout(command_limit::limit(), s.s.read_response()).await` (red here)
        use crate::port::ImapMailAdapter as _;
        let a = adapter(serve(Fake { slow_body: true, ..Fake::normal() }).await);
        let (body, _, _) = tokio::time::timeout(GUARD, a.fetch_raw("INBOX", 5)).await.expect("hung").expect("a slow but steady body is read to its end");
        assert_eq!(body.len(), 2000);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_fetch_that_starts_a_body_and_then_goes_silent_is_cut_after_the_silence() {
        // the server sends the FETCH head and half the literal, then nothing: the silence limit ends it
        let a = adapter(serve(Fake { partial: true, ..Fake::hang("UID") }).await);
        let (f, took) = failure(a.fetch_raw("INBOX", 5)).await;
        assert_timed_out(&f, took, "UID FETCH");
    }

    #[test]
    fn the_words_name_the_wait_and_the_limit() {
        TEST_LIMIT_MS.store(TEST_MS, Ordering::SeqCst);
        assert_eq!(timed_out_words("LIST"), "the server did not answer LIST within 300 ms");
    }
}
