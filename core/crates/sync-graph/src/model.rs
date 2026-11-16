//! The Graph message shapes this crate reads, and their mapping into the contract's `Message`.
use contracts::{CoreError, Message};
use serde::Deserialize;

pub(crate) fn invalid(field: &str, why: impl Into<String>) -> CoreError {
    CoreError::Invalid { field: field.to_string(), reason_words: why.into() }
}

pub(crate) fn parse<T: for<'de> Deserialize<'de>>(body: &str, what: &str) -> Result<T, CoreError> {
    serde_json::from_str(body).map_err(|e| invalid(what, format!("Microsoft sent a shape this app does not read: {e}")))
}

pub(crate) fn rfc3339_ms(field: &str, text: &str) -> Result<i64, CoreError> {
    chrono::DateTime::parse_from_rfc3339(text).map(|d| d.timestamp_millis()).map_err(|e| invalid(field, format!("{text:?} is not a date: {e}")))
}

#[derive(Deserialize)]
pub(crate) struct DeltaPage {
    #[serde(default)]
    pub value: Vec<GraphMessage>,
    #[serde(rename = "@odata.nextLink")]
    pub next_link: Option<String>,
    #[serde(rename = "@odata.deltaLink")]
    pub delta_link: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct Removed {
    pub reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GraphMessage {
    pub id: String,
    #[serde(rename = "@removed")]
    pub removed: Option<Removed>,
    pub conversation_id: Option<String>,
    pub subject: Option<String>,
    pub body_preview: Option<String>,
    pub received_date_time: Option<String>,
    pub is_read: Option<bool>,
    pub from: Option<Recipient>,
    pub to_recipients: Option<Vec<Recipient>>,
    /// contracts delta 12
    pub cc_recipients: Option<Vec<Recipient>>,
    pub bcc_recipients: Option<Vec<Recipient>>,
    /// Graph's parse of the List-Unsubscribe header (v1.0 message `unsubscribeData`).
    /// Graph exposes no List-Unsubscribe-Post, so one-click is never set from it.
    pub unsubscribe_data: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub(crate) struct Recipient {
    #[serde(rename = "emailAddress")]
    pub email_address: Option<EmailAddress>,
}

#[derive(Deserialize)]
pub(crate) struct EmailAddress {
    pub name: Option<String>,
    pub address: Option<String>,
}

/// What one delta item means for the store.
#[derive(Debug, Clone, PartialEq)]
pub enum DeltaChange {
    /// New or changed message, in full.
    Upsert(Message),
    /// Gone from this folder: deleted, or moved to another folder (`reason` is Graph's word).
    Removed { id: String, reason: String },
}

impl GraphMessage {
    pub(crate) fn into_change(self, account_id: &str, folder: &str) -> Result<DeltaChange, CoreError> {
        if let Some(r) = self.removed {
            return Ok(DeltaChange::Removed { id: self.id, reason: r.reason.unwrap_or_else(|| "deleted".into()) });
        }
        let received = self.received_date_time.ok_or_else(|| invalid("receivedDateTime", format!("message {} has none", self.id)))?;
        let (sender_name, from_address) = match self.from.and_then(|f| f.email_address) {
            Some(e) => {
                let address = e.address.unwrap_or_default();
                (e.name.filter(|n| !n.is_empty()).unwrap_or_else(|| address.clone()), address)
            }
            None => (String::new(), String::new()),
        };
        let to = self.to_recipients.unwrap_or_default().into_iter().filter_map(|r| r.email_address?.address).collect();
        let cc = self.cc_recipients.unwrap_or_default().into_iter().filter_map(|r| r.email_address?.address).collect();
        let bcc = self.bcc_recipients.unwrap_or_default().into_iter().filter_map(|r| r.email_address?.address).collect();
        let header = self.unsubscribe_data.unwrap_or_default().iter()
            .map(|e| { let e = e.trim(); if e.starts_with('<') { e.to_string() } else { format!("<{e}>") } })
            .collect::<Vec<_>>().join(", ");
        let (list_unsubscribe, list_unsubscribe_one_click) = contracts::list_unsubscribe_from_headers(Some(&header), None);
        Ok(DeltaChange::Upsert(Message { list_unsubscribe, list_unsubscribe_one_click,
            thread_id: self.conversation_id.unwrap_or_else(|| self.id.clone()),
            id: self.id,
            account_id: account_id.to_string(),
            sender_name,
            from_address,
            to,
            subject: self.subject.unwrap_or_default(),
            preview: self.body_preview.unwrap_or_default(),
            received_at_ms: rfc3339_ms("receivedDateTime", &received)?,
            read: self.is_read.unwrap_or(false),
            archived: false,
            folder: folder.to_string(),
            attachment_names: Vec::new(),
            login_code: None,
            trackers_blocked: 0,
            reply_to_name: None,
            cc,
            bcc,
            is_outgoing: false,
        }))
    }
}

#[cfg(test)]
mod d11_tests {
    use super::*;

    fn upsert(json: &str) -> Message {
        let m: GraphMessage = serde_json::from_str(json).expect("shape");
        match m.into_change("acc", "inbox").expect("maps") { DeltaChange::Upsert(m) => m, other => panic!("{other:?}") }
    }

    #[test]
    fn unsubscribe_data_fills_list_unsubscribe() {
        let m = upsert(r#"{"id":"m1","receivedDateTime":"2026-11-08T10:00:00Z","unsubscribeData":["mailto:u@list.example?subject=unsub","<https://list.example/u/1>"]}"#);
        assert_eq!(m.list_unsubscribe, vec!["<mailto:u@list.example?subject=unsub>".to_string(), "<https://list.example/u/1>".to_string()]);
        assert!(!m.list_unsubscribe_one_click);
        let none = upsert(r#"{"id":"m2","receivedDateTime":"2026-11-08T10:00:00Z"}"#);
        assert!(none.list_unsubscribe.is_empty());
    }

    #[test]
    fn cc_and_bcc_recipients_reach_the_message() {
        let m = upsert(r#"{"id":"m3","receivedDateTime":"2026-11-08T10:00:00Z","ccRecipients":[{"emailAddress":{"name":"Tom","address":"tom@y.example"}},{"emailAddress":{"address":"sara@z.example"}}],"bccRecipients":[{"emailAddress":{"address":"boss@x.example"}}]}"#);
        assert_eq!(m.cc, vec!["tom@y.example".to_string(), "sara@z.example".to_string()]);
        assert_eq!(m.bcc, vec!["boss@x.example".to_string()]);
        let none = upsert(r#"{"id":"m4","receivedDateTime":"2026-11-08T10:00:00Z"}"#);
        assert!(none.cc.is_empty() && none.bcc.is_empty());
    }
}
