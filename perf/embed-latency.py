#!/usr/bin/env python3
"""The Mac's embed latency with the real meaning model (item b9619482, fn-embedding; perf-archive round 6). Not a Swift test (owner
2026-11-12: "stop with the 700+ swift tests"): the hidden copy embeds through its own GemmaEmbedder while a meaning search runs, and the
app records each call into the model with two clock reads (Sources/Features/Embedding/EmbedTimes.swift), served by
`measure mac-main.interaction.embed`.

  python3 perf/embed-latency.py <App.app> --weights DIR [--searches N] [--fixture-load NAME]   the lead's run (hidden copy, fixture core, the real model);
                                          --fixture-load gives a larger corpus (docs/CONTROL-SOCKET.md `fixture load`) when the default one makes fewer
                                          than MIN_SAMPLES document batches
  python3 perf/embed-latency.py --selftest                                proves the judgement and its broken copies

The run launches the rig copy once through scripts/launch-one.sh (WHOOSH_RIG_HIDDEN=1, --fixture-core, WHOOSH_EMBED_WEIGHTS=DIR) and runs
N meaning searches (default 11), each with its own words, so each one embeds its query; the first search also embeds the corpus (mail
chunks, in batches). Then it reads every recorded call and prints, per kind, the median and the worst:
  query           ms of one embedQuery call (one short text)
  document chunk  ms of one embedDocumentBatch / embedTitled call divided by the texts it embedded (one mail chunk)
The FIRST call of the run is dropped and printed apart: it carries the model's load. Verdicts, one line each:
  PASS|OVER embed-query-median            median < EMBED_MS_MAC
  PASS|OVER embed-document-chunk-median   median < EMBED_MS_MAC
EMBED_MS_MAC is the ledger's "median milliseconds to embed one 512-token chunk with the model loaded, Mac, release build": the run
prints the build stamp; a Debug build's figure is printed and judged, and the line says Debug. Fewer than MIN_SAMPLES warm calls of a kind,
or a reading of 0 ms, is NOT JUDGED, never a pass. The worst of each kind is printed, not judged (the ledger row is a median).
EMBED_MS_IPHONE stays outside (a real iPhone). Exit 1 on an OVER or when nothing was judged.
"""
import importlib.util, json, os, statistics, subprocess, sys, time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable      # noqa: E402
from lib.fixture import const                            # noqa: E402
from lib import loadgate                                 # noqa: E402  (the one load rule, journeys/lib/loadgate.py)

MIN_SAMPLES = 5         # a check bound: fewer warm calls of a kind give no median worth judging
SEARCHES = 11           # the first is cold (the model loads); ten warm queries remain
RESULTS = "mac-search.results"
SEARCH_WAIT_S = 240     # the first search loads the model and embeds the corpus


def footprint():
    """perf/footprint.py as a module (its header, hidden-mode test, weights finder and query text are reused, never copied)."""
    spec = importlib.util.spec_from_file_location("footprint", os.path.join(ROOT, "perf", "footprint.py"))
    mod = importlib.util.module_from_spec(spec); spec.loader.exec_module(mod)
    return mod


def judge(calls, budget, drop_cold=True, per_chunk=True, middle=statistics.median, load=0.0):
    """calls: [{"seq", "kind", "ms", "texts"}]. Returns (rows, cold) where rows are (case, text, over True/False/None). `drop_cold`,
    `per_chunk` and `middle` are parameters only so the selftest can run the broken copies."""
    calls = sorted(calls, key=lambda c: c["seq"])
    cold = calls[0] if (drop_cold and calls) else None
    warm = calls[1:] if (drop_cold and calls) else calls
    rows = []
    for kind, case in (("query", "embed-query-median"), ("document", "embed-document-chunk-median")):
        vals = [c["ms"] / max(1, c["texts"]) if (per_chunk and kind == "document") else c["ms"] for c in warm if c["kind"] == kind]
        if len(vals) < MIN_SAMPLES:
            rows.append((case, "not judged: %d warm %s call(s), fewer than %d" % (len(vals), kind, MIN_SAMPLES), None)); continue
        if not all(v > 0 for v in vals):
            rows.append((case, "not judged: a reading of %s ms is not a reading of a running model" % min(vals), None)); continue
        m = middle(vals)
        # a timing: inside the budget passes at any load, over it above PERF_LOAD_MAX is NOT JUDGED (loadgate, round 8)
        rows.append((case, "median %.1f ms, worst %.1f ms over %d warm calls (budget EMBED_MS_MAC %s ms)%s" % (m, max(vals), len(vals), budget, loadgate.note(load)),
                     loadgate.gate(m >= budget, load)))
    return rows, cold


def finish(rows, refusal=None):
    if refusal:
        print("embed-latency: %s" % refusal); print("0 checks, 0 failed, 1 not judged"); return 1
    for case, text, over in rows:
        print("%s %s: %s" % ({True: "OVER", False: "PASS", None: "NOT JUDGED"}[over], case, text))
    bad = [r for r in rows if r[2] is True]
    nj = [r for r in rows if r[2] is None]
    print("%d checks, %d failed, %d not judged" % (len(rows), len(bad), len(nj)))
    return 1 if bad or not any(r[2] is False for r in rows) else 0


def inside(argv):
    """Inside scripts/launch-one.sh: N meaning searches, then the recorded calls."""
    fp = footprint()
    n = int(argv[argv.index("--searches") + 1]) if "--searches" in argv else SEARCHES
    base = fp.query_text()
    try:
        c = Ctl().connect()
        if "--fixture-load" in argv:          # a larger corpus gives more document batches (EMBED_BATCH mails each)
            c.sock.settimeout(fp.FIXTURE_LOAD_WAIT_S); c.send("fixture", "load", argv[argv.index("--fixture-load") + 1]); c.sock.settimeout(5.0)
        for i in range(n):
            words = base if i == 0 else "%s %d" % (base, i)        # each search its own words: each embeds its own query
            for step in (lambda: c.key("/"), lambda: c.key("⌘A"), lambda: c.type(words), lambda: c.key("Enter")):
                step()
            t_end = time.monotonic() + SEARCH_WAIT_S
            while True:
                try:
                    if c.rows(RESULTS):
                        break
                except Unreadable:
                    c.close(); c = Ctl().connect()          # the first search loads the model on a busy app; a late reply would answer the next call
                if time.monotonic() > t_end:
                    raise Unreadable("search %d answered no result in %d s" % (i + 1, SEARCH_WAIT_S))
                time.sleep(0.5)
            c.key("Escape")
        calls = c.measure("mac-main.interaction.embed").get("calls") or []
        c.close()
    except (Unreadable, CouldNotDrive, OSError) as e:
        return finish([], "not judged: the run stopped: %s" % e)
    rows, cold = judge(calls, const("EMBED_MS_MAC"), load=os.getloadavg()[0])
    print("embed-latency: %d calls recorded; the first (dropped, it carries the model's load): %s" % (len(calls), json.dumps(cold)))
    print("embed-latency: raw " + json.dumps(calls))
    return finish(rows)


def outer(argv):
    fp = footprint()
    arg = lambda k: argv[argv.index(k) + 1] if k in argv else None
    bundle = next((a for a in argv if a.endswith(".app")), None) or arg("--bundle") or os.environ.get("WHOOSH_BUNDLE")
    refusal = fp.header(bundle, False)
    if refusal:
        return finish([], refusal.replace("footprint", "embed-latency"))
    if not fp.supports_hidden(bundle):
        return finish([], "HELD, not launched: %s has no hidden rig mode" % bundle)
    weights = fp.find_weights(arg("--weights"))
    if not weights:
        return finish([], "not judged: no model folder (--weights DIR or WHOOSH_EMBED_WEIGHTS): the real model cannot run")
    print("embed-latency: model folder %s; build %s" % (weights, fp.stamp(bundle)))
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1 WHOOSH_EMBED_WEIGHTS=%s" % weights)
    inner = [sys.executable, os.path.abspath(__file__), "--inside"] + (["--searches", arg("--searches")] if arg("--searches") else []) \
        + (["--fixture-load", arg("--fixture-load")] if arg("--fixture-load") else [])
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), os.path.abspath(bundle), "--", "--fixture-core", "--rig-hidden"] + inner
    rc = subprocess.run(cmd, cwd=ROOT, env=env).returncode
    if rc == 6:
        return finish([], "HELD: hidden mode not available (scripts/launch-one.sh exited 6); nothing was launched. Not a red.")
    if rc not in (0, 1):
        return finish([], "not judged: scripts/launch-one.sh exited %d" % rc)
    return rc


def selftest():
    b = const("EMBED_MS_MAC")
    def calls(q_ms, d_ms, d_texts=32, n=10, cold_ms=3000.0):
        out = [{"seq": 1, "kind": "document", "ms": cold_ms, "texts": d_texts}]       # the first call carries the load
        for i in range(n):
            out.append({"seq": 2 + 2 * i, "kind": "query", "ms": q_ms, "texts": 1})
            out.append({"seq": 3 + 2 * i, "kind": "document", "ms": d_ms * d_texts, "texts": d_texts})
        return out
    v = lambda rows, case: next(r[2] for r in rows if r[0] == case)
    q, d = "embed-query-median", "embed-document-chunk-median"
    good = calls(b * 0.5, b * 0.5)
    cases = [
        ("both medians under the budget pass", lambda: [v(judge(good, b)[0], q), v(judge(good, b)[0], d)] == [False, False]),
        ("a query median at the budget is OVER", lambda: v(judge(calls(b, b * 0.5), b)[0], q) is True),
        ("a chunk median at the budget is OVER (the call's ms over its texts)", lambda: v(judge(calls(b * 0.5, b), b)[0], d) is True),
        ("the first call (the model's load, 3 s) is dropped, not judged", lambda: judge(good, b)[1]["ms"] == 3000.0 and v(judge(good, b)[0], d) is False),
        ("fewer than MIN_SAMPLES warm calls is NOT JUDGED", lambda: v(judge(calls(1.0, 1.0, n=MIN_SAMPLES - 1), b)[0], q) is None),
        ("a 0 ms reading is NOT JUDGED", lambda: v(judge(calls(0.0, 1.0), b)[0], q) is None),
        ("no call at all: both NOT JUDGED, the run exits 1", lambda: [r[2] for r in judge([], b)[0]] == [None, None] and finish(judge([], b)[0]) == 1),
        ("BROKEN COPY keeps the cold call: it counts the load as a warm call and judges 4 warm calls that the real judge refuses",
         lambda: v(judge(calls(1.0, 1.0, n=4), b, drop_cold=False)[0], d) is False and v(judge(calls(1.0, 1.0, n=4), b)[0], d) is None),
        ("BROKEN COPY judges the whole batch call, not one chunk: the 32-text batch reads OVER although each chunk is under",
         lambda: v(judge(good, b, per_chunk=False)[0], d) is True and v(judge(good, b)[0], d) is False),
        ("BROKEN COPY takes the mean: one slow warm call lifts it over the budget while the median stays under",
         lambda: v(judge(good + [{"seq": 99, "kind": "query", "ms": b * 20, "texts": 1}], b, middle=statistics.mean)[0], q) is True
         and v(judge(good + [{"seq": 99, "kind": "query", "ms": b * 20, "texts": 1}], b)[0], q) is False),
    ]
    hi = (loadgate.load_limit() or 12.0) + 4
    cases += [("load: medians inside the budget on a loaded Mac PASS and name the load",
               lambda: [v(judge(good, b, load=hi)[0], q), v(judge(good, b, load=hi)[0], d)] == [False, False] and "machine load %.1f" % hi in judge(good, b, load=hi)[0][0][1]),
              ("load: a median over the budget on a loaded Mac is NOT JUDGED, the number printed",
               lambda: v(judge(calls(b * 2, b * 0.5), b, load=hi)[0], q) is None and "median %.1f" % (b * 2) in judge(calls(b * 2, b * 0.5), b, load=hi)[0][0][1])]
    cases += [(n, (lambda ok: lambda: ok)(ok)) for n, ok in loadgate.selftest_cases(b)]
    bad = 0
    for name, f in cases:
        ok = bool(f()); bad += not ok
        print("%-4s %s" % ("ok" if ok else "BAD", name))
    print("%d checks, %d failed, 0 not judged" % (len(cases), bad)); return 1 if bad else 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    if "--inside" in argv:
        return inside(argv)
    return outer(argv)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
