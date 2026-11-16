//! Change notifications. A notification only SIGNALS that a folder changed: the engine answers it
//! with a delta round, so a lost or duplicated notification costs a delay, never data.
//!
//! The Mac cannot be a public webhook, so the notification URL given to `create_subscription` is the
//! relay's (relay-graph lane); the relay forwards each payload here as received. Without a relay the
//! engine polls delta on a timer. Both paths end in the same delta round.
use std::collections::{BTreeSet, HashMap};

use contracts::CoreError;
use reqwest::Method;
use serde::Deserialize;
use serde_json::json;

use crate::{
    client::GraphMailbox,
    limits,
    model::{invalid, parse, rfc3339_ms},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphSubscription {
    pub id: String,
    pub resource: String,
    pub expires_at_ms: i64,
}

impl GraphSubscription {
    /// True once the subscription has `margin_ms` or less left; the margin is the engine's.
    pub fn needs_renewal(&self, now_ms: i64, margin_ms: i64) -> bool {
        self.expires_at_ms - now_ms <= margin_ms
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSubscription {
    id: String,
    resource: String,
    expiration_date_time: String,
}

fn expiry_text(now_ms: i64) -> Result<String, CoreError> {
    // "under N minutes": ask for one minute less than the ceiling.
    let at = now_ms + (limits::GRAPH_SUB_CEILING_MIN - 1) * 60_000;
    let dt = chrono::DateTime::from_timestamp_millis(at).ok_or_else(|| invalid("expirationDateTime", "out of range"))?;
    Ok(dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

impl GraphMailbox {
    fn subscription(raw: &str) -> Result<GraphSubscription, CoreError> {
        let r: RawSubscription = parse(raw, "subscription")?;
        Ok(GraphSubscription { id: r.id, resource: r.resource, expires_at_ms: rfc3339_ms("expirationDateTime", &r.expiration_date_time)? })
    }

    /// Subscribe to created, updated and deleted messages of one folder.
    pub async fn create_subscription(&self, folder_id: &str, notification_url: &str, lifecycle_url: Option<&str>, client_state: &str, now_ms: i64) -> Result<GraphSubscription, CoreError> {
        let mut body = json!({
            "changeType": "created,updated,deleted",
            "notificationUrl": notification_url,
            "resource": format!("{}/mailFolders('{folder_id}')/messages", self.cfg.mailbox_path),
            "expirationDateTime": expiry_text(now_ms)?,
            "clientState": client_state,
        });
        if let Some(l) = lifecycle_url {
            body["lifecycleNotificationUrl"] = json!(l);
        }
        let url = self.url(&["subscriptions"])?;
        Self::subscription(&self.send(Method::POST, url.as_str(), &[], Some(&body), false).await?.body)
    }

    pub async fn renew_subscription(&self, id: &str, now_ms: i64) -> Result<GraphSubscription, CoreError> {
        let url = self.url(&["subscriptions", id])?;
        let body = json!({ "expirationDateTime": expiry_text(now_ms)? });
        Self::subscription(&self.send(Method::PATCH, url.as_str(), &[], Some(&body), false).await?.body)
    }

    pub async fn delete_subscription(&self, id: &str) -> Result<(), CoreError> {
        let url = self.url(&["subscriptions", id])?;
        self.send(Method::DELETE, url.as_str(), &[], None, false).await.map(|_| ())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    Created,
    Updated,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub subscription_id: String,
    pub change_type: ChangeType,
    pub message_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleEvent {
    /// Renew now or the subscription stops delivering.
    ReauthorizationRequired,
    /// Microsoft dropped it: create a new one.
    SubscriptionRemoved,
    /// Notifications were lost: run a delta round for the folder.
    Missed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleNotice {
    pub subscription_id: String,
    pub event: LifecycleEvent,
}

/// What one delivery from Microsoft holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Inbound {
    /// Subscription handshake: answer 200 with this text within the time Microsoft gives.
    pub validation: Option<String>,
    pub notices: Vec<Notice>,
    pub lifecycle: Vec<LifecycleNotice>,
    /// Items dropped because their clientState did not match ours.
    pub rejected: usize,
}

#[derive(Deserialize)]
struct RawInbound {
    #[serde(default)]
    value: Vec<RawNotice>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawNotice {
    subscription_id: String,
    client_state: Option<String>,
    change_type: Option<String>,
    lifecycle_event: Option<String>,
    resource_data: Option<RawData>,
}

#[derive(Deserialize)]
struct RawData {
    id: Option<String>,
}

fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

/// Read one delivery. `validation_token` is the `validationToken` query value, when present.
pub fn parse_inbound(validation_token: Option<&str>, body: &str, expected_client_state: &str) -> Result<Inbound, CoreError> {
    let mut out = Inbound { validation: validation_token.map(str::to_string), ..Inbound::default() };
    if body.trim().is_empty() {
        return Ok(out);
    }
    let raw: RawInbound = parse(body, "notification")?;
    for n in raw.value {
        if !n.client_state.as_deref().is_some_and(|c| same(c, expected_client_state)) {
            out.rejected += 1;
            continue;
        }
        if let Some(ev) = n.lifecycle_event {
            let event = match ev.as_str() {
                "reauthorizationRequired" => LifecycleEvent::ReauthorizationRequired,
                "subscriptionRemoved" => LifecycleEvent::SubscriptionRemoved,
                "missed" => LifecycleEvent::Missed,
                other => return Err(invalid("lifecycleEvent", format!("{other:?} is not one this app knows"))),
            };
            out.lifecycle.push(LifecycleNotice { subscription_id: n.subscription_id, event });
            continue;
        }
        let change_type = match n.change_type.as_deref() {
            Some("created") => ChangeType::Created,
            Some("updated") => ChangeType::Updated,
            Some("deleted") => ChangeType::Deleted,
            other => return Err(invalid("changeType", format!("{other:?} is not one this app knows"))),
        };
        let message_id = n.resource_data.and_then(|d| d.id).ok_or_else(|| invalid("resourceData.id", "a notification names no message"))?;
        out.notices.push(Notice { subscription_id: n.subscription_id, change_type, message_id });
    }
    Ok(out)
}

/// The folders to run a delta round for after this delivery. `folder_of` maps subscription id to
/// folder id; a subscription this app does not know is ignored.
pub fn folders_to_sync(inbound: &Inbound, folder_of: &HashMap<String, String>) -> BTreeSet<String> {
    let notices = inbound.notices.iter().map(|n| &n.subscription_id);
    let missed = inbound.lifecycle.iter().filter(|l| l.event == LifecycleEvent::Missed).map(|l| &l.subscription_id);
    notices.chain(missed).filter_map(|s| folder_of.get(s).cloned()).collect()
}
