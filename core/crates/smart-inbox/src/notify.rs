//! Which arrivals ring the Mac and the iPhone (docs/DECISION-MODEL.md "Notify me"): the account must be on; a VIP rings;
//! a message in the Needs Reply tab rings; a list mail stays silent; the rest rings only when it sits in Primary while
//! "notifications limited to Primary" (GotEmail 0.9.7, research/gotmail.md) is on.
use crate::facts::MessageFacts;
use crate::place::Placement;
use crate::tabs::TabSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotifyPolicy {
    pub limit_to_primary: bool,
}

pub fn should_notify(account_enabled: bool, policy: &NotifyPolicy, f: &MessageFacts, placement: &Placement, tabs: &TabSet) -> bool {
    if !account_enabled {
        return false;
    }
    if f.from_vip {
        return true;
    }
    if tabs.builtin("Needs Reply").is_some_and(|t| t.id == placement.tab_id) {
        return true;
    }
    if f.has_list_header {
        return false;
    }
    !policy.limit_to_primary || placement.tab_id == tabs.primary()
}
