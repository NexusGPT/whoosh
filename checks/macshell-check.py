#!/usr/bin/env python3
"""Mac shell check: ix-macshell (J1.1; the J4.8 link line).

Drives the RUNNING rig copy over the control socket (docs/CONTROL-SOCKET.md) through journeys/lib; expected
values come from docs/design/fixtures/mac-main-sample.json and docs/CONSTANTS.md, never from a built screen.
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/macshell-check.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
"""
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey                                  # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402
from lib.fixture import FX, ROWS, ACCOUNT, MSG, const               # noqa: E402

ID = "macshell"
TITLE = "Mac window shell"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "one window of the designed size, never smaller than the minimum, showing the Unified Inbox"
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
    w = FX["window"]
    s = j.step("J1.1", "Window and regions")
    s.arrive(("click", "dock.whoosh"), "mac-main")
    s.expect("one-window", lambda: len([f for f in s.ctl.winframes() if f.get("kind", "window") == "window"]), 1)   # the menu-bar status item is a frame of kind status-item (appshell: smoke leaves it out)
    s.expect("design-width", lambda: abs(s.ctl.winframes()[0]["w"] - w["w"]) <= const("DESIGN_TOLERANCE_PX"), True)
    s.expect("design-height", lambda: abs(s.ctl.winframes()[0]["h"] - w["h"]) <= const("DESIGN_TOLERANCE_PX"), True)
    s.expect("min-size-width", lambda: s.ctl.winframes()[0]["w"] >= w["min_w"], True)
    s.expect("min-size-height", lambda: s.ctl.winframes()[0]["h"] >= w["min_h"], True)
    s.expect("sidebar-region-exists", lambda: len(s.ctl.rows("mac-main.sidebar.accounts")) > 0, True)
    s.expect("list-region-exists", lambda: len(s.ctl.rows(LIST)) > 0, True)
    s.expect("failing-account-link-to-settings", lambda: s.ctl.rows("mac-main.sync.line")[0]["link"], "mac-settings-accounts")
    s.expect("sync-line-text", lambda: s.ctl.rows("mac-main.sync.line")[0]["text"], FX["states"]["sync"]["failing_line"])


# MUTANTS: per check, the mutation of the app that must turn it red.
# J1.1/arrives-by-control | the Dock launch opens another surface
# J1.1/one-window | a second window opens (a status-item frame does not count; a window of kind window does)
# J1.1/design-width | the window opens at another width
# J1.1/design-height | the window opens at another height
# J1.1/min-size-width | the window can shrink below the minimum width
# J1.1/min-size-height | the window can shrink below the minimum height
# J1.1/sidebar-region-exists | the sidebar is not drawn
# J1.1/list-region-exists | the list is not drawn
# J1.1/failing-account-link-to-settings | the line has no link to Settings: Accounts
# J1.1/sync-line-text | the line does not name the account


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
