#!/usr/bin/env python3
"""Interaction budgets (fn-perf-mac): 100 archive/next/open actions on the running rig copy (docs/CONSTANTS.md). Definitions (lead's ruling, 2026-11-09):
  FRAME_MS             the longest gap between two presented frames DURING the interaction (the dropped-frame measure): `measure mac-main.interaction.gap`,
                       judged under 1.5 x FRAME_MS (24 ms): a perfect 60 Hz run gaps 16.7 ms, a dropped frame gaps 33 ms (lead's ruling, 2026-11-09)
  INTERACTION_COMMIT_MS the commit (`.commit`) must be under it; the arrival to the first presented frame (`.frame`) must be under it plus one refresh,
                       one refresh being FRAME_MS (the ledger's 16 ms; one refresh of a 60 Hz display is 16.7 ms)

  python3 perf/interaction.py --bundle <App.app>      run under: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 perf/interaction.py --bundle <App.app>'
  python3 perf/interaction.py --selftest

The 100 actions: 34 archive (E, then Z so the list never empties), 33 next (J, K alternating), 33 open (Enter, Escape).
After each action: `measure mac-main.interaction.frame` and `measure mac-main.interaction.commit` (ms). A measure the app does
not serve is NOT JUDGED. Exit 1 on any frame >= FRAME_MS or commit >= INTERACTION_COMMIT_MS, or when nothing was judged.
"""
import json, os, plistlib, re, statistics, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable      # noqa: E402
from lib.fixture import const                            # noqa: E402
from lib import loadgate                                 # noqa: E402  (the one load rule, journeys/lib/loadgate.py)


def stamp(bundle):
    """The build stamp of the bundle under test (the five Info.plist keys of scripts/build.sh)."""
    try:
        with open(os.path.join(bundle, "Contents", "Info.plist"), "rb") as f:
            p = plistlib.load(f)
        return "label=%s commit=%s built=%s dirty=%s" % (p["NexusBuildLabel"], p["NexusBuildCommit"], p["NexusBuiltAt"], p.get("NexusBuildDirtyFiles", "?"))
    except (OSError, KeyError, ValueError):
        return None


def load_limit():
    """PERF_LOAD_MAX from docs/CONSTANTS.md; env PERF_LOAD_MAX overrides. None when the ledger holds no number."""
    if os.environ.get("PERF_LOAD_MAX"):
        return float(os.environ["PERF_LOAD_MAX"])
    try:
        return float(const("PERF_LOAD_MAX"))
    except KeyError:
        return None


def header(name, bundle):
    st = stamp(bundle) if bundle else None
    ld = os.getloadavg()[0]
    lim = load_limit()
    print("%s: build stamp: %s" % (name, st or "UNKNOWN (no --bundle, or no stamp in its Info.plist)"))
    print("%s: machine load (1 min): %.2f, limit PERF_LOAD_MAX: %s" % (name, ld, lim if lim is not None else "NONE (docs/CONSTANTS.md holds no number)"))
    if st is None:
        return "not judged: no build stamp, so the number cannot be tied to a build"
    if lim is None:
        return "not judged: PERF_LOAD_MAX has no number in docs/CONSTANTS.md"
    if ld > lim:   # lead 2026-11-13: the run goes on; a timing inside its budget passes, one over it is NOT JUDGED (loadgate.gate)
        print("%s: machine load %.2f is above PERF_LOAD_MAX %s: the run goes on; inside a budget is a PASS, over it is NOT JUDGED" % (name, ld, lim))
    return None


def finish(name, refusal, verdicts):
    """verdicts: list of (label, value, limit, over?) . Exit 1 when any is over, or when nothing could be judged."""
    if refusal:
        print("%s: %s" % (name, refusal)); print("0 checks, 0 failed, 1 not judged"); return 1
    bad = [v for v in verdicts if v[3] is True]
    nj = [v for v in verdicts if v[3] is None]
    for l, val, lim, over in verdicts:
        print("%s: %-34s %s (budget %s) %s" % (name, l, val, lim, {True: "OVER", False: "ok", None: "NOT JUDGED"}[over]))
    print("%d checks, %d failed, %d not judged" % (len(verdicts), len(bad), len(nj)))
    return 1 if bad or nj or not verdicts else 0


def connect():
    try:
        return Ctl().connect()
    except CouldNotDrive as e:
        print("could not drive: %s" % e); return None

ACTIONS = ["E"] * 34 + ["J", "K"] * 16 + ["J"] + ["Enter"] * 33


def judge(frames, commits, gaps=None, load=0.0):
    """Three rows: commit < INTERACTION_COMMIT_MS; arrival to first frame < INTERACTION_COMMIT_MS + FRAME_MS (one refresh); longest presented-frame gap < 1.5 x FRAME_MS.
    A measure the app does not serve (gaps is None or short) is NOT JUDGED, never a pass."""
    fl, cl = const("FRAME_MS"), const("INTERACTION_COMMIT_MS")
    def row(label, xs, lim):
        if not xs or len(xs) != len(ACTIONS):
            return (label, "%d of %d measured" % (len(xs or []), len(ACTIONS)), "< %g" % lim, None)
        if not all(x == x and abs(x) != float("inf") for x in xs):
            return (label, "not measured: a reading was not a number", "< %g" % lim, None)
        if max(xs) <= 0:
            return (label, "not measured: all %d readings were %g ms, which is not a reading of a running app" % (len(xs), max(xs)), "< %g" % lim, None)
        return (label, "worst %.1f" % max(xs), "< %g" % lim, max(xs) >= lim)
    rows = [row("commit ms over 100 actions", commits, cl), row("arrival to first frame ms over 100 actions", frames, cl + fl),
            row("longest presented-frame gap ms over 100 actions", gaps, fl * 1.5)]
    # the load rule (journeys/lib/loadgate.py): `load` is the machine load at the end of the run; 0 (the default) is a quiet machine
    return [(l, v + (loadgate.note(load) if o is not None else ""), lim, loadgate.gate(o, load)) for l, v, lim, o in rows]


def worst_actions(commits, kinds, n=3):
    """The n slowest commits as (action number from 1, kind, ms): which action type the over-budget commits belong to (the item 8736d094 note is written from these rows)."""
    order = sorted(range(len(commits)), key=lambda i: -commits[i])[:n]
    return [(i + 1, kinds[i], commits[i]) for i in order]


def dump(frames, commits, kinds):
    """Per action type: min / median / p95 / max of the two measures (information, not a verdict). Raw rows: one per action."""
    q = lambda xs, f: sorted(xs)[min(len(xs) - 1, int(f * len(xs)))]
    for name, xs in (("frame", frames), ("commit", commits)):
        for k in ("archive", "next", "open", None):
            v = [x for x, kk in zip(xs, kinds) if k is None or kk == k]
            if v:
                print("interaction: dump %-6s %-8s n=%3d min %6.1f median %6.1f p95 %6.1f max %6.1f" % (name, k or "all", len(v), min(v), statistics.median(v), q(v, 0.95), max(v)))
    for i, k, ms in worst_actions(commits, kinds):
        print("interaction: dump worst commit: action %d (%s) %.1f ms" % (i, k, ms))
    print("interaction: raw " + json.dumps([[k, round(f, 1), round(c, 1)] for k, f, c in zip(kinds, frames, commits)]))


def phase_lines(phases, kinds):
    """`--dump` rows from `measure mac-main.interaction.phases` (one {name: ms} per action, or None where the app did not serve it): one line per
    action kind and phase with the median and max over that kind's served actions, in the order the app names the phases. Information, not a verdict."""
    names = []
    for p in phases:
        for n in (p or {}):
            if n not in names:
                names.append(n)
    out = []
    for k in ("archive", "next", "open", "undo", "close"):
        if k not in kinds:
            continue
        served = [p for p, kk in zip(phases, kinds) if kk == k and p is not None]
        if not served:
            out.append("interaction: dump phase %-7s not served (0 of %d actions)" % (k, sum(1 for kk in kinds if kk == k)))
            continue
        for n in names:
            v = [float(p.get(n, 0.0)) for p in served]
            out.append("interaction: dump phase %-7s %-26s n=%3d median %7.2f max %7.2f" % (k, n, len(v), statistics.median(v), max(v)))
    return out


def worst_phase_lines(phases, kinds, name="update.queue", n=3):
    """The n actions with the largest `name` phase: action number from 1, kind, ms, and that action's next-largest phase (round 4)."""
    rows = [(i + 1, k, float(p.get(name, 0.0)), p) for i, (p, k) in enumerate(zip(phases, kinds)) if p is not None]
    out = []
    for num, k, ms, p in sorted(rows, key=lambda r: -r[2])[:n]:
        others = sorted(((float(v), kk) for kk, v in p.items() if kk not in (name, "update", "dispatch")), reverse=True)
        out.append("interaction: dump worst %s: action %d (%s) %.1f ms; its largest other phase %s %.1f ms" % (name, num, k, ms, others[0][1] if others else "-", others[0][0] if others else 0.0))
    return out


def gap_lines(gaps, kinds, phases=None):
    """Per kind: median and max of the longest presented-frame gap; then the action that holds the worst gap, its kind and its largest phase."""
    out = []
    if not gaps or len(gaps) != len(kinds):
        return ["interaction: dump gap not measured for every action (%d of %d)" % (len(gaps or []), len(kinds))]
    for k in ("archive", "next", "open"):
        v = [g for g, kk in zip(gaps, kinds) if kk == k]
        if v:
            out.append("interaction: dump gap   %-8s n=%3d median %6.1f max %6.1f" % (k, len(v), statistics.median(v), max(v)))
    i = max(range(len(gaps)), key=lambda j: gaps[j])
    p = (phases[i] if phases and len(phases) == len(gaps) else None) or {}
    top = sorted(((float(v), kk) for kk, v in p.items() if kk not in ("update", "dispatch")), reverse=True)
    out.append("interaction: dump worst gap: action %d (%s) %.1f ms; its largest phase %s" % (i + 1, kinds[i], gaps[i], ("%s %.1f ms" % (top[0][1], top[0][0])) if top else "not served"))
    return out


def selftest():
    fl, cl = const("FRAME_MS"), const("INTERACTION_COMMIT_MS")
    ok = [1.0] * len(ACTIONS)
    def with_last(x): return ok[:99] + [x]
    cases = [("all under", judge(ok, ok, ok), [False, False, False]),
             ("one commit at budget", judge(ok, with_last(cl), ok), [True, False, False]),
             ("commit 50 + 10 ms to the first frame is fine (one refresh allowed)", judge(with_last(cl + fl - 1), ok, ok), [False, False, False]),
             ("arrival to first frame at commit budget + one refresh", judge(with_last(cl + fl), ok, ok), [False, True, False]),
             ("a perfect 60 Hz gap (16.7 ms) is fine", judge(ok, ok, with_last(16.7)), [False, False, False]),
             ("one gap between presented frames at 1.5 x FRAME_MS", judge(ok, ok, with_last(fl * 1.5)), [False, False, True]),
             ("a dropped frame (33 ms gap)", judge(ok, ok, with_last(33.0)), [False, False, True]),
             ("old reading: a 24 ms first frame no longer fails on its own", judge(with_last(24.0), ok, ok), [False, False, False]),
             ("gap measure not served", judge(ok, ok, None), [False, False, None]),
             ("99 of 100 commits measured", judge(ok, ok[:99], ok), [None, False, False]),
             ("every commit read 0 ms (main83: a 0 reading passed)", judge(ok, [0.0] * 100, ok), [None, False, False]),
             ("every gap read 0 ms", judge(ok, ok, [0.0] * 100), [False, False, None]),
             ("a NaN frame reading", judge(with_last(float("nan")), ok, ok), [False, None, False])]
    bad = 0
    kinds = ["archive"] * 34 + ["next"] * 33 + ["open"] * 33
    wa = worst_actions([1.0] * 70 + [90.0] + [1.0] * 9 + [40.0] + [1.0] * 19, kinds)
    good = wa[0] == (71, "open", 90.0) and wa[1] == (81, "open", 40.0) and len(wa) == 3
    bad += not good
    print("%-4s %-66s %s" % ("ok" if good else "BAD", "the worst commits name their action number and kind", wa))
    for name, v, want in cases:
        got = [x[3] for x in v]; good = got == want; bad += not good
        print("%-4s %-66s over=%s" % ("ok" if good else "BAD", name, got))
    # the phase lines: archive actions 1..3 with engine 2, 4, 30 ms and update 10, 10, 10; next served for none; open for one action
    ph = [{"WhooshCore.run": 2.0, "update": 10.0}, {"WhooshCore.run": 4.0, "update": 10.0}, {"WhooshCore.run": 30.0, "update": 10.0},
          None, None, {"WhooshCore.run": 1.0, "ReaderThread.make": 6.5, "update": 3.0}]
    lines = phase_lines(ph, ["archive", "archive", "archive", "next", "next", "open"])
    pcases = [("a phase line per kind and phase, in the app's order",
               [l.split()[3:5] for l in lines if " n=" in l] == [["archive", "WhooshCore.run"], ["archive", "update"], ["archive", "ReaderThread.make"],
                                                                 ["open", "WhooshCore.run"], ["open", "update"], ["open", "ReaderThread.make"]]),
              ("the archive engine phase reads median 4.00 max 30.00 over n=3",
               any("archive WhooshCore.run" in " ".join(l.split()) and "n=  3 median    4.00 max   30.00" in l for l in lines)),
              ("a phase an action did not run reads 0 (open's make median 6.50; archive's make 0.00)",
               any("open    ReaderThread.make" in l and "median    6.50 max    6.50" in l for l in lines)
               and any("archive ReaderThread.make" in l and "median    0.00 max    0.00" in l for l in lines)),
              ("a kind with no served phases says so, never a 0 line", "interaction: dump phase next    not served (0 of 2 actions)" in lines),
              ("the way back prints as its own kind (undo), and a kind with no action prints nothing",
               phase_lines([{"WhooshCore.run": 1.5}], ["undo"]) == ["interaction: dump phase %-7s %-26s n=%3d median %7.2f max %7.2f" % ("undo", "WhooshCore.run", 1, 1.5, 1.5)])]
    for name, good in pcases:
        bad += not good
        print("%-4s %-66s" % ("ok" if good else "BAD", name))
    wq = worst_phase_lines([{"update.queue": 3.0, "WhooshCore.run": 1.0}, None, {"update.queue": 50.2, "InboxListState.say": 9.0, "update": 60.0}],
                           ["archive", "next", "open"], n=2)
    gl = gap_lines([10.0, 42.1, 5.0], ["archive", "next", "open"], [{"update.queue": 1.0}, {"update.queue": 30.0, "update": 33.0}, None])
    pcases += [("the worst update.queue actions name their number, kind and next phase",
                wq == ["interaction: dump worst update.queue: action 3 (open) 50.2 ms; its largest other phase InboxListState.say 9.0 ms",
                       "interaction: dump worst update.queue: action 1 (archive) 3.0 ms; its largest other phase WhooshCore.run 1.0 ms"]),
               ("the worst gap names its action, kind and largest phase", gl[-1] == "interaction: dump worst gap: action 2 (next) 42.1 ms; its largest phase update.queue 30.0 ms"
                and gl[0].startswith("interaction: dump gap   archive  n=  1")),
               ("gaps short of the actions say so", gap_lines([1.0], ["archive", "next"]) == ["interaction: dump gap not measured for every action (1 of 2)"])]
    for name, good in pcases[-3:]:
        bad += not good
        print("%-4s %-66s" % ("ok" if good else "BAD", name))
    if any(not g for _, g in pcases):
        print("\n".join(lines))
    lim = loadgate.load_limit() or 12.0
    lcases = [("load: every commit inside its budget on a loaded Mac is a PASS that names the load",
               [r[3] for r in judge(ok, ok, ok, lim + 4)] == [False, False, False] and "machine load %.1f" % (lim + 4) in judge(ok, ok, ok, lim + 4)[0][1]),
              ("load: one commit over its budget on a loaded Mac is NOT JUDGED, the number printed",
               judge(ok, with_last(cl + 7), ok, lim + 4)[0][3] is None and "worst %.1f" % (cl + 7) in judge(ok, with_last(cl + 7), ok, lim + 4)[0][1]),
              ("load: the same commit at PERF_LOAD_MAX is OVER", judge(ok, with_last(cl + 7), ok, lim)[0][3] is True)] + loadgate.selftest_cases(cl)
    for name, good in lcases:
        bad += not good
        print("%-4s %-66s" % ("ok" if good else "BAD", name))
    print("%d checks, %d failed, 0 not judged" % (len(cases) + 1 + len(pcases) + len(lcases), bad)); return 1 if bad else 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    bundle = argv[argv.index("--bundle") + 1] if "--bundle" in argv else os.environ.get("WHOOSH_BUNDLE")
    refusal = header("interaction", bundle)
    if refusal:
        return finish("interaction", refusal, [])
    ctl = connect()
    if ctl is None:
        return finish("interaction", "not judged: no socket", [])
    frames, commits, gaps, kinds, phases = [], [], [], [], []
    back_commits, back_kinds, back_phases = [], [], []
    for chord in ACTIONS:
        try:
            ctl.key(chord)
            frames.append(float(ctl.measure("mac-main.interaction.frame")["ms"]))
            commits.append(float(ctl.measure("mac-main.interaction.commit")["ms"]))
            try:
                gaps.append(float(ctl.measure("mac-main.interaction.gap")["ms"]))     # appshell serves it once the presented-frame timeline exists; until then NOT JUDGED
            except Unreadable:
                pass
            if "--dump" in argv:   # where the main thread's time went (perf-archive); read after the frame, so it never adds to the action
                try:
                    phases.append(ctl.measure("mac-main.interaction.phases")["phases"])
                except (Unreadable, KeyError, TypeError):
                    phases.append(None)
            kinds.append({"E": "archive", "J": "next", "K": "next", "Enter": "open"}[chord])
            # perf-archive round 3: the way back (Z after E, Escape after Enter) is waited for until ITS frame is out, so the next judged
            # key no longer waits in `arrival.queue` behind it (main94: archive arrival.queue median 31 ms was Z's own work). Its commit
            # and phases are printed as `undo` and `close` rows (information; the 100 judged actions are unchanged).
            back = {"E": ("Z", "undo"), "Enter": ("Escape", "close")}.get(chord)
            if back:
                ctl.key(back[0])
                try:
                    ctl.measure("mac-main.interaction.frame")
                    back_commits.append(float(ctl.measure("mac-main.interaction.commit")["ms"]))
                    back_kinds.append(back[1])
                    if "--dump" in argv:
                        try:
                            back_phases.append(ctl.measure("mac-main.interaction.phases")["phases"])
                        except (Unreadable, KeyError, TypeError):
                            back_phases.append(None)
                    else:
                        back_phases.append(None)
                except (Unreadable, KeyError, TypeError, ValueError):
                    pass
        except (Unreadable, KeyError, TypeError, ValueError):
            pass
    if "--dump" in argv:
        dump(frames, commits, kinds)
        for line in phase_lines(phases if len(phases) == len(kinds) else [None] * len(kinds), kinds):
            print(line)
        if len(phases) == len(kinds):
            for line in worst_phase_lines(phases, kinds):
                print(line)
        for line in gap_lines(gaps, kinds, phases):
            print(line)
        for k in ("undo", "close"):
            v = [c for c, kk in zip(back_commits, back_kinds) if kk == k]
            if v:
                print("interaction: dump commit %-6s n=%3d median %6.1f max %6.1f (the way back, not judged)" % (k, len(v), statistics.median(v), max(v)))
        for line in phase_lines(back_phases, back_kinds):
            print(line)
    return finish("interaction", None, judge(frames, commits, gaps, os.getloadavg()[0]))


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
