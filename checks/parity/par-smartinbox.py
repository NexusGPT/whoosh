#!/usr/bin/env python3
"""par-smartinbox: the four named features of the todo, each with one core-level check.

    smart inbox by AI with the user's provider · reminder if no reply · notifications limited to Primary · VIP rules

Each cell runs one test of core/crates/smart-inbox/tests/par_smartinbox.rs on fixtures/smart-inbox.json and reads its verdict.
The app-visible half (what the owner sees on screen) is NOT judged here: no surface is locked-on-build, so no driven check can run
against a running app; docs/lanes/core-smart-inbox.md names that gap.
    python3 checks/parity/par-smartinbox.py            run the cells
    python3 checks/parity/par-smartinbox.py --mutants  also apply each mutant, demand the named cell goes red
Prints the cells, the MUTANTS table, then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
"""
import importlib.util
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("core_smart_inbox_check", os.path.join(os.path.dirname(HERE), "core-smart-inbox-check.py"))
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)
SRC = base.SRC

CELLS = [
    ("ai-provider", "par-smartinbox", "smart inbox by AI: the user's provider (Jev or the local model) places mail, decided_by names it, only the allowed fields are sent, the engine off sends nothing", ["feature_smart_inbox_by_ai_with_the_users_provider"]),
    ("reminder", "par-smartinbox", "reminder if no reply: an owner-sent message still last in its thread after the wait is due; a reminder already set or a reply removes it", ["feature_reminder_if_no_reply"]),
    ("notify-primary", "par-smartinbox", "notifications limited to Primary: only Primary, VIPs and Needs Reply ring; list mail and quiet accounts stay silent; the switch off lets social mail ring", ["feature_notifications_limited_to_primary"]),
    ("vip-rules", "par-smartinbox", "VIP rules: a VIP address or domain lands in VIPs ahead of every other entry", ["feature_vip_rules", "marking_and_unmarking_a_vip_moves_that_senders_mail"]),
]

MUTANTS = [
    ("local-provider-named-jev", "ai-provider", [(SRC + "place.rs", "DecisionProviderKind::Local => DecidedBy::Laya,", "DecisionProviderKind::Local => DecidedBy::Jev,")], "the local model's placements are credited to Jev"),
    ("reminder-ignores-reminders-set", "reminder", [(SRC + "remind.rs", "AND NOT EXISTS (SELECT 1 FROM reminder r WHERE r.message_id = m.id) ", "")], "a message that already has a reminder is listed again"),
    ("notify-limit-ignored", "notify-primary", [(SRC + "notify.rs", "!policy.limit_to_primary || placement.tab_id == tabs.primary()", "true")], "Updates, Promotions and Social ring although the limit is on"),
    ("vip-domain-ignored", "vip-rules", [(SRC + "load.rs", " || vips.domains.iter().any(|d| crate::cond::domain_matches(&domain, d))", "")], "a VIP domain no longer places mail in VIPs"),
]

if __name__ == "__main__":
    sys.exit(base.main(CELLS, MUTANTS))
