#!/usr/bin/env python3
"""Parity: iPhone inbox, customisable swipe buttons and pull to refresh (plan todo par-inboxphone).

Drives the RUNNING iPhone app over the control socket ($WHOOSH_SOCKET_DIR, docs/CONTROL-SOCKET.md) through journeys/lib,
the same harness as checks/inboxphone-check.py. Expected values come from the fixture and the board defaults
(swipe right archives, swipe left snoozes: docs/design/boards/iphone-layout-3/mail-26), never from a built screen.
Run: sh scripts/launch-one.sh <iphone-bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/par-inboxphone.py'
Self-test: python3 checks/parity/par-inboxphone.py --selftest   (the stand-in socket of checks/inboxphone-check.py: every case passes on it and goes RED on its mutant)
Prints a line per case, then the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
"""
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, print_cases                                 # noqa: E402
from lib.fixture import ROWS                                                    # noqa: E402

ID = "par-inboxphone"
TITLE = "iPhone inbox: customisable swipe buttons, pull to refresh"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "the owner picks what each swipe does and the rows follow the pick; a pull reads the mail again and keeps the rows on screen"
FAILING = "acc-18"


def why_not_driven():
    d = os.environ.get("WHOOSH_SOCKET_DIR", "")
    if d and os.path.exists(os.path.join(d, "whoosh.sock")):
        return ""
    try:
        out = subprocess.run(["xcrun", "simctl", "list", "runtimes"], capture_output=True, text=True, timeout=20).stdout
    except (OSError, subprocess.SubprocessError):
        out = ""
    if "iOS" not in out:
        return "the iPhone simulator cannot run: Xcode reports iOS 26.5 is not installed (owner step: Xcode > Settings > Components > iOS 26.5)"
    return "no iPhone app socket: launch the iPhone app in the simulator, then set WHOOSH_SOCKET_DIR"


def drive(j):
    if j.ctl is None and why_not_driven():
        j.notes.append("could-not-drive: " + why_not_driven())
    L = "iphone-inbox.list"
    ids = lambda: [r["id"] for r in s.ctl.rows(L)]
    setting = lambda k: s.ctl.rows("iphone-inbox.swipe-settings")[0][k]
    s = j.step("par-inboxphone-swipe", "Customisable swipe buttons")
    s.expect("default-leading-archives", lambda: setting("leading"), "archive")
    s.expect("default-trailing-snoozes", lambda: setting("trailing"), "snooze")
    s.click("iphone-inbox.swipe-settings.leading.pin")
    s.expect("pick-is-kept", lambda: setting("leading"), "pin")
    s.expect("rows-follow-the-pick", lambda: {r["swipe_leading"] for r in s.ctl.rows(L)}, {"pin"})
    k = [i for i, m in enumerate(ROWS) if not m["pinned"] and m["account"] != FAILING][0]
    target = ROWS[k]["id"]
    s.click("iphone-inbox.list.row.%d.swipe-leading" % k)
    s.expect("leading-swipe-pins", lambda: s.ctl.core("pin/%s" % target), True)
    s.expect("pinned-row-stays", lambda: target in ids(), True)
    s.click("iphone-inbox.swipe-settings.leading.archive")
    s.expect("pick-restored", lambda: setting("leading"), "archive")
    r = j.step("par-inboxphone-refresh", "Pull to refresh")
    before = {}
    r.expect("rows-before-refresh", lambda: before.setdefault("ids", ids()) is not None, True)
    r.expect("stamp-before", lambda: before.setdefault("t", s.ctl.rows("iphone-inbox.refresh")[0]["refreshed_at"]) is not None, True)
    r.click("iphone-inbox.list.refresh")
    r.expect("refresh-reads-again", lambda: s.ctl.rows("iphone-inbox.refresh")[0]["refreshed_at"] > before["t"], True)
    r.expect("refresh-keeps-the-rows", lambda: ids() == before["ids"], True)


# MUTANTS: per check, the mutation of the app that must turn it red.
# par-inboxphone-swipe/default-leading-archives | swipe right does something other than archive by default
# par-inboxphone-swipe/default-trailing-snoozes | swipe left does something other than snooze by default
# par-inboxphone-swipe/pick-is-kept | the leading pick is not stored
# par-inboxphone-swipe/rows-follow-the-pick | the rows keep the default swipe after a new pick
# par-inboxphone-swipe/leading-swipe-pins | the leading swipe still archives after picking Pin
# par-inboxphone-swipe/pinned-row-stays | a pin removes the row
# par-inboxphone-swipe/pick-restored | the pick cannot be set back to Archive
# par-inboxphone-refresh/rows-before-refresh | the list region is not registered
# par-inboxphone-refresh/stamp-before | the refresh region is not registered
# par-inboxphone-refresh/refresh-reads-again | pull to refresh does nothing
# par-inboxphone-refresh/refresh-keeps-the-rows | a refresh blanks or reorders the list


def selftest(path):
    """The clean stand-in is green on every case and each declared mutant turns its own case red (the model lives in checks/inboxphone-check.py)."""
    import importlib.util
    spec = importlib.util.spec_from_file_location("inboxphone_check", os.path.join(ROOT, "checks", "inboxphone-check.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod.selftest_run(path, drive_fn=drive, jid=ID, prefix="par-inboxphone")


def main():
    path = os.path.abspath(__file__)
    if "--selftest" in sys.argv:
        return selftest(path)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    rows = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# par-inboxphone") and " | " in l]
    print_cases(r)
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
    if r["unjudged"] and r["notes"]:
        sys.stderr.write("%s\n" % r["notes"][0])
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    ok = r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
