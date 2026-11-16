//! IDLE on one folder, ended and re-issued every `IMAP_IDLE_REISSUE_MIN` minutes (RFC 2177: a
//! server may drop a client that idles longer than 29 minutes). Time comes from a `Sleeper` so a
//! test moves it without waiting.
use std::time::Duration;

use async_imap::extensions::idle::IdleResponse;
use contracts::CoreError;
use futures::future::BoxFuture;
use imap_proto::{Response, Status};
use tokio::sync::Notify;

use crate::{adapter::ImapAdapter, consts::IMAP_IDLE_REISSUE_MIN};

pub trait Sleeper: Send + Sync {
    fn sleep(&self, d: Duration) -> BoxFuture<'static, ()>;
}

pub struct TokioSleeper;
impl Sleeper for TokioSleeper {
    fn sleep(&self, d: Duration) -> BoxFuture<'static, ()> { Box::pin(tokio::time::sleep(d)) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdleEvent {
    /// The server reported a change in the folder: run `resync`.
    Changed,
    /// IDLE runs on a connection (the first one, or a new one after a lost connection).
    Listening,
    /// The IDLE connection was lost (the server's BYE, a dropped socket); a new one is opened after `retry_in_ms`.
    /// `reason_words` are the server's words, or what the socket said.
    Lost { reason_words: String, retry_in_ms: u64 },
}

/// The per-IDLE ceiling given to the client library; our own re-issue timer fires long before.
const LIBRARY_CEILING: Duration = Duration::from_secs(60 * 60 * 24);

enum Out {
    Server(Result<IdleResponse, async_imap::error::Error>),
    Reissue,
    Stop,
}

impl ImapAdapter {
    /// IDLE that survives a dropped connection: when the connection is lost, wait the table's
    /// backoff (start value doubled each time, up to the maximum) and open a new one. The new
    /// connection takes the same permit, so the account never exceeds its connection cap. A server
    /// answer such as a refused login ends the loop with that error. Returns when stopped.
    /// The backoff starts again at the first step after a connection that worked: one that reported
    /// a change, or one that held IDLE longer than the longest wait (`backoff_max_ms`, SYNC_BACKOFF_MAX_MS).
    pub async fn idle_supervised(
        &self,
        folder: &str,
        sleeper: &dyn Sleeper,
        stop: &Notify,
        on_event: &mut (dyn FnMut(IdleEvent) + Send),
    ) -> Result<(), CoreError> {
        let mut attempt: u32 = 0;
        loop {
            let mut progressed = false;
            let mut held = false;
            // A change, or a hold past the longest wait, resets the backoff: a server that takes IDLE and drops it at once must not be
            // redialled at the start rate, and a quiet mailbox whose server ends IDLE now and then must not climb to the cap.
            let r = self.idle_on(folder, sleeper, stop, true, Some(&mut held), &mut |e| { if e == IdleEvent::Changed { progressed = true; } on_event(e) }).await;
            match r {
                Ok(()) => return Ok(()),
                Err(CoreError::Action { failure }) if failure.retryable => {
                    if progressed || held { attempt = 0; }
                    let wait = self.connection_table().reconnect_delay(attempt);
                    attempt = attempt.saturating_add(1);
                    on_event(IdleEvent::Lost { reason_words: failure.reason_words, retry_in_ms: u64::try_from(wait.as_millis()).unwrap_or(u64::MAX) });
                    tokio::select! {
                        _ = sleeper.sleep(wait) => {}
                        _ = stop.notified() => return Ok(()),
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Holds one of the account's connections until `stop` is notified. Returns when stopped. Reports `Changed` only.
    pub async fn idle(
        &self,
        folder: &str,
        sleeper: &dyn Sleeper,
        stop: &Notify,
        on_event: &mut (dyn FnMut(IdleEvent) + Send),
    ) -> Result<(), CoreError> {
        self.idle_on(folder, sleeper, stop, false, None, on_event).await
    }

    /// `idle`, and with `announce` a `Listening` once the server took the first IDLE on this connection. With `held`, it is
    /// set once this connection has held IDLE for the table's longest wait (`backoff_max_ms`): the supervisor's healthy mark.
    async fn idle_on(
        &self,
        folder: &str,
        sleeper: &dyn Sleeper,
        stop: &Notify,
        announce: bool,
        mut held: Option<&mut bool>,
        on_event: &mut (dyn FnMut(IdleEvent) + Send),
    ) -> Result<(), CoreError> {
        // At most the table's number of IDLE connections per account (one): a second IDLE would
        // take a connection the working calls need.
        let _slot = self.idle_slots().try_acquire_owned().map_err(|_| self.err_words("an IDLE connection is already open for this account"))?;
        let mut lease = self.lease().await?;
        let mut sess = lease.sess.take().expect("lease holds a session");
        if !ImapAdapter::caps_idle(&sess) {
            return Err(self.err_words("the server does not offer IDLE"));
        }
        self.select_for_idle(&mut sess, folder).await?;
        let caps = sess.caps;
        let mut s = sess.s;
        let every = Duration::from_secs(IMAP_IDLE_REISSUE_MIN * 60);
        let mut announced = !announce;
        // armed only for the supervisor (the unsupervised `idle` keeps one timer, the re-issue); it runs across re-issues
        let healthy = held.as_ref().map(|_| sleeper.sleep(Duration::from_millis(self.connection_table().backoff_max_ms)));
        let mut healthy = std::pin::pin!(async move { match healthy { Some(t) => t.await, None => std::future::pending().await } });
        let mut is_held = false;
        loop {
            let mut h = s.idle();
            crate::command_limit::within("IDLE", h.init()).await.map_err(|e| self.core_err(e))?;
            if !announced { announced = true; on_event(IdleEvent::Listening); }
            let out = {
                let (fut, stop_src) = h.wait_with_timeout(LIBRARY_CEILING);
                tokio::pin!(fut);
                let reissue = sleeper.sleep(every);
                tokio::pin!(reissue);
                let stopped = stop.notified();
                tokio::pin!(stopped);
                let first = loop {
                    tokio::select! {
                        r = &mut fut => break Out::Server(r),
                        _ = &mut reissue => break Out::Reissue,
                        _ = &mut stopped => break Out::Stop,
                        // the connection held IDLE past the longest wait: marked, and the same IDLE goes on
                        _ = &mut healthy, if !is_held => {
                            is_held = true;
                            if let Some(h) = held.as_deref_mut() { *h = true; }
                        }
                    }
                };
                match first {
                    Out::Server(r) => Out::Server(r),
                    other => {
                        drop(stop_src); // ends the wait: the library then returns ManualInterrupt
                        let _ = (&mut fut).await;
                        other
                    }
                }
            };
            // A BYE while idling: the server is closing this connection. Its words go to the supervisor, which reconnects
            // (DONE would only meet the closed socket and lose them).
            if let Out::Server(Ok(IdleResponse::NewData(resp))) = &out {
                if let Response::Data { status: Status::Bye, outcome } = resp.parsed() {
                    return Err(self.err_lost(outcome.information.as_deref()));
                }
            }
            s = crate::command_limit::within("the end of IDLE", h.done()).await.map_err(|e| self.core_err(e))?;
            match out {
                Out::Stop => break,
                Out::Reissue => {}
                Out::Server(Ok(IdleResponse::NewData(_))) => on_event(IdleEvent::Changed),
                Out::Server(Ok(_)) => {}
                Out::Server(Err(e)) => return Err(self.err_imap(e)),
            }
        }
        self.give_back(crate::adapter::Sess { s, caps });
        Ok(())
    }
}
