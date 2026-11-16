//! The Gmail REST shapes this adapter reads (users.history.list, users.messages.list/get, users.labels.list,
//! users.watch, users.getProfile), as serde types. Unknown fields are ignored.
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryList {
    #[serde(default)]
    pub history: Vec<HistoryRecord>,
    pub next_page_token: Option<String>,
    pub history_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRecord {
    #[serde(default)]
    pub messages_added: Vec<MessageEntry>,
    #[serde(default)]
    pub messages_deleted: Vec<MessageEntry>,
    #[serde(default)]
    pub labels_added: Vec<LabelEntry>,
    #[serde(default)]
    pub labels_removed: Vec<LabelEntry>,
}

#[derive(Debug, Deserialize)]
pub struct MessageEntry {
    pub message: MessageRef,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelEntry {
    pub message: MessageRef,
    #[serde(default)]
    pub label_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageRef {
    pub id: String,
    pub thread_id: Option<String>,
    #[serde(default)]
    pub label_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageList {
    #[serde(default)]
    pub messages: Vec<MessageRef>,
    pub next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireMessage {
    pub id: String,
    pub thread_id: Option<String>,
    #[serde(default)]
    pub label_ids: Vec<String>,
    pub history_id: Option<String>,
    pub internal_date: Option<String>,
    pub size_estimate: Option<u64>,
    pub raw: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub history_id: String,
}

#[derive(Debug, Deserialize)]
pub struct LabelList {
    #[serde(default)]
    pub labels: Vec<WireLabel>,
}

#[derive(Debug, Deserialize)]
pub struct WireLabel {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchReply {
    pub history_id: String,
    pub expiration: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct ApiErrorEnvelope {
    #[serde(default)]
    pub error: ApiError,
}

#[derive(Debug, Default, Deserialize)]
pub struct ApiError {
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub errors: Vec<ApiErrorDetail>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ApiErrorDetail {
    #[serde(default)]
    pub reason: String,
}
