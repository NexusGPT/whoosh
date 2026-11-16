#!/usr/bin/env python3
"""Experiment E1 (realcore-firstrun test plan, section 3): do real key events reach the search field through the monitors?

THE PASS LINE, FIXED BEFORE ANY RUN (probes-r1, round 5; do not edit it after a run):
  arm a  `keyevent arm:a h`, `e`, `l`, `l`, `o` after `click mac-search.bar` (NSApp.sendEvent):
         `rows mac-search.bar` reads text == "hello" AND cursor is true, AND `rows mac-keyevent.monitor` reads seen == 5.
  arm b  the same five keys through `window.sendEvent` (bypasses the monitors): seen == 0.
         (This proves the counter can fail. The text in the field is recorded, not judged.)
  arm c  the same through NSApp.sendEvent with the window resigned as key: RECORDED (text, cursor, seen, key_before, key_after), not judged.
  arm d  only when arm a failed: the same after `makeKey()`; if d passes the arm-a line and a did not, the verb must make the window key.
Verdict: E1 passes when arm a passes its line AND arm b passes its line. If no arm passes the arm-a line, the A113 class stays owner-only
and this script says so.
Run (the lead, one hidden launch, fixture core):
  sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/keys-real-e1.py'
No app: python3 checks/keys-real-e1.py --selftest   (the judge on scripted readings, one mutant per half of the line)
"""
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable      # noqa: E402

BAR = "mac-search.bar"
MON = "mac-keyevent.monitor"
LETTERS = "hello"


def judge_a(bar, seen):
    return bar.get("text") == "hello" and bar.get("cursor") is True and seen == 5


def judge_b(seen):
    return seen == 0


def one_arm(ctl, arm):
    ctl.send("fixture", "reset")
    ctl.send("keyevent", "reset")
    ctl.send("click", BAR)
    last = {}
    for ch in LETTERS:
        last = ctl.send("keyevent", "arm:" + arm, ch)
    bar = (ctl.rows(BAR) or [{}])[0]
    seen = (ctl.rows(MON) or [{}])[0].get("seen")
    return bar, seen, last


def main(argv):
    if "--selftest" in argv:
        return selftest()
    try:
        ctl = Ctl().connect()
    except CouldNotDrive as e:
        print("NOT JUDGED: no app to drive (%s)" % e)
        return 1
    out, ok = {}, True
    try:
        for arm in ("a", "b", "c"):
            out[arm] = one_arm(ctl, arm)
            bar, seen, last = out[arm]
            print("arm %s: text=%r cursor=%r focused=%r seen=%r | key_before=%r key_after=%r" % (arm, bar.get("text"), bar.get("cursor"), bar.get("focused"), seen, last.get("key_before"), last.get("key_after")))
        a_ok, b_ok = judge_a(out["a"][0], out["a"][1]), judge_b(out["b"][1])
        print("arm a pass line (text hello, cursor true, seen 5): %s" % ("PASS" if a_ok else "FAIL"))
        print("arm b pass line (seen 0): %s" % ("PASS" if b_ok else "FAIL"))
        if not a_ok:
            out["d"] = one_arm(ctl, "d")
            bar, seen, last = out["d"]
            d_ok = judge_a(bar, seen)
            print("arm d (makeKey first): text=%r cursor=%r seen=%r -> %s" % (bar.get("text"), bar.get("cursor"), seen, "PASS: the verb must make the window key" if d_ok else "FAIL"))
            if not d_ok:
                print("NO ARM PASSES: the A113 class (real keys in the field) stays owner-only.")
        ok = a_ok and b_ok
    except Unreadable as e:
        print("NOT JUDGED: %s" % e)
        return 1
    finally:
        ctl.close()
    print("E1 %s" % ("PASSES" if ok else "DOES NOT PASS"))
    return 0 if ok else 1


def selftest():
    cases = [("a passes", judge_a({"text": "hello", "cursor": True}, 5), True),
             ("a red when the text is empty", judge_a({"text": "", "cursor": True}, 5), False),
             ("a red when the cursor is not in the field", judge_a({"text": "hello", "cursor": False}, 5), False),
             ("a red when the monitor saw 0 (the counter can fail)", judge_a({"text": "hello", "cursor": True}, 0), False),
             ("b passes at 0", judge_b(0), True), ("b red at 5 (the monitor was not bypassed)", judge_b(5), False)]
    bad = 0
    for n, got, want in cases:
        bad += got != want
        print("%s %s" % ("ok " if got == want else "RED", n))
    print("%d checks, %d not as expected" % (len(cases), bad))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
