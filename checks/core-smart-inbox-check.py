#!/usr/bin/env python3
"""Smart inbox core check: fn-smart-inbox-rules, fn-split-tabs (docs/plan.json lane core-smart-inbox).

Runs `cargo test -p smart-inbox` on fixtures/smart-inbox.json and reads each named test's verdict. The expected values live in
the fixture (labelled by hand) and in docs/CONSTANTS.md (CATEGORY_RULE_MIN), never in this file.
    python3 checks/core-smart-inbox-check.py            run the cells
    python3 checks/core-smart-inbox-check.py --mutants  also apply each mutant below, run the tests, and demand the named cell goes red
    ... --mutants --only id,id  only those mutants
Prints one line per cell, the MUTANTS table, then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
Needs only cargo (offline, -j 2). A mutant is applied to a source file, the tests run, and the file is restored byte for byte.
"""
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CORE = os.path.join(ROOT, "core")
SRC = "core/crates/smart-inbox/src/"
CARGO = os.path.expanduser("~/.cargo/bin/cargo")

# cell id -> the test names that must pass (todo, what the cell asserts)
CELLS = [
    ("rules-meet-bar", "fn-smart-inbox-rules", "rule placement matches the labels at or above CATEGORY_RULE_MIN, and misses only the model-only messages", ["rules_alone_meet_the_ledger_bar_and_miss_exactly_the_model_cases"]),
    ("model-completes", "fn-smart-inbox-rules", "with the model entry every label matches; an unsure model lets the list continue", ["with_the_model_every_label_matches", "a_model_answer_below_its_trust_level_lets_the_list_continue"]),
    ("decided-by", "fn-split-tabs", "every message_category row has a non-empty decided_by naming the entry or the model", ["every_placement_names_who_decided"]),
    ("list-order", "fn-smart-inbox-rules", "first match wins; the model entry sits where the owner puts it; the default closes the list", ["the_model_entry_sits_where_the_owner_puts_it", "the_engine_off_skips_every_model_entry_and_the_list_falls_through"]),
    ("counts", "fn-smart-inbox-rules", "per-tab and per-account counts equal the hand-labelled ones and the store's SELECT", ["per_account_and_tab_counts_equal_the_hand_labelled_ones", "tab_counts_equal_the_hand_labelled_ones_and_the_stores_select"]),
    ("waiting", "fn-smart-inbox-rules", "waiting = inbound threads whose last message is unanswered", ["waiting_is_the_inbound_threads_whose_last_message_is_unanswered", "the_counts_follow_a_new_reply"]),
    ("defaults", "fn-split-tabs", "the six default tabs at first launch", ["the_six_defaults_exist_at_first_launch_in_ledger_order"]),
    ("rename", "fn-split-tabs", "renaming a tab changes no placement", ["renaming_a_tab_changes_no_placement", "renaming_before_the_mail_arrives_changes_no_placement"]),
    ("add-domain-tab", "fn-split-tabs", "a tab with rule domain = linea-supply-example.com moves exactly that domain's fixture messages", ["adding_a_domain_tab_moves_exactly_that_domains_mail", "a_domain_tab_that_exists_before_the_mail_arrives_takes_exactly_that_domains_mail"]),
    ("remove-tab", "fn-split-tabs", "removing a tab returns its messages to Primary", ["removing_a_tab_returns_its_messages_to_primary", "disabling_and_enabling_a_tab_moves_mail_out_and_back", "a_split_inbox_keeps_at_least_one_tab_and_a_refused_edit_changes_nothing"]),
    ("label-tree", "fn-split-tabs", "a rule by label path matches that label and every descendant, never an ancestor or a sibling branch; decided_by records the exact node", ["a_label_rule_matches_the_label_and_every_descendant_and_records_the_exact_node", "a_label_rule_on_the_root_takes_the_whole_tree_and_a_leaf_rule_only_the_leaf"]),
    ("tab-kinds", "fn-split-tabs", "rules by sender, domain, account, label, decision category and the owner's own rules; reorder moves no mail; the owner's move survives", ["a_tab_by_sender_account_label_or_decision_category", "a_decision_category_tab_collects_what_the_model_names", "an_owner_rule_goes_first_and_names_itself", "reordering_tabs_moves_no_mail", "the_owners_own_move_survives_every_re_sort"]),
]

# (id, cell, [(file, old text, new text), ...], why it must turn the cell red). Each `old` is replaced everywhere it occurs; a
# mutant with several edits breaks every layer that guards the same rule.
MUTANTS = [
    ("decided-by-not-written", "decided-by", [(SRC + "place_new.rs", "self.store.write(move |ctx| write_placements(ctx, &to_write, now_ms).map(|_| ()))?;", "let _ = to_write;")], "arrival leaves the store's provisional 'default' on every row"),
    ("list-header-rule-dropped", "rules-meet-bar", [(SRC + "list.rs", 'rule("which-tab/list-header", vec![Cond::ListHeader], builtin("Updates")),', "")], "newsletters fall to Primary: rule placement drops under CATEGORY_RULE_MIN"),
    ("last-match-wins", "list-order", [(SRC + "place.rs", "for (i, e) in self.which.entries.iter().enumerate() {", "for (i, e) in self.which.entries.iter().enumerate().rev() {")], "the default (last entry) always wins"),
    ("trust-level-ignored", "model-completes", [(SRC + "place.rs", "if v.probability >= *trust_from {", "if true {")], "an unsure model answer places the message"),
    ("rename-reads-the-name", "rename", [(SRC + "tabs.rs", "t.enabled && matches!(&t.rule, SplitTabRule::Builtin { name } if name.eq_ignore_ascii_case(key))", "t.enabled && t.name.eq_ignore_ascii_case(key)")], "placement looks tabs up by display name, so a rename moves mail"),
    ("domain-matches-everything", "add-domain-tab", [(SRC + "cond.rs", '!w.is_empty() && (s == w || s.ends_with(&format!(".{w}")))', "!w.is_empty()")], "a domain tab takes mail from every domain"),
    ("removed-tab-keeps-its-mail", "remove-tab", [(SRC + "resort.rs", 'if (by == "rule" && rule_id.as_deref().is_some_and(is_custom_entry_id)) || !c.tabs.is_enabled(tab) =>', "if false =>")], "mail stays on the removed tab"),
    ("owner-move-overwritten", "tab-kinds", [(SRC + "load.rs", "WHERE (message_category.decided_by <> 'owner' OR excluded.decided_by = 'owner') \\", "WHERE (1 = 1) \\"), (SRC + "resort.rs", 'is_some_and(|(_, by, _)| by == "owner")', "is_some_and(|(_, by, _)| by == \"never\")")], "a re-sort undoes the owner's own move (both guards removed: the write and the re-sort check)"),
    ("label-exact-match-only", "label-tree", [(SRC + "cond.rs", "f.label_chains.iter().any(|chain| chain.iter().any(|n| n == label_id))", "f.label_chains.iter().any(|chain| chain.last() == Some(label_id))"), (SRC + "resort.rs", "SELECT DISTINCT message_id FROM label_assignment WHERE label_id IN (SELECT id FROM sub)", "SELECT DISTINCT message_id FROM label_assignment WHERE label_id = (SELECT id FROM sub LIMIT 1)")], "a label rule matches only a message that carries exactly that label (the match and the candidate query both)"),
    ("waiting-ignores-newer-messages", "waiting", [(SRC + "counts.rs", "n.thread_id = m.thread_id \\", "n.thread_id = m.thread_id AND 1 = 0 \\")], "every inbound message counts as waiting, answered or not"),
]


def cargo_test(extra=()):
    """Run the crate's tests; return ({test: 'ok'|'FAILED'}, build_failed, raw output)."""
    p = subprocess.run([CARGO, "test", "--offline", "-j", "2", "-p", "smart-inbox", "--no-fail-fast", *extra, "--", "--test-threads", "2"], cwd=CORE, capture_output=True, text=True)
    out = p.stdout + p.stderr
    results = {m.group(1): m.group(2) for m in re.finditer(r"^test (\S+) \.\.\. (ok|FAILED)", out, re.M)}
    return results, (not results and p.returncode != 0), out


def why_failed(out, name):
    """The first lines of the panic message of one failed test (the real words, not a bare status)."""
    m = re.search(r"---- %s stdout ----\n(.*?)(?:\n\n|\Z)" % re.escape(name), out, re.S)
    return " | ".join(l.strip() for l in (m.group(1).splitlines() if m else [])[:4])


def judge(results, tests):
    missing = [t for t in tests if t not in results]
    failed = [t for t in tests if results.get(t) == "FAILED"]
    return missing, failed


def run_cells(cells, label=""):
    """Returns (checks, failed, not_judged, results)."""
    results, build_failed, out = cargo_test()
    if any(v == "FAILED" for v in results.values()) or build_failed:
        print("raw cargo output kept in _captures/core-smart-inbox/last-red.txt")
        open(os.path.join(ROOT, "_captures", "core-smart-inbox", "last-red.txt"), "w").write(out)
    n = f = u = 0
    for cid, todo, what, tests in cells:
        n += 1
        if build_failed:
            u += 1
            print("NOT JUDGED  %-16s %s (the crate did not build)" % (cid, todo))
            continue
        missing, failed = judge(results, tests)
        if missing:
            u += 1
            print("NOT JUDGED  %-16s %s: no verdict for %s" % (cid, todo, ", ".join(missing)))
        elif failed:
            f += 1
            print("RED         %-16s %s: %s -- failing: %s" % (cid, todo, what, ", ".join(failed)))
            for t in failed:
                print("            %s: %s" % (t, why_failed(out, t)))
        else:
            print("green       %-16s %s: %s" % (cid, todo, what))
    return n, f, u, results


def run_mutants(mutants, cells):
    """Apply each mutant, run the tests, demand the named cell is red, restore the file byte for byte."""
    bad = 0
    tests_of = {c[0]: c[3] for c in cells}
    for mid, cell, edits, why in mutants:
        originals = {}
        for rel, old, new in edits:
            originals.setdefault(rel, open(os.path.join(ROOT, rel), "rb").read())
        missing_text = [rel for rel, old, new in edits if old not in originals[rel].decode()]
        if missing_text:
            print("MUTANT %-30s CANNOT APPLY: text not found in %s" % (mid, ", ".join(missing_text)))
            bad += 1
            continue
        try:
            texts = {rel: o.decode() for rel, o in originals.items()}
            for rel, old, new in edits:
                texts[rel] = texts[rel].replace(old, new)
            for rel, t in texts.items():
                open(os.path.join(ROOT, rel), "wb").write(t.encode())
            results, build_failed, out = cargo_test()
        finally:
            for rel, o in originals.items():
                open(os.path.join(ROOT, rel), "wb").write(o)
        assert all(open(os.path.join(ROOT, rel), "rb").read() == o for rel, o in originals.items())
        if build_failed:
            print("MUTANT %-30s DID NOT BUILD (a mutant must compile)" % mid)
            bad += 1
            continue
        missing, failed = judge(results, tests_of[cell])
        if failed:
            print("MUTANT %-30s red in %s: %s  [%s]" % (mid, cell, ", ".join(failed), why))
        else:
            print("MUTANT %-30s SURVIVED: cell %s stayed green  [%s]" % (mid, cell, why))
            bad += 1
    return bad


def main(cells=CELLS, mutants=MUTANTS):
    n, f, u, _ = run_cells(cells)
    if "--mutants" in sys.argv:
        only = sys.argv[sys.argv.index("--only") + 1].split(",") if "--only" in sys.argv else None
        print("MUTANTS (each applied alone, tests run, file restored):")
        f += run_mutants([m for m in mutants if only is None or m[0] in only], cells)
    print("%d checks, %d failed, %d not judged" % (n, f, u))
    return 1 if (f or u) else 0


if __name__ == "__main__":
    sys.exit(main())
