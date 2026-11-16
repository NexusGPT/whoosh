#!/usr/bin/env python3
"""Selftest of journeys/lib/harness.py `state_line`: python3 checks/checklib/statelinetest.py --selftest
A FAIL and a NOT JUDGED carry the one state line; a PASS does not; a state read that raises hides neither verdict; the line stays under 400 characters."""
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
import json, tempfile
from lib.harness import Journey, state_line, write_results   # noqa: E402
from lib.ctl import Unreadable                 # noqa: E402


class Ctl:
    def __init__(self, mode="ok"):
        self.mode = mode

    def uistate(self):
        if self.mode == "raise":
            raise Unreadable("socket closed")
        if self.mode == "typeerror":
            raise TypeError("odd reply")
        if self.mode == "long":
            return {"surface": "x" * 600}
        return {"surface": "mac-main", "first_responder": "other:mac-settings.field", "overlay": "sheet", "account_filter": "acc-2", "tab": "primary", "focus_message": "m1"}


def run(mode):
    j = Journey("t", Ctl(mode))
    s = j.step("S1", "t")
    s.expect("passes", lambda: 1, 1)
    s.expect("fails", lambda: 1, 2)
    s.expect("unjudged", lambda: (_ for _ in ()).throw(Unreadable("no row")), 1)
    return {c.label.split("/", 1)[1]: c for c in j.checks()}


def main():
    bad = []
    r = run("ok")
    if "state:" in r["passes"].detail: bad.append("a PASS carries a state line")
    for n in ("fails", "unjudged"):
        d = r[n].detail
        for want in ("surface=mac-main", "focus=other:mac-settings.field", "editing=yes", "overlay=sheet", "account_filter=acc-2"):
            if want not in d: bad.append("%s lacks %s: %s" % (n, want, d))
    if r["fails"].state != "fail" or r["unjudged"].state != "unjudged": bad.append("verdicts changed")
    for mode in ("raise", "typeerror"):
        r = run(mode)
        if r["fails"].state != "fail" or "state: unreadable" not in r["fails"].detail or "expected 2, got 1" not in r["fails"].detail:
            bad.append("%s hides or garbles the FAIL: %s" % (mode, r["fails"].detail))
        if r["unjudged"].state != "unjudged" or "no row" not in r["unjudged"].detail:
            bad.append("%s hides the NOT JUDGED reason" % mode)
    if len(state_line(Ctl("long"))) >= 400: bad.append("line not under 400")
    if "no socket" not in state_line(None): bad.append("no-socket line")
    # results file: three cases, one of each verdict, three parsing lines; an unwritable folder never raises
    j = Journey("t", Ctl("ok"))
    s = j.step("S1", "t")
    s.expect("passes", lambda: 1, 1)
    s.expect("fails", lambda: 1, 2)
    s.expect("unjudged", lambda: (_ for _ in ()).throw(Unreadable("no row")), 1)
    d = tempfile.mkdtemp(prefix="wres")
    os.environ["WHOOSH_BUILD_LABEL"] = "main99"
    path = write_results("chk", j, d)
    try:
        rows = [json.loads(l) for l in open(path, encoding="utf-8")]
        if [r["verdict"] for r in rows] != ["PASS", "FAIL", "NOT JUDGED"]: bad.append("verdicts %r" % [r["verdict"] for r in rows])
        if not all({"check", "step", "case", "verdict", "expected", "got", "seconds", "build", "commit"} <= set(r) for r in rows): bad.append("fields missing")
        if rows[1]["expected"] != 2 or rows[1]["got"] != 1 or rows[0]["build"] != "main99": bad.append("values %r" % rows[1])
        if os.path.basename(path) != "chk-main99.results.jsonl" or [f for f in os.listdir(d) if ".tmp-" in f]: bad.append("file name or temp left: %s" % os.listdir(d))
    except Exception as e:      # noqa: BLE001
        bad.append("results file unreadable: %s %s" % (type(e).__name__, e))
    try:
        got = write_results("chk", j, "/proc/definitely/not/writable")
        if got is not None: bad.append("unwritable folder returned a path")
    except BaseException as e:      # noqa: BLE001
        bad.append("unwritable folder raised %s" % type(e).__name__)
    for b in bad: print("FAIL", b)
    print("state line selftest: %d problems" % len(bad))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
