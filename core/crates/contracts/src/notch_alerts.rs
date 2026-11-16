//! The notch's alerts (owner's picks of 14 Nov 2026, docs/surfaces/mac-notch.md: "1B 2A 3A"): how a new mail shows in the notch, by the KIND of the mail.
//! The rule lives once, here: the kind of a mail (`notch_kind_of`), the style a kind takes (`notch_alert_style`), the start values (`notch_alerts_default`,
//! the picked set 1B "By weight") and the words both engines serve at the `notch_alerts/<field>` state paths (`notch_alerts_state`). The real engine and the
//! test engine call these functions, so neither holds a second copy of the rule.
//!
//! State paths (one JSON value each): `notch_alerts/vip | needs_reply | primary | updates | promotions | sign_in_codes` -> "nothing" | "light" | "line" | "card" |
//! "opened"; `notch_alerts/stay` -> "5s" | "10s" | "until-acted"; `notch_alerts/sound` and `notch_alerts/full_mail_loads_pictures` -> true | false.
use crate::constants::{NOTCH_DROP_HOLD_S, NOTCH_STAY_SHORT_S};

/// How a new mail shows in the notch, from the least of the screen to the most: nothing · the lower edge lights in the account's colour · one line (who and the
/// subject) · the card (two lines and the buttons) · the mail opened.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotchAlertStyle { Nothing, Light, Line, Card, Opened }

/// The kind of an arriving mail, for the notch. `Promotions` holds Social too (one line in Settings › Notifications). An owner's own label is `Primary`.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotchKind { SignInCode, Vip, NeedsReply, Updates, Promotions, Primary }

/// How long an alert stays before the notch shrinks by itself.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotchStay { FiveSeconds, TenSeconds, UntilActed }

/// The alert settings, saved whole (`Command::SetNotchAlerts`) and read whole (`WhooshCore::notch_alerts`).
#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct NotchAlerts {
    pub vip: NotchAlertStyle,
    pub needs_reply: NotchAlertStyle,
    pub primary: NotchAlertStyle,
    pub updates: NotchAlertStyle,
    pub promotions: NotchAlertStyle,
    pub sign_in_codes: NotchAlertStyle,
    pub stay: NotchStay,
    pub sound: bool,
    /// The notch's full mail loads its pictures at once (owner, 14 Nov 2026: "Ok. Full email load pictures by default"); tracking pixels stay blocked either way.
    pub full_mail_loads_pictures: bool,
}

/// What an engine knows of a stored mail that decides its kind.
#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct NotchFacts {
    pub has_sign_in_code: bool,
    pub vip_sender: bool,
    /// The id of the mail's category tab ("needs-reply", "updates", "promotions", "social", "primary", "vips", or an owner's tab); None while it is not sorted.
    pub tab_id: Option<String>,
    /// The mail carries a mailing-system header (a list): until the sorting has placed it, such a mail counts as Updates.
    pub has_list_header: bool,
}

/// The start values: the owner's pick 1B "By weight" (VIPs card · needs reply card · primary one line · updates light · promotions and social nothing ·
/// sign-in codes card), an alert stays `NOTCH_DROP_HOLD_S`, no sound, the full mail loads its pictures.
#[uniffi::export]
pub fn notch_alerts_default() -> NotchAlerts {
    NotchAlerts {
        vip: NotchAlertStyle::Card, needs_reply: NotchAlertStyle::Card, primary: NotchAlertStyle::Line, updates: NotchAlertStyle::Light,
        promotions: NotchAlertStyle::Nothing, sign_in_codes: NotchAlertStyle::Card, stay: NotchStay::TenSeconds, sound: false, full_mail_loads_pictures: true,
    }
}

/// The kind of a mail. The order is the rule: a sign-in code first, then a VIP sender, then the mail's category; a mail not sorted yet is Updates when it
/// carries a mailing-system header, else Primary.
#[uniffi::export]
pub fn notch_kind_of(facts: NotchFacts) -> NotchKind {
    if facts.has_sign_in_code { return NotchKind::SignInCode; }
    if facts.vip_sender { return NotchKind::Vip; }
    match facts.tab_id.as_deref() {
        Some("vips") => NotchKind::Vip,
        Some("needs-reply") => NotchKind::NeedsReply,
        Some("updates") => NotchKind::Updates,
        Some("promotions") | Some("social") => NotchKind::Promotions,
        Some(_) => NotchKind::Primary,
        None => if facts.has_list_header { NotchKind::Updates } else { NotchKind::Primary },
    }
}

/// The style the settings give a kind.
#[uniffi::export]
pub fn notch_alert_style(alerts: NotchAlerts, kind: NotchKind) -> NotchAlertStyle {
    match kind {
        NotchKind::SignInCode => alerts.sign_in_codes,
        NotchKind::Vip => alerts.vip,
        NotchKind::NeedsReply => alerts.needs_reply,
        NotchKind::Updates => alerts.updates,
        NotchKind::Promotions => alerts.promotions,
        NotchKind::Primary => alerts.primary,
    }
}

/// The one spelling of a style (the store's column value and the state path's word).
#[uniffi::export]
pub fn notch_alert_style_word(style: NotchAlertStyle) -> String {
    match style { NotchAlertStyle::Nothing => "nothing", NotchAlertStyle::Light => "light", NotchAlertStyle::Line => "line", NotchAlertStyle::Card => "card", NotchAlertStyle::Opened => "opened" }.to_string()
}

#[uniffi::export]
pub fn notch_alert_style_from_word(word: String) -> Option<NotchAlertStyle> {
    match word.as_str() {
        "nothing" => Some(NotchAlertStyle::Nothing), "light" => Some(NotchAlertStyle::Light), "line" => Some(NotchAlertStyle::Line),
        "card" => Some(NotchAlertStyle::Card), "opened" => Some(NotchAlertStyle::Opened), _ => None,
    }
}

/// The one spelling of a stay.
#[uniffi::export]
pub fn notch_stay_word(stay: NotchStay) -> String {
    match stay { NotchStay::FiveSeconds => "5s", NotchStay::TenSeconds => "10s", NotchStay::UntilActed => "until-acted" }.to_string()
}

#[uniffi::export]
pub fn notch_stay_from_word(word: String) -> Option<NotchStay> {
    match word.as_str() { "5s" => Some(NotchStay::FiveSeconds), "10s" => Some(NotchStay::TenSeconds), "until-acted" => Some(NotchStay::UntilActed), _ => None }
}

/// The seconds an alert stays (`NOTCH_STAY_SHORT_S`, `NOTCH_DROP_HOLD_S`); None for "until I act": the notch then never shrinks by itself.
#[uniffi::export]
pub fn notch_stay_seconds(stay: NotchStay) -> Option<u32> {
    match stay { NotchStay::FiveSeconds => Some(NOTCH_STAY_SHORT_S as u32), NotchStay::TenSeconds => Some(NOTCH_DROP_HOLD_S as u32), NotchStay::UntilActed => None }
}

/// The word of a kind (the test door's reads and the Settings rows' ids).
#[uniffi::export]
pub fn notch_kind_word(kind: NotchKind) -> String {
    match kind {
        NotchKind::SignInCode => "sign_in_codes", NotchKind::Vip => "vip", NotchKind::NeedsReply => "needs_reply",
        NotchKind::Updates => "updates", NotchKind::Promotions => "promotions", NotchKind::Primary => "primary",
    }.to_string()
}

/// The settings with one kind's style changed: the Settings rows save through this, so the six fields are named in one place.
#[uniffi::export]
pub fn notch_alerts_with(alerts: NotchAlerts, kind: NotchKind, style: NotchAlertStyle) -> NotchAlerts {
    let mut a = alerts;
    match kind {
        NotchKind::SignInCode => a.sign_in_codes = style,
        NotchKind::Vip => a.vip = style,
        NotchKind::NeedsReply => a.needs_reply = style,
        NotchKind::Updates => a.updates = style,
        NotchKind::Promotions => a.promotions = style,
        NotchKind::Primary => a.primary = style,
    }
    a
}

/// One state value of the alert settings (`notch_alerts/<field>`), as JSON; None for a field that does not exist.
#[uniffi::export]
pub fn notch_alerts_state(alerts: NotchAlerts, field: String) -> Option<String> {
    let word = |s: NotchAlertStyle| Some(format!("\"{}\"", notch_alert_style_word(s)));
    match field.as_str() {
        "vip" => word(alerts.vip),
        "needs_reply" => word(alerts.needs_reply),
        "primary" => word(alerts.primary),
        "updates" => word(alerts.updates),
        "promotions" => word(alerts.promotions),
        "sign_in_codes" => word(alerts.sign_in_codes),
        "stay" => Some(format!("\"{}\"", notch_stay_word(alerts.stay))),
        "sound" => Some(alerts.sound.to_string()),
        "full_mail_loads_pictures" => Some(alerts.full_mail_loads_pictures.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(code: bool, vip: bool, tab: Option<&str>, list: bool) -> NotchFacts {
        NotchFacts { has_sign_in_code: code, vip_sender: vip, tab_id: tab.map(String::from), has_list_header: list }
    }

    /// Mutants seen red on this test: K1 the code check moved under the VIP check; K2 `Some("social")` dropped from the Promotions arm;
    /// K3 the unsorted arm answers Primary whatever the header; K4 `Some("vips")` dropped.
    #[test]
    fn the_kind_follows_the_order_code_vip_category_then_the_header() {
        assert_eq!(notch_kind_of(facts(true, true, Some("promotions"), true)), NotchKind::SignInCode);
        assert_eq!(notch_kind_of(facts(false, true, Some("promotions"), true)), NotchKind::Vip);
        assert_eq!(notch_kind_of(facts(false, false, Some("vips"), false)), NotchKind::Vip);
        assert_eq!(notch_kind_of(facts(false, false, Some("needs-reply"), true)), NotchKind::NeedsReply);
        assert_eq!(notch_kind_of(facts(false, false, Some("updates"), false)), NotchKind::Updates);
        assert_eq!(notch_kind_of(facts(false, false, Some("promotions"), false)), NotchKind::Promotions);
        assert_eq!(notch_kind_of(facts(false, false, Some("social"), false)), NotchKind::Promotions);
        assert_eq!(notch_kind_of(facts(false, false, Some("primary"), true)), NotchKind::Primary);
        assert_eq!(notch_kind_of(facts(false, false, Some("clients"), false)), NotchKind::Primary);
        assert_eq!(notch_kind_of(facts(false, false, None, true)), NotchKind::Updates);
        assert_eq!(notch_kind_of(facts(false, false, None, false)), NotchKind::Primary);
    }

    /// Every default tab of the ledger (docs/CONSTANTS.md SPLIT_TABS_DEFAULT) has a kind of its own choosing here: a tab renamed or added in the ledger
    /// fails this until the rule names it. Mutant K5: the expected table loses a row.
    #[test]
    fn every_default_tab_of_the_ledger_has_its_kind() {
        let ledger = include_str!("../../../../docs/CONSTANTS.md");
        let names: Vec<String> = ledger.lines().find(|l| l.trim_start().starts_with("| SPLIT_TABS_DEFAULT |")).and_then(|l| l.split('|').nth(2))
            .map(|c| c.split(',').map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).collect()).unwrap_or_default();
        let want = [("primary", NotchKind::Primary), ("needs-reply", NotchKind::NeedsReply), ("updates", NotchKind::Updates),
                    ("promotions", NotchKind::Promotions), ("social", NotchKind::Promotions), ("vips", NotchKind::Vip)];
        assert_eq!(names.len(), want.len(), "the ledger lists {names:?}");
        for name in names {
            let id = crate::builtin_label_id(name.clone());
            let kind = want.iter().find(|(w, _)| *w == id).map(|(_, k)| *k).unwrap_or_else(|| panic!("the default tab {id} has no kind in this test"));
            assert_eq!(notch_kind_of(facts(false, false, Some(&id), false)), kind, "tab {id}");
        }
    }

    /// The start values are the owner's pick 1B. Mutant K6: `primary` starts as Card.
    #[test]
    fn the_start_values_are_the_picked_set_by_weight() {
        let a = notch_alerts_default();
        let got: Vec<String> = [NotchKind::Vip, NotchKind::NeedsReply, NotchKind::Primary, NotchKind::Updates, NotchKind::Promotions, NotchKind::SignInCode]
            .into_iter().map(|k| notch_alert_style_word(notch_alert_style(a.clone(), k))).collect();
        assert_eq!(got, ["card", "card", "line", "light", "nothing", "card"]);
        assert_eq!((notch_stay_word(a.stay).as_str(), a.sound, a.full_mail_loads_pictures), ("10s", false, true));
        assert_eq!(notch_stay_seconds(NotchStay::FiveSeconds), Some(5));
        assert_eq!(notch_stay_seconds(NotchStay::TenSeconds), Some(10));
        assert_eq!(notch_stay_seconds(NotchStay::UntilActed), None);
    }

    /// Every word reads back to its value, a changed kind changes that kind only, and the state serves each field. Mutants: K7 `notch_alerts_with` writes
    /// `updates` for Promotions; K8 the state's "stay" serves the style word of `vip`.
    #[test]
    fn words_round_trip_and_one_kind_changes_alone() {
        for s in [NotchAlertStyle::Nothing, NotchAlertStyle::Light, NotchAlertStyle::Line, NotchAlertStyle::Card, NotchAlertStyle::Opened] {
            assert_eq!(notch_alert_style_from_word(notch_alert_style_word(s)), Some(s));
        }
        for s in [NotchStay::FiveSeconds, NotchStay::TenSeconds, NotchStay::UntilActed] { assert_eq!(notch_stay_from_word(notch_stay_word(s)), Some(s)); }
        assert_eq!(notch_alert_style_from_word("banner".into()), None);
        let base = notch_alerts_default();
        for k in [NotchKind::Vip, NotchKind::NeedsReply, NotchKind::Primary, NotchKind::Updates, NotchKind::Promotions, NotchKind::SignInCode] {
            let changed = notch_alerts_with(base.clone(), k, NotchAlertStyle::Opened);
            for other in [NotchKind::Vip, NotchKind::NeedsReply, NotchKind::Primary, NotchKind::Updates, NotchKind::Promotions, NotchKind::SignInCode] {
                let want = if other == k { NotchAlertStyle::Opened } else { notch_alert_style(base.clone(), other) };
                assert_eq!(notch_alert_style(changed.clone(), other), want, "changed {k:?}, read {other:?}");
                assert_eq!(notch_alerts_state(changed.clone(), notch_kind_word(other)), Some(format!("\"{}\"", notch_alert_style_word(want))));
            }
        }
        assert_eq!(notch_alerts_state(base.clone(), "stay".into()).as_deref(), Some("\"10s\""));
        assert_eq!(notch_alerts_state(base.clone(), "sound".into()).as_deref(), Some("false"));
        assert_eq!(notch_alerts_state(base.clone(), "full_mail_loads_pictures".into()).as_deref(), Some("true"));
        assert_eq!(notch_alerts_state(base, "colour".into()), None);
    }
}
