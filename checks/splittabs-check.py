#!/usr/bin/env python3
"""Split tabs check: fn-split-tabs, ix-settings-splittabs (J1.13), as reshaped by list round 5.

Drives the RUNNING rig copy over the control socket (docs/CONTROL-SOCKET.md) through journeys/lib; expected
values come from docs/design/fixtures/mac-main-sample.json and docs/CONSTANTS.md, never from a built screen.
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/splittabs-check.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
"""
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey                                  # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402
from lib.fixture import FX, ROWS, ACCOUNT, MSG, ALL_VIEW, const               # noqa: E402

ID = "splittabs"
TITLE = "Split tabs editing"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "the tab row is the pinned labels (list round 5), the old editor is gone, and unpin and pin reach the label record"
NOT_TABS = ("Social", "VIPs")
TABS = [FX["tabs"][0]["name"], ALL_VIEW["name"]] + [t["name"] for t in FX["tabs"][1:] if t["name"] not in NOT_TABS] + ["Provanta"]  # delta 16: the views Primary and All first, then the built-in labels and the pinned user label of the fixture core
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


def drive(j):
    # List round 5: the split editor, rename and add-with-rule are gone; the tab row is the pinned labels.
    # The only edits left are unpin (the tab's menu) and pin (the label record); names come from the labels.
    s = j.step("J1.13", "Split tabs after round 5: pinned labels, no editor")
    s.expect("tabs-are-the-built-ins-and-pinned", lambda: [r["title"] for r in s.ctl.rows("mac-main.tabs")], TABS)
    s.expect("social-and-vips-are-not-tabs", lambda: [n for n in NOT_TABS if n in [r["title"] for r in s.ctl.rows("mac-main.tabs")]], [])
    s.expect("edit-control-is-retired", lambda: accepted(s, "click", "mac-main.tabs.edit"), False)
    s.expect("rename-control-is-retired", lambda: accepted(s, "click", "mac-main.tabs.edit.rename.needs-reply"), False)
    ids = []
    s._act(lambda: ids.extend(r["id"] for r in s.j.ctl.rows("mac-main.splits")))
    last = lambda: ids[-1]
    s.click("mac-main.splits.unpin.%s" % (ids[-1] if ids else "none"))
    s.expect("unpin-leaves-the-row", lambda: [r["id"] for r in s.ctl.rows("mac-main.splits")], ids[:-1] if ids else None)
    s.expect("unpin-writes-the-label", lambda: s.ctl.core("label/%s/pinned" % last()), False)
    s.click("mac-main.splits.pin.%s" % (ids[-1] if ids else "none"))
    s.expect("pin-restores-the-row", lambda: [r["id"] for r in s.ctl.rows("mac-main.splits")], ids if ids else None)
    s.expect("pin-writes-the-label", lambda: s.ctl.core("label/%s/pinned" % last()), True)


# MUTANTS: per check, the mutation of the app that must turn it red.
# J1.13/tabs-are-the-built-ins-and-pinned | a tab is missing, renamed or out of order
# J1.13/social-and-vips-are-not-tabs | Social or VIPs comes back as a tab
# J1.13/edit-control-is-retired | the old edit control answers again
# J1.13/rename-control-is-retired | the old rename control answers again
# J1.13/unpin-leaves-the-row | unpin hides nothing
# J1.13/unpin-writes-the-label | the unpin lives in the view only
# J1.13/pin-restores-the-row | pin does not bring the tab back
# J1.13/pin-writes-the-label | the pin lives in the view only


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
