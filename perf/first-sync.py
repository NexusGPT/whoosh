#!/usr/bin/env python3
"""first-sync: the first-sync time of a 2,000-message folder, without and with the compressed connection (item dc9d7086).

    python3 perf/first-sync.py [--runs N]      run the ignored measurement N times (default 3) on a quiet machine; the lead writes the figures
    python3 perf/first-sync.py --selftest      the reading and the verdicts against a model, then against each broken model: each must go red

The measurement is core/crates/sync-imap/tests/compress.rs `sync_2000_messages_with_and_without_compression` (ignored by a plain `cargo test`):
it syncs the same 2,000 messages from the loopback fake once clear ("before") and once with COMPRESS=DEFLATE ("after") and prints
    <label>: 2000 messages in <time>; compressed connections [<bool>...]
This script runs it with `cargo test --release -p sync-imap --test compress -- --ignored --nocapture sync_2000_messages_with_and_without_compression`
from core/, N times, and prints the build-free summary below. Loopback has no bandwidth limit: the figures show the CPU the deflate costs, never a
link's gain. The machine load is read before and after; above PERF_LOAD_MAX (docs/CONSTANTS.md) the figures are recorded but not judged.

Prints one `PASS <case>` line per passing case, `FAIL <case>` or `NOT JUDGED <case>` per other case, `first-sync: ...` with the medians, then
`N checks, F failed, U not judged`. Exit 1 when a case fails or is not judged.
"""
import os
import re
import statistics
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.fixture import const  # noqa: E402

COUNT = 2000
TEST = "sync_2000_messages_with_and_without_compression"
CMD = ["cargo", "test", "--release", "-p", "sync-imap", "--test", "compress", "--", "--ignored", "--nocapture", TEST]
LINE = re.compile(r"^(clear|deflate): (\d+) messages in ([0-9.]+)(ns|µs|ms|s); compressed connections \[([a-z, ]*)\]\s*$", re.M)
UNIT = {"ns": 1e-6, "µs": 1e-3, "ms": 1.0, "s": 1000.0}      # to milliseconds


def read(text):
    """{label: [(messages, ms, [compressed flags])]} from the test's printed lines."""
    out = {}
    for label, n, num, unit, flags in LINE.findall(text):
        out.setdefault(label, []).append((int(n), float(num) * UNIT[unit], [f.strip() == "true" for f in flags.split(",") if f.strip()]))
    return out


def judge(runs, load, limit, wanted):
    """[(case, verdict, detail)] with verdict pass | fail | unjudged; `runs` is read() of all runs, `wanted` the number of runs asked."""
    res = []
    for label, want_flag in (("clear", False), ("deflate", True)):
        rows = runs.get(label, [])
        if len(rows) != wanted:
            res.append(("first-sync-%s-read" % label, "unjudged", "%d of %d runs printed a %s line" % (len(rows), wanted, label)))
            continue
        bad = [r for r in rows if r[0] != COUNT]
        res.append(("first-sync-%s-count" % label, "fail" if bad else "pass", "every run synced %d messages" % COUNT if not bad else "a run synced %d messages" % bad[0][0]))
        wrong = [r for r in rows if r[2] != [want_flag]]
        res.append(("first-sync-%s-connection" % label, "fail" if wrong else "pass",
                    "compressed connections [%s] in every run" % str(want_flag).lower() if not wrong else "compressed connections %s, expected [%s]" % (wrong[0][2], str(want_flag).lower())))
        t = statistics.median(r[1] for r in rows)
        res.append(("first-sync-%s-time" % label, "pass" if t > 0 else "unjudged", "median %.1f ms over %d runs" % (t, len(rows)) if t > 0 else "a time of %s ms is not a reading" % t))
    if limit is None:
        res.append(("machine-load", "unjudged", "PERF_LOAD_MAX has no number in docs/CONSTANTS.md"))
    elif load > limit:
        res.append(("machine-load", "unjudged", "load %.2f is above PERF_LOAD_MAX %s: the figures are a load figure, not a product figure" % (load, limit)))
    else:
        res.append(("machine-load", "pass", "load %.2f is not above PERF_LOAD_MAX %s" % (load, limit)))
    return res


def summary(runs):
    c, d = runs.get("clear"), runs.get("deflate")
    if not c or not d:
        return "first-sync: not measured"
    a, b = statistics.median(r[1] for r in c), statistics.median(r[1] for r in d)
    return "first-sync: %d messages: clear (before) %.1f ms, deflate (after) %.1f ms, deflate/clear %.2f (loopback: the CPU cost of deflate, not a link's gain)" % (COUNT, a, b, b / a if a else float("nan"))


def finish(res):
    for n, v, d in res:
        if v == "pass":
            print("PASS %s: %s" % (n, d))
        else:
            print("  %s %s: %s" % ({"fail": "FAIL", "unjudged": "NOT JUDGED"}[v], n, d), file=sys.stderr)
    f = sum(1 for x in res if x[1] == "fail"); u = sum(1 for x in res if x[1] == "unjudged")
    print("%d checks, %d failed, %d not judged" % (len(res), f, u))
    return 1 if f or u or not res else 0


def model(mut):
    """What the test prints (3 runs), with one thing broken per mutant."""
    out = []
    for i in range(3):
        out.append("clear: %d messages in %.3fs; compressed connections [%s]" % (COUNT - ("short" in mut), 1.2 + i / 10, "true" if "clear-compressed" in mut else "false"))
        if "no-deflate-line" not in mut:
            out.append("deflate: %d messages in %.3fms; compressed connections [%s]" % (COUNT, 1500.0 + i, "false" if "deflate-clear" in mut else "true"))
    return "running 1 test\n" + "\n".join(out) + "\ntest result: ok\n"


def selftest():
    cases = [("base", "", 1.0, "pass"), ("a run synced too few messages", "short", 1.0, "fail"), ("the clear run was compressed", "clear-compressed", 1.0, "fail"),
             ("the deflate run was not compressed", "deflate-clear", 1.0, "fail"), ("no deflate line", "no-deflate-line", 1.0, "unjudged"),
             ("machine load over the limit", "", 30.0, "unjudged")]
    bad = 0
    for name, mut, load, want in cases:
        res = judge(read(model(mut)), load, 12.0, 3)
        got = "pass" if all(v == "pass" for _n, v, _d in res) else "fail" if any(v == "fail" for _n, v, _d in res) else "unjudged"
        ok = got == want
        bad += not ok
        print("%-4s %-40s %s" % ("ok" if ok else "BAD", name, got))
    r = read("clear: 2000 messages in 850.5µs; compressed connections [false]\ndeflate: 2000 messages in 1.5s; compressed connections [true]\n")
    ok = r == {"clear": [(2000, 0.8505, [False])], "deflate": [(2000, 1500.0, [True])]}
    bad += not ok
    print("%-4s %-40s %s" % ("ok" if ok else "BAD", "units: µs and s read in ms", r))
    print("%d checks, %d failed, 0 not judged" % (len(cases) + 1, bad))
    return 1 if bad else 0


def load_now():
    try:
        return os.getloadavg()[0]
    except OSError:
        return float("inf")


def main(argv):
    if "--selftest" in argv:
        return selftest()
    wanted = int(argv[argv.index("--runs") + 1]) if "--runs" in argv else 3
    try:
        limit = float(const("PERF_LOAD_MAX"))
    except KeyError:
        limit = None
    before = load_now()
    print("machine load (1 min) before: %.2f, limit PERF_LOAD_MAX: %s" % (before, limit))
    text, worst = "", before
    for i in range(wanted):
        try:
            p = subprocess.run(CMD, cwd=os.path.join(ROOT, "core"), capture_output=True, text=True, timeout=900)
        except (OSError, subprocess.TimeoutExpired) as e:
            print("could not run the measurement: %s %s" % (type(e).__name__, e), file=sys.stderr)
            return finish([("first-sync-run", "unjudged", "run %d: %s" % (i + 1, type(e).__name__))])
        text += p.stdout
        worst = max(worst, load_now())
        if p.returncode != 0:
            print("cargo exit %d on run %d:\n%s" % (p.returncode, i + 1, (p.stdout + p.stderr)[-2000:]), file=sys.stderr)
            return finish([("first-sync-run", "fail", "cargo exited %d on run %d" % (p.returncode, i + 1))])
    print("machine load (1 min) after: %.2f; the judged load is the highest of the readings: %.2f" % (load_now(), worst))
    runs = read(text)
    print(summary(runs))
    return finish(judge(runs, worst, limit, wanted))


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
