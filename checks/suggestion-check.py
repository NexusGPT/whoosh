#!/usr/bin/env python3
"""Suggestion check: ix-suggestions (PRD "smart suggestions after an action"; DATA-MAP row suggestion).

After junk the core offers ONE follow-up, the undo bar's place shows it with ONE key, and a suggestion never acts alone:
accepting it (its key or its click) writes the block, dismissing it writes nothing, a letter that is not the offered one does nothing.
Drives the RUNNING rig copy (docs/CONTROL-SOCKET.md); the expected values come from docs/DATA-MAP.md and the contract, never from a screen.
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/suggestion-check.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
"""
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey                                  # noqa: E402

ID = "suggestion"
TITLE = "Smart suggestions after an action"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "junk offers one follow-up with one key; its key writes the block; nothing is written until then"
BAR = "mac-main.undo.suggestion"
UNDO_AX = "mac-main.undo.suggestion.undo"            # the bar's Undo button; absent when the action has no undo entry


def _on_screen(s, ax):
    """True when something on screen carries `ax` (measure answers a frame); False when measure refuses (nothing carries it)."""
    try:
        return bool(s.ctl.measure(ax).get("frame"))
    except Exception:                                                # noqa: BLE001  (ControlFailure: no measurement at ax)
        return False


def st_list(s):
    return s.ctl.rows("mac-main.list.state")[0]


def drive(j):
    s = j.step("par-suggestion-offer", "Offer after junk")
    s.arrive(("click", "dock.whoosh"), "mac-main")
    s.expect("none-offered-before-any-action", lambda: s.ctl.rows(BAR), [])
    s.key("⇧!")
    s.expect("one-follow-up-only", lambda: len(s.ctl.rows(BAR)), 1)
    s.expect("kind-is-block-sender", lambda: s.ctl.rows(BAR)[0]["kind"], "blockSender")
    s.expect("key-is-one-letter", lambda: len(s.ctl.rows(BAR)[0]["key"]), 1)
    s.expect("key-is-B", lambda: s.ctl.rows(BAR)[0]["key"], "B")
    # Junk has no undo entry in the core yet (TriageKeys.junk says so, undoable: false): the bar must not offer Undo Z beside those words.
    s.expect("no-undo-offered-after-junk", lambda: (_on_screen(s, UNDO_AX), st_list(s)["undo_available"]), (False, False))
    s.expect("core-holds-the-suggestion", lambda: s.ctl.core("suggestion/count"), 1)
    addr = lambda: s.ctl.rows(BAR)[0]["subject"]
    s.expect("not-blocked-before-accept", lambda: s.ctl.core("block/%s/blocked" % addr()), False)

    s = j.step("par-suggestion-wrong-key", "A letter that is not offered does nothing")
    s.expect("wrong-letter-not-taken", lambda: _press_then(s, "U", lambda: s.ctl.core("block/%s/blocked" % addr())), False)
    s.expect("still-offered-after-wrong-letter", lambda: s.ctl.core("suggestion/count"), 1)

    s = j.step("par-suggestion-accept", "Accept writes the block")
    offered = {}
    s.expect("remember-subject", lambda: offered.setdefault("a", addr()) is not None, True)
    s.key("B")
    s.expect("blocked-after-accept", lambda: s.ctl.core("block/%s/blocked" % offered["a"]), True)
    s.expect("suggestion-gone-after-accept", lambda: s.ctl.rows(BAR), [])
    s.expect("core-count-zero-after-accept", lambda: s.ctl.core("suggestion/count"), 0)

    s = j.step("par-suggestion-dismiss", "Dismiss writes nothing")
    s.key("⇧!")
    s.expect("second-offer", lambda: len(s.ctl.rows(BAR)), 1)
    second = {}
    s.expect("remember-second", lambda: second.setdefault("a", addr()) is not None, True)
    s.click(BAR + ".dismiss")
    s.expect("dismissed-gone", lambda: s.ctl.rows(BAR), [])
    s.expect("dismiss-wrote-no-block", lambda: s.ctl.core("block/%s/blocked" % second["a"]), False)


def _press_then(s, chord, read):
    """A key the app declines answers ok:false; that is the expected outcome here, so it is swallowed and the state is read."""
    from lib.ctl import Unreadable
    try:
        s.ctl.key(chord)
    except Unreadable:
        pass
    return read()


# MUTANTS: per check, the mutation of the app that must turn it red.
# par-suggestion-offer/arrives-by-control | the Dock launch opens another surface
# par-suggestion-offer/none-offered-before-any-action | the bar row source always lists a sample suggestion
# par-suggestion-offer/one-follow-up-only | the bar row source lists every suggestion, not the newest
# par-suggestion-offer/kind-is-block-sender | the bar row reports the wrong kind
# par-suggestion-offer/key-is-one-letter | the key is a two-key chord
# par-suggestion-offer/key-is-B | the block-sender key is another letter
# par-suggestion-offer/core-holds-the-suggestion | the row source invents a suggestion the core never offered
# par-suggestion-offer/not-blocked-before-accept | offering the suggestion already writes the block
# par-suggestion-wrong-key/wrong-letter-not-taken | any letter accepts the suggestion
# par-suggestion-wrong-key/still-offered-after-wrong-letter | a wrong letter dismisses the suggestion
# par-suggestion-accept/remember-subject | the bar row reports no subject address
# par-suggestion-accept/blocked-after-accept | the key takes the suggestion but writes no block
# par-suggestion-accept/suggestion-gone-after-accept | an accepted suggestion stays on the bar
# par-suggestion-accept/core-count-zero-after-accept | the key dismisses instead of accepting
# par-suggestion-dismiss/remember-second | the second bar row reports no subject address
# par-suggestion-dismiss/second-offer | junk offers nothing the second time
# par-suggestion-offer/no-undo-offered-after-junk | ListChrome passes the foot no undoable flag (the default true): the bar offers Undo Z beside "no undo for junk yet"
# par-suggestion-dismiss/dismissed-gone | the dismiss click does nothing
# par-suggestion-dismiss/dismiss-wrote-no-block | dismiss behaves like accept


def main():
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    rows = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# par-") and " | " in l]
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
