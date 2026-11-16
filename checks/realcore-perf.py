#!/usr/bin/env python3
"""The move time on the REAL core, against a local mail server with a real round-trip delay (perf-realcore-r1).

perf/interaction.py measures the fixture core: zero latency, no network. The owner's real inbox (Microsoft 365 over IMAP) lags when
moving with the arrow keys; the real core fetches a message's full body from the server the first time the reader shows it. This check
starts the local fake (checks/realcore_mailfake.py) with a wait before each SELECT and FETCH, launches the rig copy with the REAL core the way
slice 2 of checks/realcore-check.py does, adds the local account through the sheet, waits for the list, then drives the same moves as
perf/interaction.py's "next" and "open" actions (J/K alternating 33 times, Enter then Escape 33 times) and reads, after each,
`measure mac-main.interaction.frame`, `.commit` and `.gap`.

  python3 checks/realcore-perf.py <bundle> [--latency-ms 300] [--messages 60] [--baseline [--archive N]] [--mutant]
  python3 checks/realcore-perf.py --selftest

  --baseline  record every number (per move type: min / median / p95 / max) and judge nothing: the lag in numbers (exit 0 when all moves measured)
  --archive N  (gap-engine-state, item f4d7a5c0) after the moves, archive N times with E (Z undoes each, as perf/interaction.py does). With --baseline the
              figures are only dumped; WITHOUT it the archive series is JUDGED: the median and the p95 of `.frame` (under INTERACTION_COMMIT_MS + FRAME_MS) and of
              `.commit` (under INTERACTION_COMMIT_MS), the budgets perf/interaction.py judges its 34 archive actions by, one line per figure
              `ok   archive-frame median 12.3 (budget 66)`; and on each E the row collapse that glides (`mac-main.list.state` collapse_glided) and the focus
              row that moved to the next row (`mac-main.list` rows' `focused`) in the key's own frame, one line each `ok   archive-collapse-glided 20 of 20`.
              The machine-load rule is the others': a load above PERF_LOAD_MAX records and judges nothing. E.g. `--archive 34`.
  --mutant    WHOOSH_CORE_MUTANT=body-fetch-sync in the launch env (the blocking body fetch of before body-r1): on a build that carries
              body-r1 the same budgets must go over, exit 1. A build without body-r1 blocks anyway, so --mutant and the plain run read the same.
Judged as perf/interaction.py judges, read through docs/CONSTANTS.md: every `.frame` under INTERACTION_COMMIT_MS + FRAME_MS, every `.commit`
under INTERACTION_COMMIT_MS, the longest gap under 1.5 x FRAME_MS. A load above PERF_LOAD_MAX is recorded, never judged.
The last line is `N checks, M failed, K not judged`. Exit 1 on any over-budget figure or when nothing was judged, 0 green.
The launch is scripts/launch-one.sh: the rig copy, hidden, the lock held by this run (exit 75 from it = BUSY: wait and retry, never kill)."""
import importlib.util
import json
import os
import statistics
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))

MUTANT = "body-fetch-sync"
MOVES = ["J", "K"] * 16 + ["J"] + ["Enter"] * 33          # the "next" and "open" actions of perf/interaction.py, in its order
KIND = {"J": "next", "K": "next", "Enter": "open", "E": "archive"}
SYNC_LIST_WAIT_S = 120


def _load(name, path):
    spec = importlib.util.spec_from_file_location(name, os.path.join(ROOT, path))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def const(name):
    from lib.fixture import const as c  # noqa: E402
    return c(name)


def launch_env(ports, mutant):
    """The LAUNCH_ENV the rig copy gets: the real core (no WHOOSH_FIXTURE_CORE), the local servers on loopback, hidden, the mutant when asked."""
    extra = ["WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports] + (["WHOOSH_CORE_MUTANT=" + MUTANT] if mutant else [])
    return " ".join(["WHOOSH_RIG_HIDDEN=1"] + extra)


def judge(frames, commits, gaps):
    """Three rows as perf/interaction.py judges them, over len(MOVES) moves. A measure not served or short is NOT JUDGED, never a pass.
    Each row: (label, value text, limit text, over) with over True / False / None."""
    fl, cl = const("FRAME_MS"), const("INTERACTION_COMMIT_MS")

    def row(label, xs, lim):
        if not xs or len(xs) != len(MOVES):
            return (label, "%d of %d measured" % (len(xs or []), len(MOVES)), "< %g" % lim, None)
        return (label, "worst %.1f" % max(xs), "< %g" % lim, max(xs) >= lim)
    return [row("commit ms over %d moves" % len(MOVES), commits, cl),
            row("arrival to first frame ms over %d moves" % len(MOVES), frames, cl + fl),
            row("longest presented-frame gap ms over %d moves" % len(MOVES), gaps, fl * 1.5)]


def verdict(name, refusal, rows):
    """Prints the rows and the count line; returns the exit code (1 on any over, any not judged, nothing judged)."""
    if refusal:
        print("%s: %s" % (name, refusal)); print("0 checks, 0 failed, 1 not judged"); return 1
    bad = [r for r in rows if r[3] is True]
    nj = [r for r in rows if r[3] is None]
    for l, val, lim, over in rows:
        print("%s: %-48s %s (budget %s) %s" % (name, l, val, lim, {True: "OVER", False: "ok", None: "NOT JUDGED"}[over]))
    print("%d checks, %d failed, %d not judged" % (len(rows), len(bad), len(nj)))
    return 1 if bad or nj or not rows else 0


def refusal_for(stamp, load, limit):
    """The reason a run is recorded and not judged, or None. Round 8 (lead 2026-11-13): the load no longer refuses; `loaded` applies it."""
    if stamp is None:
        return "not judged: no build stamp, so the number cannot be tied to a build"
    if limit is None:
        return "not judged: PERF_LOAD_MAX has no number in docs/CONSTANTS.md"
    return None


def loaded(rows, load, limit="ledger"):
    """The shared load rule (journeys/lib/loadgate.py) on judged timing rows: inside the budget passes and names the load, over it above
    PERF_LOAD_MAX is NOT JUDGED with its number. Rows whose limit is not a number (the glide, the focus row) do not depend on the load."""
    from lib import loadgate  # noqa: E402
    out = []
    for l, val, lim, over in rows:
        if lim.startswith("<") and over is not None:
            out.append((l, "%s%s" % (val, loadgate.note(load, limit)), lim, loadgate.gate(over, load, limit)))
        else:
            out.append((l, val, lim, over))
    return out


def without_archive(fr, co, ga, kinds):
    """The three series with the archive moves (baseline-only) taken out, so they can never reach the judge."""
    keep = [i for i, k in enumerate(kinds) if k != "archive"]
    return [fr[i] for i in keep], [co[i] for i in keep], [ga[i] for i in keep]


ARCHIVE_MIN = 5          # fewer archive moves than this is not a series: median and p95 of 3 numbers judge nothing


def _q(xs, f):
    return sorted(xs)[min(len(xs) - 1, int(f * len(xs)))]


def judge_archive(frames, commits, kinds, glided, focus_next, want):
    """The archive series on the real core, judged (gap-engine-state). Rows: (label, value text, limit text, over) as `judge` returns them.
    frames / commits: the whole series in move order with `kinds` beside it; glided / focus_next: one bool (or None) per E. `want`: the E count asked for.
    The figures are the median and the p95 of the E moves against the budgets perf/interaction.py judges archive by (commit < INTERACTION_COMMIT_MS,
    first frame < INTERACTION_COMMIT_MS + FRAME_MS). A series shorter than asked for, or shorter than ARCHIVE_MIN, is NOT JUDGED, never a pass."""
    fl, cl = const("FRAME_MS"), const("INTERACTION_COMMIT_MS")
    fa = [x for x, k in zip(frames, kinds) if k == "archive"]
    ca = [x for x, k in zip(commits, kinds) if k == "archive"]
    rows = []
    for label, xs, lim in (("archive-frame", fa, cl + fl), ("archive-commit", ca, cl)):
        for name, f in (("median", 0.5), ("p95", 0.95)):
            if len(xs) < max(ARCHIVE_MIN, want) or not all(x == x and abs(x) != float("inf") for x in xs):
                rows.append(("%s %s" % (label, name), "%d of %d measured" % (len(xs), want), "< %g" % lim, None)); continue
            v = statistics.median(xs) if f == 0.5 else _q(xs, f)
            rows.append(("%s %s" % (label, name), "%.1f" % v, "< %g" % lim, v >= lim))
    for label, xs in (("archive-collapse-glided", glided), ("archive-focus-next-row", focus_next)):
        if len(xs) < max(ARCHIVE_MIN, want) or any(x is None for x in xs):
            rows.append((label, "%d of %d read" % (sum(1 for x in xs if x is not None), want), "all", None))
        else:
            rows.append((label, "%d of %d" % (sum(1 for x in xs if x), len(xs)), "all", not all(xs)))
    return rows


def verdict_archive(name, refusal, rows):
    """Prints one line per judged figure in the style `ok   archive-frame median 12.3 (budget 66)`; the count line; exit 1 on any over or not judged."""
    if refusal:
        print("%s: %s" % (name, refusal)); print("0 checks, 0 failed, 1 not judged"); return 1
    for l, val, lim, over in rows:
        word = {True: "OVER", False: "ok", None: "NOT JUDGED"}[over]
        print("%-4s %s %s (budget %s)" % (word, l, val, lim.lstrip("< ")))
    bad = sum(1 for r in rows if r[3] is True); nj = sum(1 for r in rows if r[3] is None)
    print("%d checks, %d failed, %d not judged" % (len(rows), bad, nj))
    return 1 if bad or nj or not rows else 0


def dump_phases(phases, kinds):
    """Per move type and phase: median and max of the app's own split of the main thread's time (`measure mac-main.interaction.phases`), for the
    phases whose largest value reaches 0.5 ms. Information, not a verdict."""
    names = []
    for p in phases or []:
        for n in (p or {}):
            if n not in names:
                names.append(n)
    for k in ("next", "open", "archive"):
        served = [p for p, kk in zip(phases or [], kinds) if kk == k and p]
        for n in names:
            v = [float(p.get(n, 0.0)) for p in served]
            if v and max(v) >= 0.5:
                print("realcore-perf: dump phase %-7s %-38s n=%3d median %7.2f max %7.2f" % (k, n, len(v), statistics.median(v), max(v)))


def dump(frames, commits, gaps, kinds):
    """Per move type: min / median / p95 / max of the three measures. Information, not a verdict."""
    q = lambda xs, f: sorted(xs)[min(len(xs) - 1, int(f * len(xs)))]
    for name, xs in (("frame", frames), ("commit", commits), ("gap", gaps)):
        for k in ("next", "open", "archive", None):
            v = [x for x, kk in zip(xs, kinds) if x is not None and (k is None or kk == k)]
            if v:
                print("realcore-perf: dump %-6s %-5s n=%3d min %7.1f median %7.1f p95 %7.1f max %7.1f" % (name, k or "all", len(v), min(v), statistics.median(v), q(v, 0.95), max(v)))
    print("realcore-perf: raw " + json.dumps([[k, round(f, 1), round(c, 1), None if g is None else round(g, 1)] for k, f, c, g in zip(kinds, frames, commits, gaps)]))


# ---------------------------------------------------------------- the inner drive (runs inside launch-one, with the socket)
def id_below_focus(c):
    """The id of the row after the focused one in `mac-main.list`, or None when it cannot be read (the focused row is the last, or the list is unreadable)."""
    try:
        rows = c.rows("mac-main.list")
    except Exception:
        return None
    i = next((k for k, r in enumerate(rows) if r.get("focused")), None)
    return rows[i + 1].get("id") if i is not None and i + 1 < len(rows) else None


def probe_after_archive(c):
    """(did the leaving row glide, the id of the focused row) read right after the E: `mac-main.list.state` collapse_glided and the rows' `focused`."""
    try:
        glided = c.rows("mac-main.list.state")[0].get("collapse_glided")
        rows = c.rows("mac-main.list")
        focused = next((r.get("id") for r in rows if r.get("focused")), None)
        return (glided if isinstance(glided, bool) else None), focused
    except Exception:
        return None, None


def drive(n_messages, archive=0):
    """Add the local account through the sheet, Done, wait for the list; then the moves. Returns the answer dict."""
    rc = _load("realcore_check_for_perf", "checks/realcore-check.py")
    from lib.ctl import Ctl, Unreadable  # noqa: E402
    import realcore_mailfake as fake  # noqa: E402
    A = rc.A
    c = Ctl().connect()
    c.click(A + ".open")
    c.click(A + ".address"); c.type(rc.CHECK_ADDRESS)
    c.click(A + ".password"); c.type(fake.PASSWORD)
    c.click(A + ".continue")
    t0 = time.time()
    while time.time() - t0 < rc.DETECT_WAIT_S:
        st = c.rows(A + ".state")
        if st and st[0].get("stage") not in ("detecting", "entry"):
            break
        time.sleep(0.3)
    c.click(A + ".add")
    stage = None
    t0 = time.time()
    while time.time() - t0 < rc.DETECT_WAIT_S:
        st = c.rows(A + ".state")
        stage = st[0].get("stage") if st else None
        if stage in ("added", "summary"):
            break
        time.sleep(0.3)
    answer = {"stage": stage}
    try:
        c.click(A + ".done")
    except Exception as e:
        answer["done"] = str(e)
    t0 = time.time()
    rows = []
    while time.time() - t0 < SYNC_LIST_WAIT_S:
        try:
            rows = c.rows("mac-main.list")
        except Exception:
            rows = []
        if len(rows) >= n_messages:
            break
        time.sleep(0.5)
    answer["list_rows"], answer["list_seconds"] = len(rows), round(time.time() - t0, 1)
    time.sleep(1.0)   # the sync's last writes land before the first move is timed
    frames, commits, gaps, kinds, phases = [], [], [], [], []
    # REALCORE_PERF_WALK=N: walk DOWN N different messages and open each one for the first time (J, Enter, Escape),
    # the owner's real use; the default moves revisit the same two rows, whose bodies are stored after the first open.
    walk = int(os.environ.get("REALCORE_PERF_WALK", "0") or 0)
    moves = (["J", "Enter"] * walk) if walk else MOVES
    glided, focus_next, next_id = [], [], None
    for chord in moves + ["E"] * archive:
        try:
            if chord == "E":
                next_id = id_below_focus(c)     # the row the focus must land on when the focused row leaves
            c.key(chord)
            frames.append(float(c.measure("mac-main.interaction.frame")["ms"]))
            commits.append(float(c.measure("mac-main.interaction.commit")["ms"]))
            try:
                gaps.append(float(c.measure("mac-main.interaction.gap")["ms"]))
            except Unreadable:
                gaps.append(None)
            kinds.append(KIND[chord])
            try:      # where the main thread's time went, read after the frame (the app's own split, as perf/interaction.py --dump reads it)
                phases.append(c.measure("mac-main.interaction.phases")["phases"])
            except (Unreadable, KeyError, TypeError):
                phases.append(None)
            # The way back (Escape after Enter, Z after E) is waited for until ITS frame is out, as perf/interaction.py does (its round 3): sent right
            # behind it, the next judged key waited in `arrival.queue` for the way back's own work (main189, 33 opens: arrival.queue median 23.6 ms of a
            # commit median 58.5 ms, the app's own dispatch and update 34.6 ms).
            if chord == "Enter":
                c.key("Escape")
                try: c.measure("mac-main.interaction.frame")
                except (Unreadable, KeyError, TypeError): pass
            if chord == "E":
                after = probe_after_archive(c)
                glided.append(after[0]); focus_next.append(None if after[1] is None or next_id is None else after[1] == next_id)
                c.key("Z")                      # undo, so the list never empties (as perf/interaction.py does)
                try: c.measure("mac-main.interaction.frame")
                except (Unreadable, KeyError, TypeError): pass
        except (Unreadable, KeyError, TypeError, ValueError):
            pass
    answer.update(frames=frames, commits=commits, gaps=gaps, kinds=kinds, glided=glided, focus_next=focus_next, phases=phases)
    return answer


# ---------------------------------------------------------------- the outer run
def run(bundle, latency_ms, messages, baseline, mutant, archive=0):
    import realcore_mailfake as fake
    iplist = importlib_interaction()
    stamp = iplist.stamp(bundle)
    limit = iplist.load_limit()
    load0 = os.getloadavg()[0]
    print("realcore-perf: build stamp: %s" % (stamp or "UNKNOWN (no stamp in its Info.plist)"))
    print("realcore-perf: latency %d ms before each SELECT and FETCH, %d messages, mode %s%s" % (latency_ms, messages, "baseline" if baseline else "judge", ", mutant " + MUTANT if mutant else ""))
    try:
        mail, imap, smtp = fake.start(0, 0, inbox=messages, latency_ms=latency_ms, rich=True)
    except OSError as e:
        print("the local servers did not start: %s" % e); print("1 checks, 0 failed, 1 not judged"); return 1
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=launch_env(ports, mutant))
    env.pop("WHOOSH_FIXTURE_CORE", None)
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner %d %d' % (messages, archive), os.path.abspath(__file__)]
    r = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True)
    log = list(mail.log)
    imap.shutdown(); smtp.shutdown()
    load1 = os.getloadavg()[0]
    load = max(load0, load1)
    print("realcore-perf: machine load (1 min) before %.2f after %.2f, limit PERF_LOAD_MAX: %s" % (load0, load1, limit if limit is not None else "NONE"))
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:]); print("launcher exit %d" % r.returncode)
        if r.returncode == 75:
            print("BUSY: another drive holds the rig lock; wait and retry, never kill")
        print("0 checks, 0 failed, 1 not judged"); return 1 if not baseline else 1
    a = json.loads(line[len("ANSWER "):])
    sel = sum(1 for l in log if l.startswith("SELECT"))
    fet = sum(1 for l in log if l.startswith("UID FETCH") or l.startswith("FETCH"))
    print("realcore-perf: launcher exit %d; account stage %r; list %d of %d rows after %ss; the server saw %d SELECT and %d FETCH in all; %s"
          % (r.returncode, a.get("stage"), a.get("list_rows"), messages, a.get("list_seconds"), sel, fet, next((l for l in out.splitlines() if l.startswith("instances:")), "")))
    fr, co, ga, kinds = a.get("frames", []), a.get("commits", []), a.get("gaps", []), a.get("kinds", [])
    dump(fr, co, ga, kinds)
    dump_phases(a.get("phases"), kinds)
    # the archive sequence (E then Z): dumped above; with --baseline it is never judged, without it (--archive N) it is judged below (gap-engine-state)
    afr, aco, akinds = fr, co, kinds
    fr, co, ga = without_archive(fr, co, ga, kinds)
    if baseline:
        ok = len(fr) == len(MOVES) and len(co) == len(MOVES)
        print("realcore-perf: BASELINE: %d of %d moves recorded, nothing judged%s" % (len(fr), len(MOVES), "; load %.2f is above PERF_LOAD_MAX (recorded all the same)" % load if limit is not None and load > limit else ""))
        print("%d checks, 0 failed, 0 not judged (baseline)" % (len(MOVES) if ok else 0)); return 0 if ok else 1
    gaps = ga if ga and all(g is not None for g in ga) else None
    refusal = refusal_for(stamp, load, limit)
    code = verdict("realcore-perf", refusal, loaded(judge(fr, co, gaps), load))
    if archive:
        code = max(code, verdict_archive("realcore-perf", refusal, loaded(judge_archive(afr, aco, akinds, a.get("glided", []), a.get("focus_next", []), archive), load)))
    return code


def importlib_interaction():
    return _load("perf_interaction_for_realcore", "perf/interaction.py")


# ---------------------------------------------------------------- selftest
def selftest():
    import imaplib
    import realcore_mailfake as fake
    fl, cl = const("FRAME_MS"), const("INTERACTION_COMMIT_MS")
    n = len(MOVES)
    ok = [1.0] * n
    with_last = lambda x: ok[:n - 1] + [x]
    cases = [("all under", judge(ok, ok, ok), [False, False, False]),
             ("one commit at budget", judge(ok, with_last(cl), ok), [True, False, False]),
             ("one first frame over commit budget + one refresh", judge(with_last(cl + fl), ok, ok), [False, True, False]),
             ("a first frame just under it", judge(with_last(cl + fl - 1), ok, ok), [False, False, False]),
             ("a dropped frame (33 ms gap)", judge(ok, ok, with_last(33.0)), [False, False, True]),
             ("a blocked body fetch (700 ms frame, 600 ms commit)", judge(with_last(700.0), with_last(600.0), ok), [True, True, False]),
             ("gap measure not served", judge(ok, ok, None), [False, False, None]),
             ("one move short of %d" % n, judge(ok, ok[:n - 1], ok), [None, False, False]),
             ("nothing measured", judge([], [], []), [None, None, None])]
    bad = total = 0
    for name, rows, want in cases:
        got = [x[3] for x in rows]; good = got == want; bad += not good; total += 1
        print("%-4s %-60s over=%s" % ("ok" if good else "BAD", name, got))
    # the exit codes
    def code(refusal, rows):
        import io, contextlib
        with contextlib.redirect_stdout(io.StringIO()):
            return verdict("t", refusal, rows)
    for name, got, want in [("green set exits 0", code(None, judge(ok, ok, ok)), 0),
                            ("one over-budget frame exits 1", code(None, judge(with_last(500.0), ok, ok)), 1),
                            ("nothing judged exits 1", code(None, judge([], [], [])), 1),
                            ("no rows at all exits 1", code(None, []), 1),
                            ("load over the cap: a green set still exits 0 (round 8)", code(refusal_for("stamp", 20.0, 12.0), loaded(judge(ok, ok, ok), 20.0, 12.0)), 0),
                            ("load over the cap: an over-budget frame is NOT JUDGED (exit 1)", code(refusal_for("stamp", 20.0, 12.0), loaded(judge(with_last(500.0), ok, ok), 20.0, 12.0)), 1)]:
        good = got == want; bad += not good; total += 1
        print("%-4s %-60s exit %d" % ("ok" if good else "BAD", name, got))
    for name, got, want in [("load at the cap is judged", refusal_for("s", 12.0, 12.0), None),
                            ("load over the cap no longer refuses the run", refusal_for("s", 12.5, 12.0), None),
                            ("load: inside the budget on a loaded Mac is ok and names the load", [(r[3], "machine load 20.0" in r[1]) for r in loaded(judge(ok, ok, ok), 20.0, 12.0)], [(False, True)] * 3),
                            ("load: over the budget on a loaded Mac is NOT JUDGED", [r[3] for r in loaded(judge(with_last(500.0), ok, ok), 20.0, 12.0)], [False, None, False]),
                            ("load: over the budget at the cap is OVER", [r[3] for r in loaded(judge(with_last(500.0), ok, ok), 12.0, 12.0)], [False, True, False]),
                            ("no stamp is not judged", refusal_for(None, 1.0, 12.0) is not None, True),
                            ("no limit in the ledger is not judged", refusal_for("s", 1.0, None) is not None, True)]:
        good = got == want; bad += not good; total += 1
        print("%-4s %-60s %r" % ("ok" if good else "BAD", name, got))
    # the archive figures never reach the judge: a 900 ms archive among green moves leaves the judged rows green
    k = ["next"] * (n - 1) + ["archive"]
    f2, c2, g2 = without_archive(ok[:n - 1] + [900.0], ok[:n - 1] + [900.0], ok[:n - 1] + [900.0], k)
    good = len(f2) == n - 1 and max(f2) == 1.0 and [x[3] for x in judge(ok, ok, ok)] == [False] * 3
    bad += not good; total += 1
    print("%-4s archive figures are removed before judging (%d kept of %d)" % ("ok" if good else "BAD", len(f2), n))
    # gap-engine-state: the archive series is judged (median and p95 of frame and commit, the glide, the focus row), a short or unread series is not judged
    na = 20
    ak = ["archive"] * na
    fa_ok, ca_ok, g_ok = [10.0] * na, [5.0] * na, [True] * na
    def arch(fr_, co_, gl, fo, kinds=ak, want=na):
        return [x[3] for x in judge_archive(fr_, co_, kinds, gl, fo, want)]
    # row order: frame median, frame p95, commit median, commit p95, glided, focus
    arch_cases = [("archive: all under budget", arch(fa_ok, ca_ok, g_ok, g_ok), [False] * 6),
                  ("archive: a median frame over %g ms" % (cl + fl), arch([cl + fl] * na, ca_ok, g_ok, g_ok), [True, True, False, False, False, False]),
                  ("archive: one slow frame in 20 moves the p95 only (p95 index 19)", arch(fa_ok[:-1] + [cl + fl + 1], ca_ok, g_ok, g_ok), [False, True, False, False, False, False]),
                  ("archive: a median commit at the budget", arch(fa_ok, [cl] * na, g_ok, g_ok), [False, False, True, True, False, False]),
                  ("archive: one row did not glide", arch(fa_ok, ca_ok, g_ok[:-1] + [False], g_ok), [False, False, False, False, True, False]),
                  ("archive: the focus did not land on the next row once", arch(fa_ok, ca_ok, g_ok, g_ok[:-1] + [False]), [False, False, False, False, False, True]),
                  ("archive: a glide that could not be read is not judged", arch(fa_ok, ca_ok, g_ok[:-1] + [None], g_ok), [False, False, False, False, None, False]),
                  ("archive: 3 of 20 moves measured is not judged", arch([1.0] * 3, [1.0] * 3, [True] * 3, [True] * 3, ["archive"] * 3), [None] * 6),
                  ("archive: a NaN frame is not judged", arch(fa_ok[:-1] + [float("nan")], ca_ok, g_ok, g_ok), [None, None, False, False, False, False]),
                  ("archive: the next/open moves never count (20 archive among 66 moves)", arch([900.0] * 66 + fa_ok, [900.0] * 66 + ca_ok, g_ok, g_ok, ["next"] * 66 + ak), [False] * 6)]
    for name, got, want in arch_cases:
        good = got == want; bad += not good; total += 1
        print("%-4s %-70s over=%s" % ("ok" if good else "BAD", name, got))
    import io, contextlib
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        c_ok = verdict_archive("t", None, judge_archive(fa_ok, ca_ok, ak, g_ok, g_ok, na))
        c_bad = verdict_archive("t", None, judge_archive([cl + fl] * na, ca_ok, ak, g_ok, g_ok, na))
    lines = buf.getvalue().splitlines()
    good = (c_ok, c_bad) == (0, 1) and any(l.startswith("ok   archive-frame median 10.0 (budget %g)" % (cl + fl)) for l in lines) and any(l.startswith("OVER archive-frame median") for l in lines)
    bad += not good; total += 1
    print("%-4s archive: one line per figure, `ok   archive-frame median 10.0 (budget %g)`, exit 0 green and 1 over" % ("ok" if good else "BAD", cl + fl))
    # the mutant flag reaches LAUNCH_ENV, and only when asked; the real core is asked for (no fixture core)
    on, off = launch_env("1:2", True), launch_env("1:2", False)
    for name, good in [("mutant flag in LAUNCH_ENV when --mutant", "WHOOSH_CORE_MUTANT=body-fetch-sync" in on.split()),
                       ("mutant flag absent without --mutant", "WHOOSH_CORE_MUTANT" not in off),
                       ("the local servers' ports reach LAUNCH_ENV", "WHOOSH_CORE_CHECK_SERVERS=1:2" in on.split()),
                       ("the real core: no fixture core in LAUNCH_ENV", "WHOOSH_FIXTURE_CORE=1" not in on.split()),
                       ("hidden rig in LAUNCH_ENV", "WHOOSH_RIG_HIDDEN=1" in off.split())]:
        bad += not good; total += 1
        print("%-4s %s" % ("ok" if good else "BAD", name))
    # the fake's latency option: a FETCH and a SELECT wait, nothing else does; at 0 neither does
    def timed(latency_ms):
        mail, imap, smtp = fake.start(0, 0, inbox=5, latency_ms=latency_ms, rich=True)
        try:
            c = imaplib.IMAP4("127.0.0.1", imap.server_address[1])
            c.login(fake.ADDRESS, fake.PASSWORD)
            t = {}
            for label, fn in (("list", lambda: c.list()), ("noop", lambda: c.noop()), ("select", lambda: c.select("INBOX")),
                              ("fetch", lambda: c.uid("FETCH", "2", "(BODY.PEEK[])")), ("search", lambda: c.uid("SEARCH", "UID 1:*"))):
                t0 = time.time(); res = fn(); t[label] = (time.time() - t0) * 1000.0
                if label == "fetch":
                    t["fetch_bytes"] = b"".join(x[1] for x in res[1] if isinstance(x, tuple))
            c.logout()
            return t
        finally:
            imap.shutdown(); smtp.shutdown()
    slow, fast = timed(200), timed(0)
    for name, good in [("a SELECT with --latency-ms 200 answers after >= 200 ms (%.0f)" % slow["select"], slow["select"] >= 200),
                       ("a FETCH with --latency-ms 200 answers after >= 200 ms (%.0f)" % slow["fetch"], slow["fetch"] >= 200),
                       ("LIST, NOOP and SEARCH are not slowed (%.0f %.0f %.0f ms)" % (slow["list"], slow["noop"], slow["search"]), max(slow["list"], slow["noop"], slow["search"]) < 100),
                       ("at 0 ms a SELECT and a FETCH answer under 50 ms (%.0f %.0f)" % (fast["select"], fast["fetch"]), max(fast["select"], fast["fetch"]) < 50),
                       ("the fetched body is multipart/alternative, quoted-printable", b"multipart/alternative" in slow["fetch_bytes"] and b"quoted-printable" in slow["fetch_bytes"] and b"text/html" in slow["fetch_bytes"])]:
        bad += not good; total += 1
        print("%-4s %s" % ("ok" if good else "BAD", name))
    # --messages: N INBOX messages seeded; the default is what slice 2 seeds (3)
    mail, imap, smtp = fake.start(0, 0, inbox=60, rich=True)
    try:
        c = imaplib.IMAP4("127.0.0.1", imap.server_address[1]); c.login(fake.ADDRESS, fake.PASSWORD)
        typ, data = c.select("INBOX"); n60 = int(data[0])
        c.logout()
    finally:
        imap.shutdown(); smtp.shutdown()
    mail3, imap3, smtp3 = fake.start(0, 0)
    n3 = len(mail3.boxes["INBOX"]); imap3.shutdown(); smtp3.shutdown()
    for name, good in [("--messages 60 seeds 60 INBOX messages (%d)" % n60, n60 == 60), ("the default seeds 3 (%d)" % n3, n3 == 3)]:
        bad += not good; total += 1
        print("%-4s %s" % ("ok" if good else "BAD", name))
    print("%d checks, %d failed, 0 not judged" % (total, bad)); return 1 if bad else 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    if "--inner" in argv:
        i = argv.index("--inner"); print("ANSWER " + json.dumps(drive(int(argv[i + 1]), int(argv[i + 2])))); return 0
    if not argv or not os.path.isdir(argv[0]):
        print("usage: realcore-perf.py <bundle> [--latency-ms N] [--messages N] [--baseline] [--mutant] | --selftest"); return 2
    opt = lambda name, d: int(argv[argv.index(name) + 1]) if name in argv else d
    return run(os.path.abspath(argv[0]), opt("--latency-ms", 300), opt("--messages", 60), "--baseline" in argv, "--mutant" in argv, opt("--archive", 0))


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
