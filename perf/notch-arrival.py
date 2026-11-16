#!/usr/bin/env python3
"""Perf cell fn-perf-notch: server receipt to notch drop.

Delivers the perf arrival of each mode through the fixture verb `fixture deliver perf-<mode> <fixtures/tests-j6.json> <mode>`
once per mode (idle: the connection was quiet; eventsource: an open event stream pushes it) and times, on this
side of the socket, from the verb's reply (the core has the event) until `rows mac-notch.list` lists the mail.
The budget NOTCH_ARRIVAL_MS is read from docs/CONSTANTS.md, never typed here. Exit 1 when a mode is over budget
or could not be measured (a cell that could not run is not green). Prints the build stamp and the machine load.
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 perf/notch-arrival.py --bundle <bundle>'
"""
import os
import plistlib
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable      # noqa: E402
from lib.fixture import const                           # noqa: E402
from lib import loadgate                                # noqa: E402  (the one load rule, journeys/lib/loadgate.py)
import J6                                               # noqa: E402

MODES = ["idle", "eventsource"]
POLL_S = 0.01


def stamp(bundle):
    if not bundle:
        return "unknown (no --bundle)"
    try:
        with open(os.path.join(bundle, "Contents", "Info.plist"), "rb") as f:
            p = plistlib.load(f)
        return "%s commit %s built %s dirty %s" % (p.get("NexusBuildLabel"), p.get("NexusBuildCommit"), p.get("NexusBuiltAt"), p.get("NexusBuildDirtyFiles"))
    except (OSError, ValueError) as e:
        return "unreadable (%s)" % e


def judged(ms, budget_ms, mutant=None):
    """The verdict on a time this script measured itself: pass at or under the budget, fail over it."""
    if mutant == "ignore-budget":
        return "pass"
    return "pass" if ms <= budget_ms else "fail"


def own_measure(ctl, mutant=None):
    """The app's own arrival measure, an extra: the app serves none today, so a refusal is words, never a verdict."""
    try:
        return "the app's own measure %s ms" % ctl.measure(J6.NOTCH + ".arrival").get("ms")
    except Unreadable as e:
        if mutant == "own-refusal-unjudges":
            raise                                    # the old line 49: a refusal of the extra voided the measured time
        return "the app serves no own measure (%s)" % e


def measure(ctl, mode, budget_ms, clock=time.monotonic, sleep=time.sleep, mutant=None):
    """-> (state, text). state: pass | fail | unjudged. The judged number is `ms`, timed here from the verb's reply."""
    mail = J6.F6["arrivals"]["perf-" + mode]["id"]
    try:
        if mail in [r["id"] for r in ctl.rows(J6.LIST)]:
            return "unjudged", "the notch already lists %s before the arrival (no baseline)" % mail
        ctl.send("fixture", "deliver", "perf-" + mode, J6.FX_PATH, mode)
        t0 = clock()
        limit = t0 + budget_ms / 1000.0 + 2.0          # keep polling past the budget so a slow arrival is measured, not guessed
        while clock() < limit:
            if mail in [r["id"] for r in ctl.rows(J6.LIST)]:
                ms = (clock() - t0) * 1000.0
                extra = own_measure(ctl, mutant)
                return judged(ms, budget_ms, mutant), "%s: %.0f ms to show (budget %d ms); %s" % (mode, ms, budget_ms, extra)
            sleep(POLL_S)
        return "fail", "%s: not shown within %d ms (budget %d ms)" % (mode, budget_ms + 2000, budget_ms)
    except Unreadable as e:
        return "unjudged", "%s: %s" % (mode, e)


class _Fake:
    """A rig that lists the arrival `delay_s` after the deliver verb, on a fake clock, and serves no own measure."""

    def __init__(self, mode, delay_s):
        self.t, self.delay, self.at, self.mail = 0.0, delay_s, None, J6.F6["arrivals"]["perf-" + mode]["id"]

    def clock(self):
        return self.t

    def sleep(self, s):
        self.t += s

    def send(self, *a):
        self.at = self.t

    def rows(self, _region):
        return [{"id": self.mail}] if self.at is not None and self.t - self.at >= self.delay else []

    def measure(self, _id):
        raise Unreadable("no measurement at %s: the app serves none" % _id)


def under_load(state, text, load):
    """Round 8 (lead 2026-11-13): a fail measured above PERF_LOAD_MAX is NOT JUDGED with its number; a pass holds and names the load."""
    over = {"fail": True, "pass": False}.get(state)
    v = loadgate.gate(over, load)
    return ({True: "fail", False: "pass", None: "unjudged" if over is not None else state}[v], text + (loadgate.note(load) if over is not None else ""))


def selftest():
    """Judges the verdict on a fake rig: a fast arrival passes, a slowed one fails, the refused own measure changes
    neither; each mutant must go red on its named case."""
    budget = 66
    cases = [("fast-arrival-passes", 0.02, "pass"), ("slowed-arrival-fails", budget / 1000.0 + 0.5, "fail")]
    mutants = {"own-refusal-unjudges": "fast-arrival-passes", "ignore-budget": "slowed-arrival-fails"}
    bad = 0

    def run(mutant):
        out = {}
        for name, delay, want in cases:
            f = _Fake("idle", delay)
            state, _ = measure(f, "idle", budget, f.clock, f.sleep, mutant)
            out[name] = (state, want)
        return out
    n = 0
    for name, (got, want) in run(None).items():
        n += 1
        ok = got == want
        bad += not ok
        print("%s %s (got %s, want %s)" % ("ok " if ok else "RED", name, got, want))
    for mu, case in mutants.items():
        n += 1
        got, want = run(mu)[case]
        red = got != want
        bad += not red
        print("%s mutant %s is red on %s (got %s, want %s)" % ("ok " if red else "NOT RED", mu, case, got, want))
    hi = (loadgate.load_limit() or 12.0) + 4
    for name, got, want in (("load: a fast arrival on a loaded Mac is a pass that names the load", under_load("pass", "12 ms", hi), ("pass", "12 ms" + loadgate.note(hi))),
                            ("load: a slow arrival on a loaded Mac is NOT JUDGED with its number", under_load("fail", "900 ms", hi)[0], "unjudged"),
                            ("load: a slow arrival at PERF_LOAD_MAX fails", under_load("fail", "900 ms", hi - 4)[0], "fail")):
        n += 1
        ok = got == want
        bad += not ok
        print("%s %s (got %s)" % ("ok " if ok else "RED", name, got))
    for name, ok in loadgate.selftest_cases(66):
        n += 1
        bad += not ok
        print("%s %s" % ("ok " if ok else "RED", name))
    print("%d checks, %d not as expected" % (n, bad))
    return 1 if bad else 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    bundle = argv[argv.index("--bundle") + 1] if "--bundle" in argv else os.environ.get("WHOOSH_BUNDLE", "")
    budget = const("NOTCH_ARRIVAL_MS")
    cap = loadgate.load_limit()
    print("build stamp: %s" % stamp(bundle))
    print("machine load (1 min): %.2f, limit PERF_LOAD_MAX: %s; above it a pass holds and a fail is NOT JUDGED (journeys/lib/loadgate.py)" % (loadgate.load_now(), cap))
    try:
        ctl = Ctl().connect()
    except CouldNotDrive as e:
        print("not judged: %s" % e)
        print("%d checks, 0 failed, %d not judged" % (len(MODES), len(MODES)))
        return 1
    results = []
    try:
        for m in MODES:
            results.append(measure(ctl, m, budget))
    finally:
        ctl.close()
    load = loadgate.load_now()
    results = [under_load(st, tx, load) for st, tx in results]
    for state, text in results:
        print("%-8s %s" % (state.upper(), text))
    f = sum(1 for s, _ in results if s == "fail")
    u = sum(1 for s, _ in results if s == "unjudged")
    print("%d checks, %d failed, %d not judged" % (len(results), f, u))
    return 1 if f or u else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
