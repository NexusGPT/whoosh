#!/usr/bin/env python3
"""Palette check: ix-palettemac (J1.9).

Drives the RUNNING rig copy over the control socket (docs/CONTROL-SOCKET.md) through journeys/lib; expected
values come from docs/design/fixtures/mac-main-sample.json and docs/CONSTANTS.md, never from a built screen.
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/palettemac-check.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
"""
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey                                  # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402
from lib.fixture import FX, ROWS, ACCOUNT, MSG, const               # noqa: E402

ID = "palettemac"
TITLE = "Command palette"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "every action is findable by name, shows its key, and Esc closes back to the inbox"
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
    s = j.step("J1.9", "Palette")
    s.arrive(("key", "⌘P"), "mac-palette")
    s.type("archive")
    s.expect("archive-found", lambda: s.ctl.rows("mac-palette.results")[0]["title"], "Archive")
    s.expect("archive-shows-its-key", lambda: s.ctl.rows("mac-palette.results")[0]["key"], "E")
    s.expect("move-probe-control", lambda: (s.key("Escape") or s.key("⌘P") or s.type("move") or True) and len(s.ctl.rows("mac-palette.results")) >= 1, True)
    s.expect("nonsense-finds-nothing", lambda: (s.key("Escape") or s.key("⌘P") or s.type("zzqxv") or True) and len(s.ctl.rows("mac-palette.results")), 0)
    s.key("Escape")
    s.expect("escape-returns-to-inbox", lambda: s.ctl.uistate()["surface"], "mac-main")
    s.expect("palette-keeps-focus", lambda: s.ctl.uistate()["focus_message"], FX["focused_message"])
    for key, action in (("H", "snooze"), ("L", "label"), ("⇧P", "pin")):
        s.key("⌘P")
        s.type(action)
        s.expect("key-hint-" + action, lambda a=action: s.ctl.rows("mac-palette.results")[0]["key"], key)
        s.key("Escape")


# MUTANTS: per check, the mutation of the app that must turn it red.
# J1.9/arrives-by-control | the palette key opens nothing
# J1.9/archive-found | fuzzy search misses an exact name
# J1.9/archive-shows-its-key | the key column is empty
# J1.9/move-probe-control | the results probe returns nothing for a real action
# J1.9/nonsense-finds-nothing | every query lists all actions
# J1.9/escape-returns-to-inbox | Esc leaves the palette open
# J1.9/palette-keeps-focus | closing the palette drops row focus
# J1.9/key-hint-snooze | the snooze hint shows another key
# J1.9/key-hint-label | the label hint shows another key
# J1.9/key-hint-pin | the pin hint shows another key


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
