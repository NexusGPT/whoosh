//! real-settings: the settings commands the real core did not serve ("The local engine does not do this yet."), on the store (`store/src/app_settings.rs`,
//! the pattern of `ai_settings.rs`), answering the way the test engine does (Sources/CoreBridge/FixtureSettings.swift, FixtureCore.swift):
//!
//! - SetNotchSetting, SetNotificationSetting, SetNotification, SetShortcutPreset: one row each (`notch_setting`, `notification_setting`, `shortcut_preset`).
//! - AddSplitTab, RenameSplitTab, ReorderSplitTabs, SetSplitTabEnabled, RemoveSplitTab, RestoreDefaultSplitTabs: the `split_tab` rows the store seeds at the first
//!   launch (the six default tabs); the tab counts read the same rows (store counts.rs), so a hidden tab leaves the counts at once.
//! - RemoveProviderKey and GiveAgentConsent are served by `ai_settings.rs` (the owner of the provider rows and keys).
//!
//! Every write answers on the store's writer, on this Mac, in well under 50 ms, and never waits for the network: nothing here opens a socket. None of these
//! acts is undoable (the test engine returns no undo entry for them), so none records one. The reads (`notch_setting()`, `notification_settings()`,
//! `shortcut_preset()`, `split_tabs()`) and the state paths below serve what the commands wrote, and the defaults before the first write.
//!
//! State paths (one JSON value each, the test engine's spelling):
//!   notch_setting/show                     "latest" | "unread"      (what the notch's short list holds; the test engine's odd name for it)
//!   notch_setting/enabled                  true | false
//!   notch_setting/buttons                  ["mark-read", ...] or "none"
//!   notch_setting/hidden_in_screen_share   true | false
//!   notch_setting/follow_focus             true | false
//!   notch_alerts/<field>                   the alert settings (contracts/src/notch_alerts.rs `notch_alerts_state`): vip | needs_reply | primary | updates | promotions |
//!                                          sign_in_codes -> "nothing" | "light" | "line" | "card" | "opened"; stay -> "5s" | "10s" | "until-acted"; sound, full_mail_loads_pictures
//!   notch_kind/<message>                   the kind of a stored mail for the notch: "sign_in_codes" | "vip" | "needs_reply" | "updates" | "promotions" | "primary"
//!   notification_setting/off               ["<account id>", ...] sorted: the muted accounts
//!   notification_setting/delivery          "notch" | "notifications" | "both": the one every account agrees on, "both" when they differ
//!   notification_setting/<account>/enabled | /delivery
//!   shortcut_preset/name                   "Superhuman" | "GotEmail"
//!   agent_consent/<provider>               true | false: the owner allowed mail text to go to that provider
//!   split_tab/count                        the number of tabs
//!   split_tab/<tab>/order | /enabled | /rule      the position, shown or hidden, "builtin:<id>" | "sender:<a>" | "domain:<d>" | "account:<id>" | "label:<id>" | "category:<c>"
//!   split_tab/<tab>/name                   served here only for a tab the owner renamed or added (the strip's own names stay state_extra's)
use std::collections::HashMap;
use std::sync::Arc;

use contracts::{Command, CommandOutcome, CoreError, NotchAlerts, NotchKind, NotchSetting, NotificationDelivery, NotificationSetting, ShortcutPreset, SplitTab, SplitTabRule};
use store::app_settings::{delivery_word, notch_default, SHORTCUT_PRESETS};
use store::Store;

use crate::reads::json_text;
use crate::rust_core::now_ms;

const OK: CommandOutcome = CommandOutcome { undo_entry_id: None, created_id: None };

/// The commands this module serves (one dispatch line in `RustCore::run`).
pub(crate) fn is_command(c: &Command) -> bool {
    matches!(
        c,
        Command::SetNotchSetting { .. } | Command::SetNotchAlerts { .. } | Command::SetNotificationSetting { .. } | Command::SetNotification { .. } | Command::SetShortcutPreset { .. }
            | Command::AddSplitTab { .. } | Command::RenameSplitTab { .. } | Command::ReorderSplitTabs { .. } | Command::SetSplitTabEnabled { .. }
            | Command::RemoveSplitTab { .. } | Command::RestoreDefaultSplitTabs
    )
}

pub(crate) fn run(store: &Arc<Store>, command: Command) -> Result<CommandOutcome, CoreError> {
    let now = now_ms();
    match command {
        Command::SetNotchSetting { setting } => store.notch_setting_put(setting)?,
        Command::SetNotchAlerts { alerts } => store.notch_alerts_put(alerts)?,
        Command::SetNotificationSetting { setting } => store.notification_put(setting, now)?,
        Command::SetNotification { account_id, enabled } => store.notification_mute(account_id, enabled, now)?,
        Command::SetShortcutPreset { preset } => store.shortcut_preset_put(preset, now)?,
        Command::AddSplitTab { name, rule } => return Ok(CommandOutcome { undo_entry_id: None, created_id: Some(store.split_tab_add(name, rule, now)?) }),
        Command::RenameSplitTab { tab_id, name } => store.split_tab_rename(tab_id, name, now)?,
        Command::ReorderSplitTabs { ids } => store.split_tab_reorder(ids, now)?,
        Command::SetSplitTabEnabled { id, on } => store.split_tab_set_enabled(id, on, now)?,
        Command::RemoveSplitTab { id } => store.split_tab_remove(id)?,
        Command::RestoreDefaultSplitTabs => store.split_tab_restore_defaults()?,
        other => return Err(CoreError::Invalid { field: "command".into(), reason_words: format!("{other:?} is not a settings command") }),
    }
    Ok(OK)
}

// ---------------------------------------------------------------- reads

/// The notch settings: the owner's, else the documented defaults (the test engine's).
pub(crate) fn notch(store: &Store) -> NotchSetting { store.notch_setting_row().ok().flatten().unwrap_or_else(notch_default) }

/// The notch's alert settings: the owner's, else the picked start values (`contracts::notch_alerts_default`).
pub(crate) fn notch_alerts(store: &Store) -> NotchAlerts { store.notch_alerts_row().ok().flatten().unwrap_or_else(contracts::notch_alerts_default) }

/// The kind of one stored mail, for the notch; Primary for an id the store does not hold.
pub(crate) fn notch_kind(store: &Store, message_id: &str) -> NotchKind {
    store.notch_facts(message_id).ok().flatten().map(contracts::notch_kind_of).unwrap_or(NotchKind::Primary)
}

/// One setting per account, in the sidebar's order: the owner's, else on and delivering to both.
pub(crate) fn notifications(store: &Store) -> Vec<NotificationSetting> {
    let saved = store.notification_rows().unwrap_or_default();
    store.account_ids().unwrap_or_default().into_iter()
        .map(|id| saved.iter().find(|s| s.account_id == id).cloned().unwrap_or(NotificationSetting { account_id: id, enabled: true, delivery: Some(NotificationDelivery::Both) }))
        .collect()
}

pub(crate) fn shortcut_preset(store: &Store) -> ShortcutPreset {
    ShortcutPreset { preset: store.shortcut_preset_name().ok().flatten().unwrap_or_else(|| SHORTCUT_PRESETS[0].to_string()), overrides: HashMap::new() }
}

/// The strip's rows in order. A default tab's rule names the tab's id, as the test engine's does (`.builtin(name: id)`); the store keeps the display name.
pub(crate) fn split_tabs(store: &Store) -> Vec<SplitTab> {
    store.split_tab_rows().unwrap_or_default().into_iter()
        .map(|r| { let mut t = r.tab; if matches!(t.rule, SplitTabRule::Builtin { .. }) { t.rule = SplitTabRule::Builtin { name: t.id.clone() }; } t })
        .collect()
}

// ---------------------------------------------------------------- state paths

fn strings(words: impl Iterator<Item = String>) -> String { format!("[{}]", words.map(|w| json_text(&w)).collect::<Vec<_>>().join(",")) }

fn rule_word(r: &SplitTabRule) -> String {
    match r {
        SplitTabRule::Builtin { name } => format!("builtin:{name}"),
        SplitTabRule::Sender { address } => format!("sender:{address}"),
        SplitTabRule::Domain { domain } => format!("domain:{domain}"),
        SplitTabRule::Account { account_id } => format!("account:{account_id}"),
        SplitTabRule::Label { label_id } => format!("label:{label_id}"),
        SplitTabRule::DecisionCategory { category } => format!("category:{category}"),
    }
}

/// The delivery every account agrees on; both when they differ (`FixtureSettingsBook.commonDelivery`).
fn common_delivery(all: &[NotificationSetting]) -> NotificationDelivery {
    let words: std::collections::BTreeSet<&str> = all.iter().map(|s| delivery_word(s.delivery.unwrap_or(NotificationDelivery::Both))).collect();
    match words.iter().next() {
        Some(&"notch") if words.len() == 1 => NotificationDelivery::Notch,
        Some(&"notifications") if words.len() == 1 => NotificationDelivery::Notifications,
        _ => NotificationDelivery::Both,
    }
}

pub(crate) fn state(store: &Store, path: &str) -> Option<String> {
    let p: Vec<&str> = path.split('/').collect();
    match p.as_slice() {
        ["notch_setting", field] => {
            let n = notch(store);
            match *field {
                "show" => Some(json_text(match n.list_shows { contracts::NotchList::Latest => "latest", contracts::NotchList::Unread => "unread" })),
                "enabled" => Some(n.show.to_string()),
                "buttons" => Some(if n.buttons.is_empty() { json_text("none") } else { strings(n.buttons.into_iter()) }),
                "hidden_in_screen_share" => Some(n.hidden_in_screen_share.to_string()),
                "follow_focus" => Some(n.follow_focus.to_string()),
                _ => None,
            }
        }
        ["notch_alerts", field] => contracts::notch_alerts_state(notch_alerts(store), field.to_string()),
        ["notch_kind", message] => Some(json_text(&contracts::notch_kind_word(notch_kind(store, message)))),
        ["notification_setting", "off"] => {
            let mut off: Vec<String> = notifications(store).into_iter().filter(|s| !s.enabled).map(|s| s.account_id).collect();
            off.sort();
            Some(strings(off.into_iter()))
        }
        ["notification_setting", "delivery"] => Some(json_text(delivery_word(common_delivery(&notifications(store))))),
        ["notification_setting", account, field] => {
            let s = notifications(store).into_iter().find(|s| s.account_id == *account)?;
            match *field {
                "enabled" => Some(s.enabled.to_string()),
                "delivery" => Some(json_text(delivery_word(s.delivery.unwrap_or(NotificationDelivery::Both)))),
                _ => None,
            }
        }
        ["shortcut_preset", "name"] => Some(json_text(&shortcut_preset(store).preset)),
        ["agent_consent", provider] => {
            if *provider != crate::ai_settings::JEV_ID && !crate::ai_settings::providers(store).iter().any(|a| a.id == *provider) { return None; }
            Some(store.agent_consent_at(provider).ok().flatten().is_some().to_string())
        }
        ["split_tab", "count"] => Some(store.split_tab_rows().ok()?.len().to_string()),
        ["split_tab", id, field] => {
            let rows = store.split_tab_rows().ok()?;
            let row = rows.iter().find(|r| r.tab.id == *id)?;
            match *field {
                // a seeded name is the strip's (state_extra answers it from the labels); an edited one is this table's
                "name" => (row.updated_at_ms > 0).then(|| json_text(&row.tab.name)),
                "order" => Some(row.tab.order.to_string()),
                "enabled" => Some(row.tab.enabled.to_string()),
                "rule" => Some(json_text(&rule_word(&split_tabs_rule(&row.tab)))),
                _ => None,
            }
        }
        _ => None,
    }
}

fn split_tabs_rule(t: &SplitTab) -> SplitTabRule {
    match &t.rule { SplitTabRule::Builtin { .. } => SplitTabRule::Builtin { name: t.id.clone() }, other => other.clone() }
}
