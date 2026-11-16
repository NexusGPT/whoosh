//! The two synchronous ports other crates call into IMAP through. The core's send path is synchronous
//! (the outbox ticks on a timer thread), the adapter is async: each call here blocks the CALLING thread on
//! the adapter's future, so it must not be called from inside the async runtime's own worker threads.
use crate::adapter::ImapAdapter;
use crate::port::ImapMailAdapter;
use contracts::{AppendSentPort, AppendSentRequest, CoreError};
use send::{parse_message_ids, ParentHeaders, ParentHeadersPort, ParentRef};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Handle;

/// One IMAP adapter per account, found by account id.
#[derive(Clone)]
pub struct AccountAdapters {
    by_account: HashMap<String, Arc<ImapAdapter>>,
    runtime: Handle,
}

impl AccountAdapters {
    pub fn new(runtime: Handle) -> Self {
        Self { by_account: HashMap::new(), runtime }
    }

    pub fn with(mut self, account_id: &str, adapter: Arc<ImapAdapter>) -> Self {
        self.by_account.insert(account_id.to_string(), adapter);
        self
    }

    fn adapter(&self, account_id: &str) -> Result<&Arc<ImapAdapter>, CoreError> {
        self.by_account.get(account_id).ok_or_else(|| CoreError::Unexpected { reason_words: format!("no IMAP connection is set up for the account {account_id}") })
    }

    /// A144: `ImapAdapter::thread_headers_many` for a synchronous caller: blocks the calling thread (never a runtime worker thread).
    pub fn thread_headers_many(&self, account_id: &str, folder: &str, uids: &[u32]) -> Result<Vec<(u32, ParentHeaders)>, CoreError> {
        let adapter = self.adapter(account_id)?;
        self.runtime.block_on(adapter.thread_headers_many(folder, uids))
    }
}

impl AccountAdapters {
    /// real-replyto: `thread_headers_many` with each mail's Reply-To (name, address) from the same fetch; blocks the calling thread.
    pub fn thread_headers_reply_to_many(&self, account_id: &str, folder: &str, uids: &[u32]) -> Result<Vec<(u32, ParentHeaders, Vec<(String, String)>)>, CoreError> {
        let adapter = self.adapter(account_id)?;
        self.runtime.block_on(adapter.thread_headers_reply_to_many(folder, uids))
    }
}

/// `AppendSentPort` over IMAP: the copy goes to the account's Sent folder (nothing for Gmail, which saves it).
pub struct ImapSentAppender(pub AccountAdapters);

impl AppendSentPort for ImapSentAppender {
    fn execute(&self, request: &AppendSentRequest) -> Result<(), CoreError> {
        let adapter = self.0.adapter(&request.account_id)?;
        self.0.runtime.block_on(adapter.append_sent(&request.rfc822))
    }
}

/// `ParentHeadersPort` over IMAP: the Message-ID and References of the message a reply answers.
pub struct ImapParentHeaders(pub AccountAdapters);

/// The values of every header called `name` in a header block, folded lines joined (RFC 5322 2.2.3).
fn header_values(block: &[u8], name: &str) -> Vec<String> {
    let text = String::from_utf8_lossy(block).replace("\r\n", "\n");
    let unfolded = text.replace("\n ", " ").replace("\n\t", " ");
    unfolded.lines().filter_map(|l| l.split_once(':')).filter(|(n, _)| n.trim().eq_ignore_ascii_case(name)).map(|(_, v)| v.trim().to_string()).collect()
}

/// Every message id (angle brackets kept) in every header called `name`, in header order.
fn header_ids(header_block: &[u8], name: &str) -> Vec<String> {
    header_values(header_block, name).iter().flat_map(|v| parse_message_ids(v)).collect()
}

/// A144: a mail's own chain: References, else In-Reply-To (RFC 5322 3.6.4), each id in angle brackets; empty when it has neither.
pub(crate) fn thread_chain(header_block: &[u8]) -> Vec<String> {
    let references = header_ids(header_block, "References");
    if references.is_empty() { header_ids(header_block, "In-Reply-To") } else { references }
}

/// Message-ID, and the chain: References, else In-Reply-To (RFC 5322 3.6.4), as the parent's own headers say.
pub fn parent_headers_from(header_block: &[u8]) -> Result<ParentHeaders, CoreError> {
    let message_id = header_ids(header_block, "Message-ID").into_iter().next().ok_or_else(|| CoreError::Invalid { field: "message_id".into(), reason_words: "the message you reply to has no Message-ID header".into() })?;
    Ok(ParentHeaders { message_id, references: thread_chain(header_block) })
}

impl ParentHeadersPort for ImapParentHeaders {
    fn execute(&self, parent: &ParentRef) -> Result<ParentHeaders, CoreError> {
        let uid: u32 = parent.remote_id.parse().map_err(|_| CoreError::Invalid { field: "remote_id".into(), reason_words: format!("'{}' is not an IMAP UID", parent.remote_id) })?;
        let adapter = self.0.adapter(&parent.account_id)?;
        let headers = self.0.runtime.block_on(adapter.message_headers(&parent.folder, uid))?;
        parent_headers_from(&headers)
    }
}
