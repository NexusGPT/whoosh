#!/usr/bin/env python3
"""Performance cell: the first screen of search results on 1M messages (docs/CONSTANTS.md SEARCH_FIRST_SCREEN_MS).

    sh scripts/launch-one.sh <bundle> -- sh -c 'WHOOSH_SOCKET_DIR="$D" python3 perf/search.py --bundle <bundle>'
    python3 perf/search.py [--bundle PATH] [--runs N]

Prints the build stamp and the machine load first. Exit 1 when the median first-screen time is AT OR OVER the budget (the
budget is the line a screen must stay under), and exit 1 when the cell is not judged. A cell is not judged when:
  - the machine load is above PERF_LOAD_MAX (docs/CONSTANTS.md; if the ledger holds no number for it the cell is not judged
    and says so: a clock verdict needs the threshold),
  - the build stamp cannot be read,
  - the app is not reachable, cannot seed the corpus (`fixture seed-corpus <N>`, reserved at this commit), or reports fewer
    messages than the ledger's "1M".
Every number comes from docs/CONSTANTS.md. `--load-max-for-selftest X` exists only for _captures/tests-j2/perf-selftest.py.
Prints `N checks, F failed, U not judged` last.
`--selftest` judges the verdict rules below on made-up numbers (just under the ledger's budget ok, at it FAIL, a loaded machine, fewer than 1M messages: NOT JUDGED), prints `ok|BAD <case>`
per case and `N checks, F failed, 0 not judged`; it launches no app and reads only docs/CONSTANTS.md.
"""
import glob
import json
import os
import re
import statistics
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable  # noqa: E402

CONSTANTS = os.path.join(ROOT, "docs", "CONSTANTS.md")
CORPUS = json.load(open(os.path.join(ROOT, "fixtures", "tests-j2.json"), encoding="utf-8"))
QUERY = CORPUS["expect"]["meaning_query"]
RES = "mac-search.results"


def ledger_cell(name):
    for line in open(CONSTANTS, encoding="utf-8"):
        if line.startswith("|"):
            cells = [c.strip() for c in line.strip().strip("|").split("|")]
            if len(cells) >= 2 and name in [n.strip() for n in re.split(r"[·/]", cells[0])]:
                return cells[1]
    return None


def number(cell):
    m = re.search(r"\d[\d,]*(?:\.\d+)?", cell or "")
    return float(m.group(0).replace(",", "")) if m else None


SEED_WAIT_S = 900    # the longest the tool waits for `fixture seed-corpus`; a tool's patience, not a budget of the app


def build_stamp(bundle):
    # The bundle's own stamp first (the Info.plist keys of scripts/build.sh, as perf/interaction.py reads them): a kept copy of a build sits outside its
    # build tree, where no build-<label>.json is found (17 Nov 2026, main206: the one run on a quiet Mac ended NOT JUDGED on "the build stamp cannot be read").
    if bundle:
        try:
            import plistlib
            with open(os.path.join(bundle, "Contents", "Info.plist"), "rb") as f:
                p = plistlib.load(f)
            return "label=%s commit=%s built_at=%s dirty_files=%s" % (p["NexusBuildLabel"], p["NexusBuildCommit"][:12], p["NexusBuiltAt"], p.get("NexusBuildDirtyFiles", "?"))
        except (OSError, KeyError, ValueError):
            pass
    tree = os.path.abspath(bundle)
    for _ in range(4):
        tree = os.path.dirname(tree)                # Whoosh.app -> Debug -> Products -> Build -> the build tree
    cands = [os.path.join(tree, "build-*.json")] if bundle else []
    cands += glob.glob(os.path.join(ROOT, "build", "Whoosh-*", "build-*.json"))
    for pat in cands:
        for p in sorted(glob.glob(pat), key=os.path.getmtime, reverse=True):
            try:
                d = json.load(open(p))
                return "label=%s commit=%s built_at=%s worktree=%s dirty_files=%s" % (d["label"], d["commit"][:12], d["built_at"], d["worktree"], d.get("uncommitted_files"))
            except (OSError, ValueError, KeyError):
                continue
    return None


def over_budget(median_ms, budget):
    """The exit-1 rule: the median first-screen time AT OR OVER the budget fails (the budget is the line a screen must stay under)."""
    return median_ms >= budget


def not_judged_reasons(budget, corpus_n, load, load_max, stamp):
    """Why the cell cannot give a clock verdict before the app is driven (empty: it can). One sentence each."""
    why = []
    if budget is None or corpus_n is None:
        why.append("SEARCH_FIRST_SCREEN_MS or its '1M messages' size is missing from docs/CONSTANTS.md")
    if load_max is None:
        why.append("PERF_LOAD_MAX has no number in docs/CONSTANTS.md, so no clock verdict can be given")
    # round 8 (lead 2026-11-13, journeys/lib/loadgate.py): a load above PERF_LOAD_MAX no longer stops the run; `verdict` applies it
    if not stamp:
        why.append("the build stamp cannot be read")
    return why


def verdict(median_ms, budget, load, load_max):
    """'pass', 'fail' or 'not judged': the median inside the budget passes at any load; over it, above PERF_LOAD_MAX, is not judged."""
    over = over_budget(median_ms, budget)
    if over and load_max is not None and load > load_max:
        return "not judged"
    return "fail" if over else "pass"


def corpus_reason(have, corpus_n):
    """Why the corpus the app reports is not the one the budget is for (None: it is)."""
    if not isinstance(have, (int, float)) or have < corpus_n:
        return "the app reports %r indexed messages, the budget is for %d" % (have, corpus_n)
    return None


def selftest():
    """Each rule on made-up numbers, the budget, the size and the load limit read from the ledger. No app is launched."""
    budget, load_max = number(ledger_cell("SEARCH_FIRST_SCREEN_MS")), number(ledger_cell("PERF_LOAD_MAX"))
    size_m = re.search(r"on\s+([\d.]+)\s*M\s+messages", ledger_cell("SEARCH_FIRST_SCREEN_MS") or "")
    n = int(float(size_m.group(1)) * 1_000_000) if size_m else None
    quiet, stamp = (load_max or 0) / 2.0, "label=selftest"
    cases = [("a median just under the budget passes", over_budget(budget - 0.1, budget), False),
             ("a median at the budget fails", over_budget(budget, budget), True),
             ("a median over the budget fails", over_budget(budget + 25.0, budget), True),
             ("a quiet machine, the stamp, the ledger numbers: judged", not_judged_reasons(budget, n, quiet, load_max, stamp), []),
             ("a load above PERF_LOAD_MAX no longer stops the run", not_judged_reasons(budget, n, load_max + 1.0, load_max, stamp), []),
             ("load: a median inside the budget on a loaded Mac is a PASS", verdict(budget - 1, budget, load_max + 4, load_max), "pass"),
             ("load: a median over the budget on a loaded Mac is NOT JUDGED", verdict(budget + 1, budget, load_max + 4, load_max), "not judged"),
             ("load: a median over the budget at PERF_LOAD_MAX fails", verdict(budget + 1, budget, load_max, load_max), "fail"),
             ("a load exactly at PERF_LOAD_MAX is judged", not_judged_reasons(budget, n, load_max, load_max, stamp), []),
             ("no build stamp is NOT JUDGED", len(not_judged_reasons(budget, n, quiet, load_max, None)), 1),
             ("no ledger limit for the load is NOT JUDGED", len(not_judged_reasons(budget, n, quiet, None, stamp)), 1),
             ("no ledger budget is NOT JUDGED", len(not_judged_reasons(None, n, quiet, load_max, stamp)), 1),
             ("fewer than the budget's messages is NOT JUDGED", corpus_reason(n - 1, n) is not None, True),
             ("exactly the budget's messages is judged", corpus_reason(n, n), None),
             ("a count the app does not give is NOT JUDGED", corpus_reason(None, n) is not None, True)]
    bad = 0
    for name, got, want in cases:
        ok = got == want
        print("%-4s %s" % ("ok" if ok else "BAD", name))
        bad += not ok
    print("%d checks, %d failed, 0 not judged" % (len(cases), bad))
    return 1 if bad else 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    arg = lambda k, d=None: argv[argv.index(k) + 1] if k in argv else d
    runs = int(arg("--runs", "15"))
    stamp = build_stamp(arg("--bundle") or os.environ.get("WHOOSH_BUNDLE"))
    load = os.getloadavg()[0]
    print("build stamp: %s" % (stamp or "UNKNOWN (no build-<label>.json found)"))
    print("machine load (1 min): %.2f" % load)
    budget = number(ledger_cell("SEARCH_FIRST_SCREEN_MS"))
    size_m = re.search(r"on\s+([\d.]+)\s*M\s+messages", ledger_cell("SEARCH_FIRST_SCREEN_MS") or "")
    corpus_n = int(float(size_m.group(1)) * 1_000_000) if size_m else None
    load_max = arg("--load-max-for-selftest")
    load_max = float(load_max) if load_max else number(ledger_cell("PERF_LOAD_MAX"))
    print("budget: first results screen < %s ms on %s messages (docs/CONSTANTS.md SEARCH_FIRST_SCREEN_MS)" % (budget, corpus_n))
    why = not_judged_reasons(budget, corpus_n, load, load_max, stamp)
    ctl = None
    if not why:
        try:
            ctl = Ctl().connect()
            # The fill of the corpus gets its own wait: one million generated mails take longer than the driver's 5 s for a plain request
            # (17 Nov 2026, main206, load 8.8: NOT JUDGED on "seed-corpus 1000000: timed out"). The time it took is printed.
            import time
            import lib.ctl as _ctl
            saved, _ctl.TIMEOUT_S, t0 = _ctl.TIMEOUT_S, SEED_WAIT_S, time.time()
            try:
                ctl.send("fixture", "seed-corpus", str(corpus_n))
                t1 = time.time()
                have = ctl.core("index/message_count")      # the app's first answer after the fill: it may still be settling its lists on the new corpus
            finally:
                _ctl.TIMEOUT_S = saved
            print("corpus: %s mails made in %.1f s; the app's first answer after it took %.1f s (wait limit %s s)" % (corpus_n, t1 - t0, time.time() - t1, SEED_WAIT_S))
            if corpus_reason(have, corpus_n):
                why.append(corpus_reason(have, corpus_n))
        except CouldNotDrive as e:
            why.append("no app to drive: %s" % e)
        except Unreadable as e:
            why.append("the app cannot seed or report the corpus: %s" % e)
    if why:
        for w in why:
            print("NOT JUDGED: %s" % w)
        print("1 checks, 0 failed, 1 not judged")
        return 1
    samples = []
    try:
        for _ in range(runs):
            ctl.key("/")
            ctl.key("⌘A")
            ctl.type(QUERY)
            samples.append(float(ctl.measure(RES)["ms"]))
            ctl.key("Escape")
    except Unreadable as e:
        print("NOT JUDGED: a run could not be driven: %s" % e)
        print("1 checks, 0 failed, 1 not judged")
        return 1
    finally:
        ctl.close()
    med = statistics.median(samples)
    load = os.getloadavg()[0]
    print("first results screen over %d runs: median %.1f ms, max %.1f ms, min %.1f ms (machine load %.2f at the end, PERF_LOAD_MAX %s)" % (runs, med, max(samples), min(samples), load, load_max))
    if verdict(med, budget, load, load_max) == "not judged":
        print("NOT JUDGED: median %.1f ms is at or over the %s ms budget, measured at machine load %.2f above PERF_LOAD_MAX %s" % (med, budget, load, load_max))
        print("1 checks, 0 failed, 1 not judged")
        return 1
    if over_budget(med, budget):
        print("FAIL: median %.1f ms is at or over the %s ms budget" % (med, budget))
        print("1 checks, 1 failed, 0 not judged")
        return 1
    print("1 checks, 0 failed, 0 not judged")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
