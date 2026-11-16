#!/usr/bin/env python3
"""Trainer check: ix-trainer (J1.10, J1.11); stop-after from docs/CONSTANTS.md TRAINER_HINT_STOP_AFTER.

Drives the RUNNING rig copy over the control socket (docs/CONTROL-SOCKET.md) through journeys/lib; expected
values come from docs/design/fixtures/mac-main-sample.json and docs/CONSTANTS.md, never from a built screen.
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/trainer-check.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
"""
import os
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey                                  # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402
from lib.fixture import PRESET_ANCHORS, PRESET_KEY_ROWS, FX, ROWS, ACCOUNT, MSG, const               # noqa: E402

ID = "trainer"
TITLE = "Shortcut trainer"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "the hint appears after a mouse action, the overlay lists every key and marks the learned ones"
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


def wait_until(read, ok, secs=3.0):
    """Poll `read` every 100 ms until ok(value) or `secs` pass; returns the last value (a bounded wait, never a sleep-and-hope)."""
    end = time.time() + secs
    v = read()
    while not ok(v) and time.time() < end:
        time.sleep(0.1)
        v = read()
    return v


def hint_text(s):
    """The text of the one `mac-trainer.hint` row (the app serves `rows: [{text, key, action, ...}]`), polled because the
    hint is drawn after the click returns. No row after the wait is a plain value the check compares, never an IndexError."""
    rows = wait_until(lambda: s.ctl.rows("mac-trainer.hint"), bool)
    if not rows:
        return "<no mac-trainer.hint row served; %s>" % ("; ".join(s.j.notes) or "the click was accepted")
    return rows[0].get("text", "<hint row without text>")


def drive(j):
    s = j.step("J1.10", "Hint after the mouse")
    # The hints switch is a preference of the rig identity (whoosh.trainer.hints in io.nexus.whoosh.rig), and the rig copy keeps its
    # preferences between runs: main48 read it OFF (`defaults read io.nexus.whoosh.rig whoosh.trainer.hints` = 0), so no hint was ever served.
    # The check sets the state it needs, then proves it (a precondition that is not on makes the next check red for the real reason).
    s.click("mac-trainer.hints.on")
    s.expect("hints-switch-is-on", lambda: s.ctl.rows("mac-trainer.settings")[0]["hints"], "on")
    s.expect("no-hint-before-any-mouse-action", lambda: len(s.ctl.rows("mac-trainer.hint")), 0)
    if j.ctl is not None:                       # the list is filled after launch: a click on a row that is not there yet is refused
        wait_until(lambda: len(ids(s)), lambda n: n > 4)
    s.click("mac-main.list.row.4.archive")
    s.expect("hint-text", lambda: hint_text(s), FX["states"]["trainer_hint"])
    for n in range(const("TRAINER_HINT_STOP_AFTER")):
        s.key("E")
    s.click("mac-main.list.row.4.archive")
    s.expect("hint-gone-after-stop-count", lambda: len(s.ctl.rows("mac-trainer.hint")), 0)
    s = j.step("J1.11", "Overlay")
    s.key("⌘P")                      # ⇧? is a second key of the search in the Superhuman preset (shortcuts lane); the key list opens from the palette (the "Keys" entry)
    s.type("keys")
    s.arrive(("key", "Enter"), "mac-trainer")
    s.expect("every-key-with-its-action", lambda: (len(s.ctl.rows("mac-trainer.overlay.keys")), {r["key"]: r["action"] for r in s.ctl.rows("mac-trainer.overlay.keys") if r["key"] in PRESET_ANCHORS}), (len(FX["shortcuts"]), PRESET_ANCHORS))   # the overlay lists every key of the preset: the fixture map holds 111 since ⌘J (write a reply draft) joined it (owner approved AI replies with ⌘J in a thread, docs/JOURNEYS.md J10; Commands.swift:59); journeys/lib/fixture.py PRESET_KEY_ROWS still says 110
    s.expect("learned-key-marked", lambda: [r["used"] for r in s.ctl.rows("mac-trainer.overlay.keys") if r["key"] == "E"], [True])
    s.expect("never-pressed-key-unmarked", lambda: [r["used"] for r in s.ctl.rows("mac-trainer.overlay.keys") if r["key"] == "X"], [False])   # X (select conversation) is never pressed here
    s.key("Escape")
    s.expect("overlay-closes", lambda: s.ctl.uistate()["overlay"], None)


# MUTANTS: per check, the mutation of the app that must turn it red.
# J1.10/hints-switch-is-on | the hints-on door does nothing, or the switch stays off
# J1.10/no-hint-before-any-mouse-action | a hint shows at launch
# J1.10/hint-text | the hint names the wrong key
# J1.10/hint-gone-after-stop-count | the hint never stops
# J1.11/arrives-by-control | the show-keys key opens nothing
# J1.11/every-key-with-its-action | the overlay lists fewer keys or wrong actions
# J1.11/learned-key-marked | no key is marked used
# J1.11/never-pressed-key-unmarked | every key is marked used
# J1.11/overlay-closes | Esc leaves the overlay open


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
