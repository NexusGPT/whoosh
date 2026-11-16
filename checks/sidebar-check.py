#!/usr/bin/env python3
"""Sidebar check: ix-sidebar (docs/JOURNEYS.md J1.1, J1.5).

Drives the RUNNING rig copy over the control socket (docs/CONTROL-SOCKET.md) through journeys/lib; expected
values come from docs/design/fixtures/mac-main-sample.json and docs/CONSTANTS.md, never from a built screen.
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/sidebar-check.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
"""
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey                                  # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402
from lib.fixture import FX, ROWS, ACCOUNT, MSG, const               # noqa: E402

ID = "sidebar"
TITLE = "Sidebar lists accounts, mailboxes, labels and smart folders"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "the sidebar shows every account with its count, the mailboxes in order, nested labels and smart folders"
LIST = "mac-main.list"


def ids(s):
    return [r["id"] for r in s.ctl.rows(LIST)]


def accepted(s, *args):
    """True when the app answers ok; ok:false is False so the check after it turns red (dry run: not judged)."""
    ctl = s.ctl
    try:
        ctl.send(*args)
        return True
    except Unreadable:
        return False


BUILT_IN_LABELS = [t["name"] for t in FX["tabs"][1:] if t["name"] != "VIPs"]   # Needs Reply, Updates, Promotions, Social: Primary is a VIEW since contract delta 16, not a label (docs/lanes/contracts.md "## Delta 16"); VIPs is a mailbox


def drive(j):
    s = j.step("J1.1", "Sidebar content")
    s.arrive(("click", "dock.whoosh"), "mac-main")
    accts = lambda: s.ctl.rows("mac-main.sidebar.accounts")
    s.expect("account-count", lambda: len(accts()), len(FX["accounts"]))
    s.expect("account-ids-in-order", lambda: [r["id"] for r in accts()], [a["id"] for a in FX["accounts"]])
    s.expect("account-unread-per-row", lambda: [r["unread"] for r in accts()], [a["unread"] for a in FX["accounts"]])
    s.expect("failing-account-marked", lambda: [r["id"] for r in accts() if r.get("status") == "sign in again"], [a["id"] for a in FX["accounts"] if a.get("status")])
    s.expect("mailboxes-in-order", lambda: [r["name"] for r in s.ctl.rows("mac-main.sidebar.mailboxes")], [m["name"] for m in FX["mailboxes"]])
    s.expect("mailbox-counts", lambda: [r.get("count") for r in s.ctl.rows("mac-main.sidebar.mailboxes")], [m.get("count") for m in FX["mailboxes"]])
    s.expect("top-level-labels", lambda: [r["name"] for r in s.ctl.rows("mac-main.sidebar.labels") if r.get("depth") == 0], BUILT_IN_LABELS + [l["name"] for l in FX["labels"]])   # labels trio: the core lists the built-in labels at depth 0, in tab order, before the owner's
    s.expect("nested-label-depth", lambda: [r["depth"] for r in s.ctl.rows("mac-main.sidebar.labels") if r["name"] == "Nordlicht"], [2])
    s.expect("smart-folders", lambda: [r["name"] for r in s.ctl.rows("mac-main.sidebar.smart-folders")], [f["name"] for f in FX["smart_folders"]])
    s.click("mac-main.accounts.%s" % FX["accounts"][3]["id"])
    s.expect("account-click-filters-list", lambda: {r["account"] for r in s.ctl.rows(LIST) if not r["pinned"]}, {FX["accounts"][3]["id"]})
    s.click("mac-main.accounts.all")
    s.expect("all-accounts-restores-list", lambda: len(s.ctl.rows(LIST)), len(ROWS))


# MUTANTS: per check, the mutation of the app that must turn it red.
# J1.1/arrives-by-control | the Dock launch opens another surface
# J1.1/account-count | the sidebar lists 29 accounts or only the first page
# J1.1/account-ids-in-order | accounts are sorted by name instead of the fixture order
# J1.1/account-unread-per-row | unread counts are summed into one number
# J1.1/failing-account-marked | the failing account looks healthy
# J1.1/mailboxes-in-order | Sent comes before Snoozed
# J1.1/mailbox-counts | mailbox counts are all zero
# J1.1/top-level-labels | labels are flattened
# J1.1/nested-label-depth | nested labels render at depth 0
# J1.1/smart-folders | smart folders are missing
# J1.1/account-click-filters-list | an account click leaves the list unfiltered
# J1.1/all-accounts-restores-list | All accounts leaves a filter on


def main():
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    rows = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# J") and " | " in l]
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant |")
    print("|---|---|")
    for k, v in rows:
        print("| %s | %s |" % (k, v))
    for line in r["fail_list"]:
        sys.stderr.write("FAIL %s\n" % line)
    for line in r["no_mutant"]:
        sys.stderr.write("NO MUTANT %s\n" % line)
    for line in r["stray_mutant"]:
        sys.stderr.write("STRAY MUTANT %s\n" % line)
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    ok = r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
