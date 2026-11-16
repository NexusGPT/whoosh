//! real-settings: the settings the owner makes outside Settings, AI and agents, kept in the store (the pattern of `ai_settings.rs`): the notch
//! (`notch_setting`), where each account announces mail (`notification_setting`), the key preset (`shortcut_preset`), the split tabs
//! (`split_tab`) and the owner's "Allow" on the mail consent question (`ai_provider.mail_consent_at_ms`). Migration 0028 added the columns the
//! first two and the last lacked. No query here takes a tenant: the store is one owner's, on one Mac.
//!
//! A refusal is a `Refused`: the words the owner reads, or "not found" with the kind and the id, as the test engine's `CoreError`s say them
//! (Sources/CoreBridge/FixtureSettings.swift, FixtureCore.swift). A refusal writes nothing: every check runs before the first statement.
//! Every write goes through the store's writer, so it is on disk at once and survives a restart; none waits for the network.
use crate::error::StoreError;
use crate::Store;
use contracts::{notch_alert_style_from_word, notch_alert_style_word, notch_alerts_default, notch_stay_from_word, notch_stay_word, CoreError, NotchAlertStyle, NotchAlerts, NotchList, NotchSetting, NotificationDelivery, NotificationSetting, SplitTab, SplitTabRule};
use rusqlite::{params, Connection, OptionalExtension};

/// Why a settings write did not happen.
#[derive(Debug)]
pub enum Refused {
    /// A refusal in words, naming the field it is about.
    Invalid { field: &'static str, words: String },
    NotFound { kind: &'static str, id: String },
    Store(StoreError),
}

impl From<StoreError> for Refused {
    fn from(e: StoreError) -> Self { Refused::Store(e) }
}

impl From<rusqlite::Error> for Refused {
    fn from(e: rusqlite::Error) -> Self { Refused::Store(StoreError::Sqlite(e)) }
}

impl From<Refused> for CoreError {
    fn from(r: Refused) -> Self {
        match r {
            Refused::Invalid { field, words } => CoreError::Invalid { field: field.into(), reason_words: words },
            Refused::NotFound { kind, id } => CoreError::NotFound { kind: kind.into(), id },
            Refused::Store(e) => e.into(),
        }
    }
}

fn invalid<T>(field: &'static str, words: impl Into<String>) -> Result<T, Refused> { Err(Refused::Invalid { field, words: words.into() }) }

/// The notch buttons the contract names (`Command::SetNotchSetting`: mark-read, archive, reply, delete).
pub const NOTCH_BUTTONS: [&str; 4] = ["mark-read", "archive", "reply", "delete"];
/// The key presets (`Command::SetShortcutPreset`). The first is the default.
pub const SHORTCUT_PRESETS: [&str; 2] = ["Superhuman", "GotEmail"];
/// What the notch holds before the owner saves it (the test engine's `FixtureSettingsBook.notch`). Reply is among the start buttons since the owner's pick
/// of 14 Nov 2026: the picked new-mail card shows Open, Reply, Archive and Mark read.
pub fn notch_default() -> NotchSetting {
    NotchSetting { show: true, buttons: vec!["mark-read".into(), "archive".into(), "reply".into()], hidden_in_screen_share: true, follow_focus: true, list_shows: NotchList::Latest }
}

fn list_word(l: NotchList) -> &'static str { match l { NotchList::Latest => "latest", NotchList::Unread => "unread" } }
fn list_from(w: &str) -> NotchList { if w == "unread" { NotchList::Unread } else { NotchList::Latest } }

/// The one spelling of a delivery in `notification_setting.delivery` and in the `notification_setting/<account>/delivery` state value.
pub fn delivery_word(d: NotificationDelivery) -> &'static str {
    match d { NotificationDelivery::Notch => "notch", NotificationDelivery::Notifications => "notifications", NotificationDelivery::Both => "both" }
}
fn delivery_from(w: &str) -> NotificationDelivery {
    match w { "notch" => NotificationDelivery::Notch, "notifications" => NotificationDelivery::Notifications, _ => NotificationDelivery::Both }
}

fn buttons_json(buttons: &[String]) -> String {
    format!("[{}]", buttons.iter().map(|b| format!("\"{b}\"")).collect::<Vec<_>>().join(","))
}

/// The buttons are plain words from `NOTCH_BUTTONS`, so no escaping is needed; anything else reads as empty.
fn buttons_from(json: &str) -> Vec<String> {
    let t = json.trim();
    let Some(inner) = t.strip_prefix('[').and_then(|r| r.strip_suffix(']')) else { return vec![] };
    inner.split(',').map(|w| w.trim().trim_matches('"').to_string()).filter(|w| !w.is_empty()).collect()
}

/// The split tab's rule as the two columns `rule_kind`, `rule_value`.
pub fn rule_columns(r: &SplitTabRule) -> (&'static str, String) {
    match r {
        SplitTabRule::Sender { address } => ("sender", address.clone()),
        SplitTabRule::Domain { domain } => ("domain", domain.clone()),
        SplitTabRule::Account { account_id } => ("account", account_id.clone()),
        SplitTabRule::Label { label_id } => ("label", label_id.clone()),
        SplitTabRule::DecisionCategory { category } => ("decision_category", category.clone()),
        SplitTabRule::Builtin { name } => ("builtin", name.clone()),
    }
}

pub fn rule_from_columns(kind: &str, value: &str) -> SplitTabRule {
    let v = value.to_string();
    match kind {
        "sender" => SplitTabRule::Sender { address: v },
        "domain" => SplitTabRule::Domain { domain: v },
        "account" => SplitTabRule::Account { account_id: v },
        "label" => SplitTabRule::Label { label_id: v },
        "decision_category" => SplitTabRule::DecisionCategory { category: v },
        _ => SplitTabRule::Builtin { name: v },
    }
}

/// The id of a tab the owner adds: the name's letters and digits, joined by single "-" (the test engine's `slug`).
pub fn tab_slug(name: &str) -> String {
    let s: String = name.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
    let parts: Vec<&str> = s.split('-').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() { "tab".into() } else { parts.join("-") }
}

/// One `split_tab` row with the time of its last edit (0 = as shipped: the seed writes 0, so a name or a flag the owner changed is told apart).
#[derive(Debug, Clone, PartialEq)]
pub struct SplitTabRow {
    pub tab: SplitTab,
    pub updated_at_ms: i64,
}

fn tab_rows(c: &Connection) -> Result<Vec<SplitTabRow>, StoreError> {
    let mut st = c.prepare_cached("SELECT id, name, sort_order, rule_kind, rule_value, enabled, updated_at_ms FROM split_tab ORDER BY sort_order, id")?;
    let rows = st.query_map([], |r| {
        Ok(SplitTabRow {
            tab: SplitTab { id: r.get(0)?, name: r.get(1)?, order: 0, rule: rule_from_columns(&r.get::<_, String>(3)?, &r.get::<_, String>(4)?), enabled: r.get::<_, i64>(5)? != 0 },
            updated_at_ms: r.get(6)?,
        })
    })?;
    let mut out = rows.collect::<Result<Vec<_>, _>>()?;
    for (i, r) in out.iter_mut().enumerate() { r.tab.order = i as u32; } // the position in the strip, whatever gaps `sort_order` holds
    Ok(out)
}

fn exists(c: &Connection, sql: &str, id: &str) -> Result<bool, rusqlite::Error> {
    Ok(c.query_row(sql, params![id], |r| r.get::<_, i64>(0))? > 0)
}

impl Store {
    // ------------------------------------------------------------ the notch

    /// The notch settings the owner saved; None before the first save (the caller serves `notch_default`).
    pub fn notch_setting_row(&self) -> Result<Option<NotchSetting>, StoreError> {
        self.read(|c| {
            Ok(c.prepare_cached("SELECT show, buttons_json, hidden_in_screen_share, follow_focus, list_shows FROM notch_setting WHERE id = 1")?
                .query_row([], |r| {
                    Ok(NotchSetting {
                        show: r.get::<_, i64>(0)? == 1, buttons: buttons_from(&r.get::<_, String>(1)?), hidden_in_screen_share: r.get::<_, i64>(2)? == 1,
                        follow_focus: r.get::<_, i64>(3)? == 1, list_shows: list_from(&r.get::<_, String>(4)?),
                    })
                })
                .optional()?)
        })
    }

    /// Saves the notch settings whole. A button outside `NOTCH_BUTTONS` is refused in words; a button named twice is kept once, in the order given.
    pub fn notch_setting_put(&self, s: NotchSetting) -> Result<(), Refused> {
        if let Some(bad) = s.buttons.iter().find(|b| !NOTCH_BUTTONS.contains(&b.as_str())) {
            return invalid("buttons", format!("“{bad}” is not a notch button; the buttons are mark-read, archive, reply and delete"));
        }
        let mut buttons: Vec<String> = Vec::new();
        for b in &s.buttons { if !buttons.contains(b) { buttons.push(b.clone()); } }
        Ok(self.write(move |ctx| {
            ctx.tx.execute(
                "INSERT INTO notch_setting (id, show, buttons_json, hidden_in_screen_share, follow_focus, list_shows) VALUES (1, ?1, ?2, ?3, ?4, ?5) \
                 ON CONFLICT(id) DO UPDATE SET show = excluded.show, buttons_json = excluded.buttons_json, hidden_in_screen_share = excluded.hidden_in_screen_share, \
                 follow_focus = excluded.follow_focus, list_shows = excluded.list_shows",
                params![s.show as i64, buttons_json(&buttons), s.hidden_in_screen_share as i64, s.follow_focus as i64, list_word(s.list_shows)],
            )?;
            Ok(())
        })?)
    }

    // ------------------------------------------------------------ the notch's alerts (migration 0033)

    /// The alert settings the owner saved; None before the first save (the caller serves `contracts::notch_alerts_default`). The table's CHECKs hold every
    /// word to the contract's spelling; a word the contract no longer knows would read as that field's start value.
    pub fn notch_alerts_row(&self) -> Result<Option<NotchAlerts>, StoreError> {
        self.read(|c| {
            Ok(c.prepare_cached("SELECT vip, needs_reply, primary_mail, updates, promotions, sign_in_codes, stay, sound, full_mail_loads_pictures FROM notch_alerts WHERE id = 1")?
                .query_row([], |r| {
                    let d = notch_alerts_default();
                    let style = |i: usize, start: NotchAlertStyle| -> rusqlite::Result<NotchAlertStyle> { Ok(notch_alert_style_from_word(r.get::<_, String>(i)?).unwrap_or(start)) };
                    Ok(NotchAlerts {
                        vip: style(0, d.vip)?, needs_reply: style(1, d.needs_reply)?, primary: style(2, d.primary)?, updates: style(3, d.updates)?,
                        promotions: style(4, d.promotions)?, sign_in_codes: style(5, d.sign_in_codes)?,
                        stay: notch_stay_from_word(r.get::<_, String>(6)?).unwrap_or(d.stay), sound: r.get::<_, i64>(7)? == 1,
                        full_mail_loads_pictures: r.get::<_, i64>(8)? == 1,
                    })
                })
                .optional()?)
        })
    }

    /// Saves the alert settings whole (one row).
    pub fn notch_alerts_put(&self, a: NotchAlerts) -> Result<(), Refused> {
        Ok(self.write(move |ctx| {
            ctx.tx.execute(
                "INSERT INTO notch_alerts (id, vip, needs_reply, primary_mail, updates, promotions, sign_in_codes, stay, sound, full_mail_loads_pictures) \
                 VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
                 ON CONFLICT(id) DO UPDATE SET vip = excluded.vip, needs_reply = excluded.needs_reply, primary_mail = excluded.primary_mail, updates = excluded.updates, \
                 promotions = excluded.promotions, sign_in_codes = excluded.sign_in_codes, stay = excluded.stay, sound = excluded.sound, \
                 full_mail_loads_pictures = excluded.full_mail_loads_pictures",
                params![notch_alert_style_word(a.vip), notch_alert_style_word(a.needs_reply), notch_alert_style_word(a.primary), notch_alert_style_word(a.updates),
                        notch_alert_style_word(a.promotions), notch_alert_style_word(a.sign_in_codes), notch_stay_word(a.stay), a.sound as i64, a.full_mail_loads_pictures as i64],
            )?;
            Ok(())
        })?)
    }

    // ------------------------------------------------------------ notifications

    // `account_ids` (the ids in the sidebar's order) is defined once, in realcore.rs (the accounts builder added the same read).

    /// The setting the owner saved for each account that has one (an account with no row is on, delivering to both: the caller fills it in).
    pub fn notification_rows(&self) -> Result<Vec<NotificationSetting>, StoreError> {
        self.read(|c| {
            let mut st = c.prepare_cached("SELECT account_id, enabled, delivery FROM notification_setting ORDER BY account_id")?;
            let rows = st.query_map([], |r| {
                Ok(NotificationSetting { account_id: r.get(0)?, enabled: r.get::<_, i64>(1)? == 1, delivery: Some(delivery_from(&r.get::<_, String>(2)?)) })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// Saves one account's setting whole; an unknown account is NotFound. `delivery` None is "not chosen": both (the contract's default).
    pub fn notification_put(&self, s: NotificationSetting, now_ms: i64) -> Result<(), Refused> { self.notification_write(s, false, now_ms) }

    /// Mutes or unmutes one account, leaving where it announces as it was.
    pub fn notification_mute(&self, account_id: String, enabled: bool, now_ms: i64) -> Result<(), Refused> {
        self.notification_write(NotificationSetting { account_id, enabled, delivery: None }, true, now_ms)
    }

    fn notification_write(&self, s: NotificationSetting, keep_delivery: bool, now_ms: i64) -> Result<(), Refused> {
        Ok(self.write(move |ctx| {
            if !exists(ctx.tx, "SELECT count(*) FROM account WHERE id = ?1", &s.account_id)? {
                return Ok(Err(Refused::NotFound { kind: "account", id: s.account_id }));
            }
            let kept: Option<String> = ctx.tx.query_row("SELECT delivery FROM notification_setting WHERE account_id = ?1", params![s.account_id], |r| r.get(0)).optional()?;
            let word = match (s.delivery, keep_delivery) {
                (Some(d), _) => delivery_word(d).to_string(),
                (None, true) => kept.unwrap_or_else(|| "both".into()),
                (None, false) => "both".into(),
            };
            ctx.tx.execute(
                "INSERT INTO notification_setting (account_id, enabled, updated_at_ms, delivery) VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT(account_id) DO UPDATE SET enabled = excluded.enabled, updated_at_ms = excluded.updated_at_ms, delivery = excluded.delivery",
                params![s.account_id, s.enabled as i64, now_ms, word],
            )?;
            Ok(Ok(()))
        })??)
    }

    // ------------------------------------------------------------ the key preset

    /// The preset the owner picked; None before the first pick (the caller serves the first of `SHORTCUT_PRESETS`).
    pub fn shortcut_preset_name(&self) -> Result<Option<String>, StoreError> {
        self.read(|c| Ok(c.prepare_cached("SELECT preset FROM shortcut_preset WHERE id = 1")?.query_row([], |r| r.get(0)).optional()?))
    }

    /// Picks the preset; a name outside `SHORTCUT_PRESETS` is refused in words (a key the owner changed by hand stays: `overrides_json` is not touched).
    pub fn shortcut_preset_put(&self, preset: String, now_ms: i64) -> Result<(), Refused> {
        if !SHORTCUT_PRESETS.contains(&preset.as_str()) { return invalid("preset", "the presets are Superhuman and GotEmail"); }
        Ok(self.write(move |ctx| {
            ctx.tx.execute(
                "INSERT INTO shortcut_preset (id, preset, updated_at_ms) VALUES (1, ?1, ?2) ON CONFLICT(id) DO UPDATE SET preset = excluded.preset, updated_at_ms = excluded.updated_at_ms",
                params![preset, now_ms],
            )?;
            Ok(())
        })?)
    }

    // ------------------------------------------------------------ the split tabs

    /// Every tab of the strip in order, with the time of its last edit (0 = as shipped).
    pub fn split_tab_rows(&self) -> Result<Vec<SplitTabRow>, StoreError> { self.read(tab_rows) }

    /// Adds a tab after the last one; returns its id. See `Command::AddSplitTab` for the refusals.
    pub fn split_tab_add(&self, name: String, rule: SplitTabRule, now_ms: i64) -> Result<String, Refused> {
        let name = name.trim().to_string();
        if name.is_empty() { return invalid("name", "a tab needs a name"); }
        match &rule {
            SplitTabRule::Builtin { .. } => return invalid("rule", "pick a rule: sender, domain, account, label or decision category"),
            SplitTabRule::Sender { address } if !address.contains('@') || address.starts_with('@') || address.ends_with('@') => {
                return invalid("rule", "a sender is a full address, like orders@example.com");
            }
            SplitTabRule::Domain { domain } if domain.contains('@') || !domain.contains('.') => return invalid("rule", "a domain looks like example.com"),
            SplitTabRule::DecisionCategory { category } if category.trim().is_empty() => return invalid("rule", "name the decision category"),
            _ => {}
        }
        Ok(self.write(move |ctx| {
            match &rule {
                SplitTabRule::Account { account_id } if !exists(ctx.tx, "SELECT count(*) FROM account WHERE id = ?1", account_id)? => {
                    return Ok(Err(Refused::NotFound { kind: "account", id: account_id.clone() }));
                }
                SplitTabRule::Label { label_id } if !exists(ctx.tx, "SELECT count(*) FROM label WHERE id = ?1", label_id)? => {
                    return Ok(Err(Refused::NotFound { kind: "label", id: label_id.clone() }));
                }
                _ => {}
            }
            let rows = tab_rows(ctx.tx)?;
            if rows.iter().any(|r| r.tab.name.to_lowercase() == name.to_lowercase()) {
                return Ok(Err(Refused::Invalid { field: "name", words: format!("a tab called “{name}” exists already") }));
            }
            let base = tab_slug(&name);
            let (mut id, mut k) = (base.clone(), 2);
            while rows.iter().any(|r| r.tab.id == id) { id = format!("{base}-{k}"); k += 1; }
            let (kind, value) = rule_columns(&rule);
            ctx.tx.execute(
                "INSERT INTO split_tab (id, name, sort_order, rule_kind, rule_value, enabled, updated_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)",
                params![id, name, rows.len() as i64, kind, value, now_ms],
            )?;
            Ok(Ok(id))
        })??)
    }

    /// Renames one tab (the default tabs too: only the name changes). An unknown tab is NotFound, an empty name is refused in words.
    pub fn split_tab_rename(&self, id: String, name: String, now_ms: i64) -> Result<(), Refused> {
        let name = name.trim().to_string();
        Ok(self.write(move |ctx| {
            if !exists(ctx.tx, "SELECT count(*) FROM split_tab WHERE id = ?1", &id)? { return Ok(Err(Refused::NotFound { kind: "split tab", id })); }
            if name.is_empty() { return Ok(Err(Refused::Invalid { field: "name", words: "a tab needs a name".into() })); }
            ctx.tx.execute("UPDATE split_tab SET name = ?2, updated_at_ms = ?3 WHERE id = ?1", params![id, name, now_ms])?;
            Ok(Ok(()))
        })??)
    }

    /// Puts the tabs in order: `ids` must hold every tab exactly once, else it is refused in words.
    pub fn split_tab_reorder(&self, ids: Vec<String>, now_ms: i64) -> Result<(), Refused> {
        Ok(self.write(move |ctx| {
            let rows = tab_rows(ctx.tx)?;
            let mut want: Vec<&str> = ids.iter().map(String::as_str).collect();
            want.sort_unstable();
            let mut have: Vec<&str> = rows.iter().map(|r| r.tab.id.as_str()).collect();
            have.sort_unstable();
            if want != have { return Ok(Err(Refused::Invalid { field: "ids", words: "the order must list every tab once".into() })); }
            for (i, id) in ids.iter().enumerate() {
                ctx.tx.execute("UPDATE split_tab SET sort_order = ?2, updated_at_ms = ?3 WHERE id = ?1", params![id, i as i64, now_ms])?;
            }
            Ok(Ok(()))
        })??)
    }

    /// Shows or hides one tab (hiding keeps it in the store). The last shown tab cannot be hidden. A tab that is a label (a built-in category or an
    /// owner's label) is pinned or unpinned with it: the strip the app draws is the pinned labels.
    pub fn split_tab_set_enabled(&self, id: String, on: bool, now_ms: i64) -> Result<(), Refused> {
        Ok(self.write(move |ctx| {
            let rows = tab_rows(ctx.tx)?;
            let Some(row) = rows.iter().find(|r| r.tab.id == id) else { return Ok(Err(Refused::NotFound { kind: "split tab", id })) };
            if !on && row.tab.enabled && rows.iter().filter(|r| r.tab.enabled).count() == 1 {
                return Ok(Err(Refused::Invalid { field: "enabled", words: "at least one tab has to stay".into() }));
            }
            ctx.tx.execute("UPDATE split_tab SET enabled = ?2, updated_at_ms = ?3 WHERE id = ?1", params![id, on as i64, now_ms])?;
            ctx.tx.execute("UPDATE label SET pinned = ?2, updated_at_ms = ?3 WHERE id = ?1 AND pinned != ?2", params![id, on as i64, now_ms])?;
            Ok(Ok(()))
        })??)
    }

    /// Removes a tab the owner added. The default tabs cannot be removed (hide them), an unknown tab is NotFound.
    pub fn split_tab_remove(&self, id: String) -> Result<(), Refused> {
        Ok(self.write(move |ctx| {
            let rows = tab_rows(ctx.tx)?;
            let Some(row) = rows.iter().find(|r| r.tab.id == id) else { return Ok(Err(Refused::NotFound { kind: "split tab", id })) };
            if matches!(row.tab.rule, SplitTabRule::Builtin { .. }) {
                return Ok(Err(Refused::Invalid { field: "id", words: "the default tabs cannot be removed: hide a tab instead".into() }));
            }
            ctx.tx.execute("DELETE FROM split_tab WHERE id = ?1", params![id])?;
            for (i, r) in rows.iter().filter(|r| r.tab.id != id).enumerate() {
                ctx.tx.execute("UPDATE split_tab SET sort_order = ?2 WHERE id = ?1", params![r.tab.id, i as i64])?;
            }
            Ok(Ok(()))
        })??)
    }

    /// Back to the default tabs exactly as the first launch wrote them (`seed::seed_split_tabs`: the names, the order, the rule, shown, time 0); every tab the owner
    /// added is gone. The built-in categories are pinned as the first launch pins them (every one but Social).
    pub fn split_tab_restore_defaults(&self) -> Result<(), Refused> {
        Ok(self.write(move |ctx| {
            ctx.tx.execute("DELETE FROM split_tab", [])?;
            crate::seed::seed_split_tabs(ctx.tx)?;
            ctx.tx.execute("UPDATE label SET pinned = (id != 'social') WHERE kind = 'builtin' AND pinned != (id != 'social')", [])?;
            Ok(())
        })?)
    }

    // ------------------------------------------------------------ the consent to send mail text to a provider

    /// When the owner allowed mail text to go to this provider; None = not given.
    pub fn agent_consent_at(&self, provider_id: &str) -> Result<Option<i64>, StoreError> {
        self.read(|c| Ok(c.prepare_cached("SELECT mail_consent_at_ms FROM ai_provider WHERE id = ?1")?.query_row(params![provider_id], |r| r.get::<_, Option<i64>>(0)).optional()?.flatten()))
    }

    /// Records the owner's "Allow" for `provider_id` (named `name` when its row does not exist yet: a row with no key, not connected). The first
    /// "Allow" keeps its time; a second one changes nothing.
    pub fn agent_consent_give(&self, provider_id: String, name: String, now_ms: i64) -> Result<(), StoreError> {
        self.write(move |ctx| {
            ctx.tx.execute("INSERT INTO ai_provider (id, name, connected, updated_at_ms, mail_consent_at_ms) VALUES (?1, ?2, 0, ?3, ?3) ON CONFLICT(id) DO UPDATE SET mail_consent_at_ms = coalesce(mail_consent_at_ms, ?3)", params![provider_id, name, now_ms])?;
            Ok(())
        })
    }

    /// The provider keeps its row but loses its key: not connected, no Keychain reference, no models (a provider whose consent was given keeps that). A speech
    /// engine on it falls back to this Mac, as `ai_provider_forget` does. Returns the Keychain reference it held.
    pub fn ai_provider_key_cleared(&self, id: String) -> Result<Option<String>, StoreError> {
        self.write(move |ctx| {
            let held: Option<Option<String>> = ctx.tx.query_row("SELECT keychain_ref FROM ai_provider WHERE id = ?1", params![id], |r| r.get(0)).optional()?;
            ctx.tx.execute("UPDATE speech_engine SET kind = 'local', cloud_provider_id = NULL, ready = 0 WHERE cloud_provider_id = ?1", params![id])?;
            ctx.tx.execute("UPDATE ai_provider SET connected = 0, keychain_ref = NULL, models_json = '[]' WHERE id = ?1", params![id])?;
            Ok(held.flatten())
        })
    }

    /// The providers that hold a given consent: (id, time).
    pub fn agent_consents(&self) -> Result<Vec<(String, i64)>, StoreError> {
        self.read(|c| {
            let mut st = c.prepare_cached("SELECT id, mail_consent_at_ms FROM ai_provider WHERE mail_consent_at_ms IS NOT NULL ORDER BY id")?;
            let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_slug_is_the_test_engines() {
        assert_eq!(tab_slug("Orders & Receipts"), "orders-receipts");
        assert_eq!(tab_slug("  !!  "), "tab");
        assert_eq!(tab_slug("Café 2"), "café-2");
    }

    #[test]
    fn buttons_round_trip() {
        let b = vec!["mark-read".to_string(), "reply".to_string()];
        assert_eq!(buttons_from(&buttons_json(&b)), b);
        assert_eq!(buttons_from("[]"), Vec::<String>::new());
        assert_eq!(buttons_from("junk"), Vec::<String>::new());
    }

    #[test]
    fn a_rule_round_trips_through_its_columns() {
        for r in [
            SplitTabRule::Sender { address: "a@b.com".into() }, SplitTabRule::Domain { domain: "b.com".into() }, SplitTabRule::Account { account_id: "x".into() },
            SplitTabRule::Label { label_id: "l".into() }, SplitTabRule::DecisionCategory { category: "c".into() }, SplitTabRule::Builtin { name: "Updates".into() },
        ] {
            let (k, v) = rule_columns(&r);
            assert_eq!(rule_from_columns(k, &v), r);
        }
    }
}
