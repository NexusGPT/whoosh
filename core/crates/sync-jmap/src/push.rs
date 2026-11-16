//! EventSource push (JMAP RFC 8620 section 7.3): one connection, every mailbox. A state change is
//! only a hint; the messages come from `resync`. After every (re)connect the watch first catches
//! up from the stored cursor, so mail that arrived while the stream was down is not lost.
use std::time::Duration;

use contracts::{CoreError, SyncCursor};
use futures::StreamExt;
use jmap_client::event_source::{Changes, PushNotification};
use jmap_client::DataType;
use tokio::sync::watch;

use crate::adapter::JmapAdapter;
use crate::port::*;

/// Supplied by the caller from docs/CONSTANTS.md; nothing here is defaulted.
#[derive(Debug, Clone, Copy)]
pub struct PushConfig {
    /// Seconds between server pings that keep the stream open.
    pub ping_seconds: u32,
    /// How long to wait before opening the stream again.
    pub reconnect_delay: Duration,
    pub opts: FetchOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEnd {
    /// The caller asked the watch to stop.
    Stopped,
    /// The server can no longer list changes since the cursor: run the first screen again.
    NeedsReset { reason_words: String },
}

fn retryable(e: &CoreError) -> bool {
    matches!(e, CoreError::Action { failure } if failure.retryable)
}

fn words(e: &CoreError) -> String {
    match e {
        CoreError::Action { failure } => failure.reason_words.clone(),
        other => other.to_string(),
    }
}

impl JmapAdapter {
    fn email_state_moved(&self, changes: &Changes, cursor: &SyncCursor) -> bool {
        let SyncCursor::Jmap { state } = cursor else { return true };
        match changes.changes(&self.server_account) {
            Some(mut kinds) => kinds.any(|(kind, new_state)| *kind == DataType::Email && new_state != state),
            None => false,
        }
    }

    /// Resync from `cursor`, hand the result to the sink, and only then move the cursor.
    async fn catch_up(&self, cursor: &mut SyncCursor, sink: &mut dyn MessageSink, opts: FetchOptions) -> Result<Option<WatchEnd>, CoreError> {
        match self.resync(cursor, opts).await? {
            Resync::Reset { reason_words } => Ok(Some(WatchEnd::NeedsReset { reason_words })),
            Resync::Delta(delivery) => {
                let next = delivery.cursor.clone();
                sink.deliver(delivery).await?;
                if let Some(next) = next {
                    *cursor = next;
                }
                Ok(None)
            }
        }
    }

    /// Hold the EventSource stream and deliver every new message to `sink` until `stop` fires.
    /// Network and server trouble is reported through `sink.status` and retried after
    /// `reconnect_delay`; a refused sign-in or a sink error ends the watch with the error.
    pub async fn watch(&self, mut cursor: SyncCursor, sink: &mut dyn MessageSink, mut stop: watch::Receiver<bool>, cfg: PushConfig) -> Result<WatchEnd, CoreError> {
        loop {
            if *stop.borrow() {
                return Ok(WatchEnd::Stopped);
            }
            let attempt = self.hold_stream(&mut cursor, sink, &mut stop, cfg).await;
            match attempt {
                Ok(Some(end)) => return Ok(end),
                Ok(None) => {}
                Err(e) if retryable(&e) => sink.status(LinkStatus::Reconnecting { reason_words: words(&e) }).await,
                Err(e) => return Err(e),
            }
            tokio::select! {
                _ = tokio::time::sleep(cfg.reconnect_delay) => {}
                _ = stop.changed() => return Ok(WatchEnd::Stopped),
            }
        }
    }

    /// One connection's life. Ok(Some(end)) = leave the watch; Ok(None) = the stream ended, reconnect.
    async fn hold_stream(&self, cursor: &mut SyncCursor, sink: &mut dyn MessageSink, stop: &mut watch::Receiver<bool>, cfg: PushConfig) -> Result<Option<WatchEnd>, CoreError> {
        if let Some(end) = self.catch_up(cursor, sink, cfg.opts).await? {
            return Ok(Some(end));
        }
        let mut stream = self.client.event_source(Some([DataType::Email]), false, Some(cfg.ping_seconds), None).await.map_err(|e| self.err(e))?;
        sink.status(LinkStatus::Connected).await;
        loop {
            tokio::select! {
                _ = stop.changed() => return Ok(Some(WatchEnd::Stopped)),
                event = stream.next() => match event {
                    Some(Ok(PushNotification::StateChange(changes))) => {
                        if self.email_state_moved(&changes, cursor) {
                            if let Some(end) = self.catch_up(cursor, sink, cfg.opts).await? {
                                return Ok(Some(end));
                            }
                        }
                    }
                    Some(Ok(_)) => {}
                    // One event the parser cannot read is not worth the connection: the next event, or
                    // the catch-up after a reconnect, brings the state.
                    Some(Err(jmap_client::Error::Parse(_))) => {}
                    Some(Err(e)) => return Err(self.err(e)),
                    None => {
                        sink.status(LinkStatus::Reconnecting { reason_words: "the push stream closed".into() }).await;
                        return Ok(None);
                    }
                },
            }
        }
    }
}
