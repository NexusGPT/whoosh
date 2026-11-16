#!/usr/bin/env python3
"""core-actions-check: drives the real `actions` crate (cargo test) and judges each feature of fn-actions and
fn-actions-contact-merge by NAME, not by a total. Standard library only.

    python3 checks/core-actions-check.py            run; prints `N checks, F failed, U not judged`; exit 1 when F > 0 or U > 0
    python3 checks/core-actions-check.py --selftest every check must go RED on its mutant (the source is edited, the check run, the source restored)
    --only id,id                                     limit either run to the named checks

docs/TESTING.md: a check is green only after it has been seen red on its mutant. The mutant table below is that proof's recipe;
the recorded runs are in _captures/core-actions/. Each check names the cargo tests that must pass; a name that matched no test
is NOT JUDGED (a renamed test must not read as green).
"""
import os
import re
import shutil
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CORE = os.path.join(ROOT, "core")
SRC = os.path.join(CORE, "crates", "actions", "src")
CARGO = shutil.which("cargo") or os.path.expanduser("~/.cargo/bin/cargo")

# check id -> (what the owner sees, [cargo test names that must all pass])
CHECKS = {
    "archive-commits-and-writes-back": ("archive: row leaves the inbox, server told, one undo entry", ["archive_commits_first_writes_back_and_pushes_one_undo_entry"]),
    "archive-refused-rolls-back": ("archive refused: the row returns, the line names the account", ["archive_refused_puts_the_row_back_and_names_the_account", "archive_of_two_accounts_keeps_the_accepted_one_and_returns_the_refused_one"]),
    "undo-restores-position": ("undo after archive restores folder and position", ["undo_after_archive_restores_folder_and_position"]),
    "snooze-words": ("snooze: tomorrow morning is 08:00 local, undo lifts it, refusal returns the row", ["snooze_hides_the_row_until_its_return_time_and_undo_lifts_it", "snooze_refused_returns_the_row_and_leaves_no_snooze", "tomorrow_morning_is_eight_local_the_next_day"]),
    "pin": ("pin: first in the list, undo restores, refusal leaves it unpinned", ["pin_puts_the_row_first_and_undo_restores_the_prior_state", "pin_refused_leaves_the_row_unpinned"]),
    "label-create-when-new": ("label: a new name is created, an existing one reused, a refusal removes both", ["label_creates_a_new_name_when_asked_and_reuses_it_after", "label_refused_removes_the_assignment_and_the_label_it_created"]),
    "move-target-refuses": ("move to account: the source is untouched when the target refuses", ["move_to_account_leaves_the_source_untouched_when_the_target_refuses", "move_to_account_moves_rows_and_undo_moves_them_back"]),
    "mark-read": ("mark read: written back, no undo entry, refusal marks unread again", ["mark_read_writes_back_and_no_undo_entry", "mark_read_refused_marks_them_unread_again"]),
    "undo-lifo": ("undo is last in, first out, UNDO_DEPTH deep, and says Nothing to undo when empty", ["undo_is_last_in_first_out", "undo_depth_is_the_ledger_value"]),
    "contact-merge-sum": ("merge two addresses: one contact id carrying both, the count is the sum", ["merging_two_addresses_yields_one_contact_carrying_both_and_the_counts_add_up", "merging_without_contact_rows_builds_them_from_the_headers_first"]),
    "contact-label": ("label a contact: every message from all its addresses", ["label_contact_labels_every_message_from_all_its_addresses"]),
    "contact-device-permission": ("device Contacts: read only after permission, the device name wins", ["without_the_owners_permission_the_device_contacts_are_not_read", "device_contacts_join_an_existing_contact_by_a_shared_address_and_their_name_wins"]),
}

# check id -> (source file under crates/actions/src, text to find, text to put there, why that is the defect)
MUTANTS = {
    "archive-refused-rolls-back": ("engine.rs", "revert(ctx, &failed)?;", "let _ = (&revert, &failed);", "rollback skipped on a refused writeback"),
    "archive-commits-and-writes-back": ("usecases/archive.rs", "set_flag(ctx, \"archived\", &changed_ids, archived)?;", "let _ = &changed_ids;", "the store step does nothing"),
    "undo-restores-position": ("usecases/undo.rs", "archive::apply(ctx, message_ids, false, false)?", "archive::apply(ctx, message_ids, true, false)?", "undo archives again instead of restoring"),
    "undo-lifo": ("undo.rs", "ORDER BY at_ms DESC, rowid DESC LIMIT 1", "ORDER BY at_ms ASC, rowid ASC LIMIT 1", "undo takes the oldest entry first"),
    "snooze-words": ("constants.rs", "(day + 1) * DAY_MS + self.tomorrow_minute * MINUTE_MS - off", "(day + 1) * DAY_MS + self.tomorrow_minute * MINUTE_MS", "tomorrow morning ignores the local zone"),
    "pin": ("usecases/pin.rs", "ctx.set_pinned_at(message_id, pinned.then_some(now))?;", "ctx.set_pinned_at(message_id, None)?;", "pin never pins"),
    "label-create-when-new": ("usecases/label.rs", "if found.is_none() && !create {", "if found.is_none() && create {", "create flag inverted"),
    "move-target-refuses": ("usecases/move_to_account.rs", "ctx.restore_account_move(&moved, ids)?;", "let _ = (&moved, ids);", "the source row is not put back"),
    "mark-read": ("usecases/mark_read.rs", "undo: None,", "undo: Some(contracts::UndoAction::Archive { message_ids: changed_ids.clone() }),", "mark read pushes an undo entry"),
    "contact-merge-sum": ("usecases/contacts_support.rs", "ctx.contact_save(&keep.id, &keep.name, &addresses, now)?;", "ctx.contact_save(&keep.id, &keep.name, &keep.addresses, now)?;", "the merged contact keeps one address only, so the count covers one address"),
    "contact-label": ("usecases/label_contact.rs", ".execute(ids, label_name, create_if_missing)", ".execute(ids.into_iter().take(1).collect(), label_name, create_if_missing)", "only one message is labelled"),
    "contact-device-permission": ("usecases/build_contacts.rs", "let Ok(people) = directory.execute() else { return Ok(None) };", "let people = directory.execute().unwrap_or_default();", "a denied permission reads as an empty list"),
}


def run_tests(extra=()):
    """Run the crate's tests; return ({name: 'ok'|'FAILED'}, exit code, raw output)."""
    p = subprocess.run([CARGO, "test", "--offline", "-j", "2", "-p", "actions", "--no-fail-fast", *extra, "--", "--test-threads", "4"], cwd=CORE, capture_output=True, text=True)
    out = p.stdout + p.stderr
    seen = {}
    for m in re.finditer(r"^test (\S+) \.\.\. (ok|FAILED)", out, re.M):
        seen[m.group(1)] = m.group(2)
    return seen, p.returncode, out


def judge(ids, seen):
    """-> [(id, verdict, detail)] with verdict in ok / FAILED / NOT JUDGED."""
    rows = []
    for cid in ids:
        _, names = CHECKS[cid]
        missing = [n for n in names if n not in seen]
        bad = [n for n in names if seen.get(n) == "FAILED"]
        if missing:
            rows.append((cid, "NOT JUDGED", "no such test: " + ", ".join(missing)))
        elif bad:
            rows.append((cid, "FAILED", ", ".join(bad)))
        else:
            rows.append((cid, "ok", "%d test(s)" % len(names)))
    return rows


def report(rows):
    for cid, verdict, detail in rows:
        print("%-12s %-34s %s" % (verdict, cid, detail))
    f = sum(1 for r in rows if r[1] == "FAILED")
    u = sum(1 for r in rows if r[1] == "NOT JUDGED")
    print("%d checks, %d failed, %d not judged" % (len(rows), f, u))
    return 1 if f or u else 0


def run_group(ids):
    seen, code, out = run_tests()
    if not seen:
        print(out[-2000:])
        return [(cid, "NOT JUDGED", "cargo test produced no test lines (exit %d)" % code) for cid in ids]
    return judge(ids, seen)


def selftest(ids):
    """Each check must pass on the real source and go red, on its own tests, when the mutant is applied."""
    rows = []
    base = judge(ids, run_tests()[0])
    for cid, verdict, detail in base:
        if verdict != "ok":
            rows.append((cid, "FAILED", "not green before the mutant: " + detail))
    if rows:
        return rows
    for cid in ids:
        if cid not in MUTANTS:
            rows.append((cid, "FAILED", "no mutant declared"))
            continue
        rel, old, new, why = MUTANTS[cid]
        path = os.path.join(SRC, rel)
        original = open(path).read()
        if original.count(old) != 1:
            rows.append((cid, "FAILED", "mutant anchor found %d times in %s" % (original.count(old), rel)))
            continue
        try:
            open(path, "w").write(original.replace(old, new))
            seen, code, out = run_tests()
            if not seen:
                rows.append((cid, "FAILED", "the mutant does not compile: " + out[-600:].replace("\n", " ")))
                continue
            verdict = judge([cid], seen)[0]
            if verdict[1] == "FAILED":
                rows.append((cid, "ok", "red on its mutant (%s): %s" % (why, verdict[2])))
            else:
                rows.append((cid, "FAILED", "stayed %s on its mutant (%s)" % (verdict[1], why)))
        finally:
            open(path, "w").write(original)
    clean = subprocess.run(["git", "-C", ROOT, "diff", "--quiet", "--", "core/crates/actions/src"]).returncode == 0
    if not clean:
        rows.append(("restore", "FAILED", "the source differs from HEAD after the mutants; look at git diff"))
    return rows


def main():
    ids = list(CHECKS)
    if "--only" in sys.argv:
        ids = [i for i in sys.argv[sys.argv.index("--only") + 1].split(",") if i]
    if "--selftest" in sys.argv:
        return report(selftest(ids))
    return report(run_group(ids))


if __name__ == "__main__":
    sys.exit(main())
