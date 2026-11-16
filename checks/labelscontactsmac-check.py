#!/usr/bin/env python3
"""labelscontactsmac functional check: smart-folder nesting and rule editing, and the sender card, over the control socket.

    python3 checks/labelscontactsmac-check.py [--dry]     exit 0 only when every check passed and none was not judged
    python3 checks/labelscontactsmac-check.py --selftest   (no app: the dry structure and the helper on stand-ins)

Covers the lane's acceptance (nesting a smart folder updates the sidebar; a sender card lists mail from every
account newest first) plus JOURNEYS J2.6 and J2.7 failure paths (a rule matching nothing shows 0 matches before
saving; one sender with two addresses is one card, merged by the owner). Written from docs/JOURNEYS.md,
docs/CONTROL-SOCKET.md and fixtures/tests-j2.json. Accessibility ids not yet in the socket doc are ASSUMPTIONS
(docs/lanes/tests-j2.md). Step ids reuse the J2 step they cover.
"""
import json
import os
import re
import sys
import types

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey  # noqa: E402

CORPUS = json.load(open(os.path.join(ROOT, "fixtures", "tests-j2.json"), encoding="utf-8"))
EXP = CORPUS["expect"]
NAME = EXP["smart_folder_name"]
SIDE = "mac-labels-contacts.sidebar"
MATCHES = "mac-labels-contacts.rule.matches"
EDIT = "mac-search.sidebar.smart-folders.supplier-quotes.edit"
N = len(EXP["smart_folder_threads_after_save"])
CORPUS_PATH = os.path.join(ROOT, "fixtures", "tests-j2.json")


def check(s, name, got_fn, want, ctx=None):
    """`expect` where a failure line carries the values that decide it (`ctx`, read when the check fails)."""
    def run():
        got = got_fn()
        s._eg = (want, got)
        ok = got == want
        return ok, "expected %r, got %r" % (want, got) + ("" if ok or ctx is None else " | " + ctx())
    s._record(name, run)


def drive(j):
    row = lambda s: [r for r in s.ctl.rows(SIDE) if r["title"] == NAME]

    s = j.step("J2.6", "Nest and edit a smart folder")
    # The corpus the expectations are computed from must be the corpus the app searches (journeys/J2.py:178): without this the app serves its
    # default corpus, which has none of tests-j2.json's ids, its second Linea account or its second address.
    s.raw("fixture", "corpus", CORPUS_PATH)
    s.key("/")                                          # setup: search, save the folder
    # The folder is saved from J2's exact search, the one whose answer is known (msg-01). The words "from Linea Supply before June has attachment" make no
    # filters in the Mac field (SPEC A142, A143: only the colon forms are pills), so a folder saved from them searched for every one of those words.
    s.type(EXP["exact_query"])
    s.key("Enter")                                      # Save needs a LAUNCHED search (SearchMacSave.swift startSaving: `guard launched`); without Enter no folder is saved and no Edit door exists
    s.click("mac-search.save-as-smart-folder")
    s.type(NAME)
    s.key("Enter")
    s.arrive(("click", EDIT), "mac-labels-contacts")
    # The parent is a label, as journeys/J2.py does it: "Work" when the tree has one, else the tree's first top-level label (the tests-j2 corpus holds only the
    # built-in ones); the editor lists the top-level labels as chips `parent.<slug>` and the sidebar row publishes the label's NAME.
    pick = []
    s._act(lambda: pick.append(next((r["name"] for r in s.ctl.rows("mac-labels-contacts.labels") if r["name"] == EXP["smart_folder_parent"]),
                                    next((r["name"] for r in s.ctl.rows("mac-labels-contacts.labels") if r.get("depth") == 1), None))))
    parent = pick[0] if pick and pick[0] else EXP["smart_folder_parent"]
    s._act(lambda: s.ctl.click("mac-labels-contacts.parent.%s" % "-".join(w for w in re.split(r"[^\w]+", parent.lower()) if w)))
    # item 3bb3d817 (lane every-word): a match holds EVERY typed word, so the words "and has attachment" (held by no mail) made the rule match nothing and the
    # preview below could never be above zero; the colon form is the attachment filter, and the figure's mails with a file are 2 (msg-01, msg-s02:
    # _captures/every-word/match-sets-8.out)
    s.type(" has:attachment")
    preview, editor = [], []                            # the number the editor shows before the save, and its whole row (the parent it holds)
    s._act(lambda: (editor.append(dict(s.ctl.rows(MATCHES)[0])), preview.append(editor[-1]["matches"])))
    s.key("Enter")
    check(s, "rule-edited", lambda: (s.ctl.core("smart_folder/%s/rule" % NAME) or "").endswith(" has:attachment"), True,
          lambda: "core smart_folder/%s/rule is %r; sidebar rows %r" % (NAME, s.ctl.core("smart_folder/%s/rule" % NAME), [(r["title"], r.get("count")) for r in s.ctl.rows(SIDE)]))
    saved = lambda s: [r["count"] for r in row(s)]
    # the preview and the saved folder are one computation: what the editor said before the save is what the sidebar counts after it, and it is not zero
    check(s, "rule-preview-counts-matches", lambda: bool(preview) and bool(saved(s)) and preview[0] == saved(s)[0] and preview[0] > 0, True,
          lambda: "the editor said %r before the save, the sidebar counts %r after it" % (preview, saved(s)))
    check(s, "nesting-shows-in-sidebar", lambda: [r["parent"] for r in row(s)], [parent],
          lambda: "the editor held %r before the save; labels on the pane %r; core parent %r; sidebar rows %r" % (
              editor, [(r.get("id"), r.get("name")) for r in s.ctl.rows("mac-labels-contacts.labels")],
              [s.ctl.core("smart_folder/%s/parent" % r["id"]) for r in row(s)], [(r["id"], r["title"], r["parent"]) for r in s.ctl.rows(SIDE)]))
    # the sidebar's count is the core's own count of the saved rule
    check(s, "count-updates", lambda: saved(s) == [s.ctl.core("smart_folder/%s/count" % r["id"]) for r in row(s)] and bool(saved(s)), True,
          lambda: "the sidebar counts %r, the core counts %r" % (saved(s), [s.ctl.core("smart_folder/%s/count" % r["id"]) for r in row(s)]))
    kept = []
    s._act(lambda: kept.extend(saved(s)))
    s.click(EDIT)
    s.type(" from:nobody-at-all-example")               # the colon form is a filter: no sender is called that
    s.expect("rule-matching-nothing-shows-zero-before-save", lambda: s.ctl.rows(MATCHES)[0]["matches"], 0)   # before any save
    s.key("Escape")
    s.expect("unsaved-edit-leaves-the-folder-alone", lambda: saved(s), kept)

    s = j.step("J2.7", "The sender card")
    s.key("Enter")                                      # Esc left the editor for the results; Enter opens the first result's thread
    s.arrive(("click", "mac-main.reader.sender"), "mac-labels-contacts")
    s.expect("sender-messages-newest-first", lambda: [r["id"] for r in s.ctl.rows("mac-labels-contacts.contact.messages")], EXP["sender_messages_newest_first"])
    s.expect("card-spans-accounts", lambda: sorted({r["account"] for r in s.ctl.rows("mac-labels-contacts.contact.messages")}), sorted(set(EXP["sender_accounts"])))
    s.expect("sender-is-linea", lambda: s.ctl.rows("mac-labels-contacts.contact.header")[0]["address"], EXP["sender_address"])
    s.click("mac-labels-contacts.contact.merge")
    s.expect("merged-card-carries-both-addresses", lambda: s.ctl.rows("mac-labels-contacts.contact.header")[0]["addresses"],
             [EXP["sender_address"], EXP["sender_second_address"]])


MUTANTS = """# MUTANTS
# J2.6/arrives-by-control | Edit opens nothing
# J2.6/rule-preview-counts-matches | the preview always says 0 matches
# J2.6/rule-edited | the rule edit is discarded
# J2.6/nesting-shows-in-sidebar | the sidebar keeps the folder at the top level
# J2.6/count-updates | the folder count does not follow the saved rule
# J2.6/rule-matching-nothing-shows-zero-before-save | a rule matching nothing shows no number before saving
# J2.6/unsaved-edit-leaves-the-folder-alone | Esc in the editor saves the half-typed rule
# J2.7/arrives-by-control | the sender name is not clickable
# J2.7/sender-messages-newest-first | the list is oldest first
# J2.7/sender-is-linea | the contact page shows another sender
# J2.7/card-spans-accounts | the card lists only the account of the open thread
# J2.7/merged-card-carries-both-addresses | merging drops the second address
"""


def selftest():
    """No app: the dry structure (every check has a mutant, none is stray) and the failure line's values on a stand-in. Exit 0 when all hold."""
    bad = []

    def report(ok, what):
        print(("ok   " if ok else "FAIL ") + what)
        if not ok:
            bad.append(what)

    mod = types.SimpleNamespace(ID="labelscontactsmac-check", TITLE="selftest", TARGET="LOCAL", VISIBLE_OUTCOME="", drive=drive)
    path = os.path.join(ROOT, "_captures", ".tmp", "labelscontactsmac-mutants.txt")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    open(path, "w", encoding="utf-8").write(MUTANTS)
    r = run_journey(mod, path, force_dry=True)
    report(r["failed"] == 0 and not r["no_mutant"] and not r["stray_mutant"] and r["checks"] > 0,
           "dry run: %d checks, %d failed, no check without a mutant, no stray mutant" % (r["checks"], r["failed"]))
    seen = {}
    rec = types.SimpleNamespace(_eg=None, _record=lambda name, fn: seen.__setitem__(name, fn()))
    check(rec, "same", lambda: True, True, lambda: "ctx")
    check(rec, "other", lambda: False, True, lambda: "rule is 'x'")
    report(seen["same"] == (True, "expected True, got True") and seen["other"] == (False, "expected True, got False | rule is 'x'"),
           "check passes equal values and a failure carries the deciding values")
    ids = {m["id"] for m in CORPUS["messages"]}
    report(set(EXP["sender_messages_newest_first"]) <= ids and "msg-s02" in ids,
           "the expected ids are in the corpus the check stages (%s), not in the default corpus" % os.path.basename(CORPUS_PATH))
    print("labelscontactsmac selftest: %d problems" % len(bad))
    return 1 if bad else 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    mod = types.SimpleNamespace(ID="labelscontactsmac-check", TITLE="Labels, smart folders, contacts (Mac)", TARGET="LOCAL", VISIBLE_OUTCOME="", drive=drive)
    path = os.path.join(ROOT, "_captures", ".tmp", "labelscontactsmac-mutants.txt")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    open(path, "w", encoding="utf-8").write(MUTANTS)
    r = run_journey(mod, path, force_dry="--dry" in argv)
    for line in r["fail_list"]:
        sys.stderr.write("FAIL %s\n" % line)
    for lab in r["no_mutant"]:
        sys.stderr.write("NO MUTANT %s\n" % lab)
    for lab in r["stray_mutant"]:
        sys.stderr.write("STRAY MUTANT %s\n" % lab)
    sys.stderr.write(r["verdict"] + ": " + r["evidence"] + "\n")
    sys.stdout.write(MUTANTS)
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    ok = r["verdict"] == "GREEN" and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
