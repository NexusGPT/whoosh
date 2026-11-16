#!/usr/bin/env python3
"""Launch budgets (fn-perf-mac): warm launch to inbox, cold launch, first frame. Constants read from docs/CONSTANTS.md.

  python3 perf/launch.py --bundle <App.app> [--samples 5]     launches the rig copy once per sample through
                                                              scripts/launch-one.sh (never two at once) and reads
                                                              `measure mac-main.launch` (process start to first paint).
  python3 perf/launch.py --selftest                           proves the judgement flips at each budget.
  One more same-copy pair (always, since perf-archive round 4; `--phases` is accepted and changes nothing) prints
  `launch: phase <name> at <ms> ms, +<step> ms` for the second launch (`measure mac-main.launch.phases`, Sources/App/LaunchClock.swift),
  or `launch: phases not served`. From it: the same-copy line is judged WITHOUT the hidden copy's own set-up (RIG_OWN; the raw figure is
  printed beside it), and the first frame is judged on root-init to first-frame (the raw process-start figure is printed beside it).
Sample 1 is the cold launch (the first launch this script makes, nothing primed by it); samples 2..N are warm and the
median is judged. First frame: `measure mac-main.first-frame` (not served at this commit = NOT JUDGED, never a pass).
Exit 1 when warm >= LAUNCH_WARM_MS, cold >= LAUNCH_COLD_MS, first frame >= FIRST_FRAME_MS, or when nothing could be judged.

Two warm figures, never merged. "warm, fresh copy" is the median of samples 2..N, each launched from a NEW copy of the bundle (the
rig copies, re-signs and launches a new file every time, so every sample pays the OS's first-run scan of that file). "warm, same copy"
is the SECOND launch of one rig copy, inside one lock hold (scripts/launch-one.sh with LAUNCH_RUNS=2: launch, read, quit by pid,
launch the same file again, read again); `--pairs N` (default 3) pairs, the median of the second launches is judged. That is the
launch an owner feels after the first one of an install. Its budget row is LAUNCH_SAME_COPY_MS when docs/CONSTANTS.md holds it,
else LAUNCH_WARM_MS, and the line says which. The first launch of each pair is printed beside it (the same-copy pair table).
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
    if ld > lim:   # lead 2026-11-13: the run goes on; a launch inside its budget passes, one over it is NOT JUDGED (loadgate.gate)
        print("%s: machine load %.2f is above PERF_LOAD_MAX %s: the run goes on; inside a budget is a PASS, over it is NOT JUDGED" % (name, ld, lim))
    return None


def loaded(rows, load):
    """The shared load rule on the launch rows: inside the budget passes and names the load; over it above PERF_LOAD_MAX is NOT JUDGED."""
    return [(l, v if over is None else "%s%s" % (v, loadgate.note(load)), lim, loadgate.gate(over, load)) for l, v, lim, over in rows]


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


def same_copy_budget():
    """(limit, row name) for the same-copy line: its own row LAUNCH_SAME_COPY_MS when the ledger holds one, else LAUNCH_WARM_MS."""
    try:
        return const("LAUNCH_SAME_COPY_MS"), "LAUNCH_SAME_COPY_MS"
    except KeyError:
        return const("LAUNCH_WARM_MS"), "LAUNCH_WARM_MS, no own row"


# perf-archive round 4 (lead's ruling): the hidden copy's own set-up, which the owner's copy never runs, as launch-phase segments of the
# hidden copy's launch: the activation policy (nsapp-made to rig-nsapp-end), the framework loading and panel hooks (to rig-system-ui-end),
# the window and application hooks (to app-init-end). AppKit's own start (app-init to nsapp-made) is NOT in it: the owner pays that too.
RIG_OWN = [("nsapp-made", "rig-nsapp-end"), ("rig-nsapp-end", "rig-system-ui-end"), ("rig-system-ui-end", "app-init-end")]


def rig_own_ms(phases):
    """The hidden copy's own set-up in ms from one launch's phases; None when a point is missing (an older build, phases not served)."""
    at = dict(phases or [])
    if not all(a in at and b in at for a, b in RIG_OWN):
        return None
    return sum(at[b] - at[a] for a, b in RIG_OWN)


def first_frame_own_ms(phases):
    """FIRST_FRAME_MS as proposed in perf-archive round 2/4: first-frame minus root-init (the root view's first build to the first frame,
    the app's own share); None when either point is missing."""
    at = dict(phases or [])
    return at["first-frame"] - at["root-init"] if "first-frame" in at and "root-init" in at else None


def judge(cold, warm, first_frame, same=None, own=0.0, ff_own="raw"):
    """Four verdicts. `same` is the median second launch of one copy (None = not measured = NOT JUDGED, never a pass); the judged
    same-copy figure is `same` minus `own`, the hidden copy's own set-up (None = not measured = NOT JUDGED). `ff_own` is the first
    frame judged as first-frame minus root-init ("raw" = the old whole-launch first frame, for the older callers)."""
    # launch-r3 round 2: a fresh copy's launch is the FIRST launch of a new file (the OS scans it first: 1.1 to 1.2 s more than the same
    # file's second launch, every pair, main57 to main74), the launch an owner meets once after an install, so it is judged against
    # LAUNCH_COLD_MS. Judged against LAUNCH_WARM_MS it could never pass; main74 read 1978.6 fresh copy against 1975.9 cold: one figure.
    c, f = const("LAUNCH_COLD_MS"), const("FIRST_FRAME_MS")
    sm, sname = same_copy_budget()
    # the judged figures take the old names, so the comparisons below (and their broken copies in checks/proofs) stay as they were
    same = None if same is None or own is None else same - own
    first_frame = first_frame if ff_own == "raw" else ff_own
    return [("warm, fresh copy, median ms", warm, "< %d (LAUNCH_COLD_MS: a new copy's first launch)" % c, None if warm is None else warm >= c),
            ("launch without the hidden copy's own set-up, same copy, median ms" if own != 0.0 else "warm, same copy, median ms", same,
             "< %d (%s, judged without the hidden copy's own set-up)" % (sm, sname) if own != 0.0 else "< %d (%s)" % (sm, sname),
             None if same is None else same >= sm),
            ("cold launch ms", cold, "< %d" % c, None if cold is None else cold >= c),
            ("first frame ms" if ff_own == "raw" else "first frame ms, root-init to first-frame", first_frame, "< %d" % f,
             None if first_frame is None else first_frame >= f)]


def hidden_ok(bundle):
    """The same test scripts/launch-one.sh applies: the bundle declares the hidden rig mode (Contents/Resources/rig-hidden.ok)."""
    return os.path.isfile(os.path.join(bundle, "Contents", "Resources", "rig-hidden.ok"))


def sample(bundle, verb="mac-main.launch"):
    """One launch of the rig copy in HIDDEN mode (launch-one.sh refuses every other launch with exit 6: WHOOSH_RIG_HIDDEN=1, the app flag
    --rig-hidden and LAUNCH_ENV) and one `measure <verb>` through the control socket. None when the launch was refused or the verb is not served."""
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=(os.environ.get("LAUNCH_ENV", "") + " WHOOSH_RIG_HIDDEN=1").strip())
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--fixture-core", "--rig-hidden", sys.executable,
           os.path.join(ROOT, "tools", "whoosh-ctl.py"), "measure", verb]
    r = subprocess.run(cmd, capture_output=True, text=True, cwd=ROOT, env=env)
    m = re.search(r'"ms"\s*:\s*([0-9.]+)', r.stdout)
    if not m:
        print("launch: sample of %s gave no number (launch-one exit %d): %s" % (verb, r.returncode, (r.stdout + r.stderr).strip().splitlines()[-1:] or ""))
    return float(m.group(1)) if m else None


def sample_pair(bundle, verb="mac-main.launch"):
    """Two launches of ONE rig copy inside one lock hold (launch-one.sh LAUNCH_RUNS=2): [first ms, second ms], None for a launch with no number."""
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_RUNS="2", LAUNCH_ENV=(os.environ.get("LAUNCH_ENV", "") + " WHOOSH_RIG_HIDDEN=1").strip())
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--fixture-core", "--rig-hidden", sys.executable,
           os.path.join(ROOT, "tools", "whoosh-ctl.py"), "measure", verb]
    r = subprocess.run(cmd, capture_output=True, text=True, cwd=ROOT, env=env)
    return parse_pair(r.stdout, r.stderr, r.returncode)


def parse_pair(out, err, rc):
    """The numbers of a LAUNCH_RUNS=2 run: one `"ms"` per run, each after its `launch-one: run K of 2` marker. A run with no number stays None."""
    parts = re.split(r"launch-one: run (\d) of 2", out)      # [pre, "1", text1, "2", text2]
    got = {}
    for i in range(1, len(parts) - 1, 2):
        m = re.search(r'"ms"\s*:\s*([0-9.]+)', parts[i + 1])
        got[parts[i]] = float(m.group(1)) if m else None
    pair = [got.get("1"), got.get("2")]
    if None in pair:
        print("launch: same-copy pair gave %s (launch-one exit %d): %s" % (pair, rc, (out + err).strip().splitlines()[-1:] or ""))
    return pair


def find_phases(obj):
    """The `phases` list anywhere in a reply (the socket wraps a measure's answer)."""
    if isinstance(obj, dict):
        if isinstance(obj.get("phases"), list):
            return obj["phases"]
        for v in obj.values():
            p = find_phases(v)
            if p is not None:
                return p
    return None


def parse_phases(out):
    """`measure mac-main.launch.phases` of the SECOND launch of a LAUNCH_RUNS=2 run (the same copy, the launch the owner feels):
    [(phase, ms since process start)] in the order reached; None when that run printed no phases."""
    parts = re.split(r"launch-one: run (\d) of 2", out)
    text = dict(zip(parts[1::2], parts[2::2])).get("2")
    if text is None:
        return None
    for line in text.splitlines():
        if '"phases"' not in line:
            continue
        try:
            p = find_phases(json.loads(line))
        except ValueError:
            continue
        if p is not None:
            return [(str(x["phase"]), float(x["ms"])) for x in p if "phase" in x and "ms" in x]
    return None


def phase_lines(phases):
    """One line per launch phase: ms since process start and ms since the phase before (information, not a verdict)."""
    if not phases:
        return ["launch: phases not served (no `mac-main.launch.phases` on the second launch of the same copy)"]
    out, before = [], 0.0
    for name, ms in phases:
        out.append("launch: phase %-26s at %8.1f ms, +%7.1f ms" % (name, ms, ms - before))
        before = ms
    return out


def sample_phases(bundle):
    """One LAUNCH_RUNS=2 pair; the second launch's `measure mac-main.launch.phases`."""
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_RUNS="2", LAUNCH_ENV=(os.environ.get("LAUNCH_ENV", "") + " WHOOSH_RIG_HIDDEN=1").strip())
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--fixture-core", "--rig-hidden", sys.executable,
           os.path.join(ROOT, "tools", "whoosh-ctl.py"), "measure", "mac-main.launch.phases"]
    r = subprocess.run(cmd, capture_output=True, text=True, cwd=ROOT, env=env)
    return parse_phases(r.stdout)


def selftest():
    c, f = const("LAUNCH_COLD_MS"), const("FIRST_FRAME_MS")
    sm = same_copy_budget()[0]
    cases = [("all just under", (c - 1, c - 1, f - 1, sm - 1), [False, False, False, False]),
             ("fresh copy at the cold budget", (c - 1, c, f - 1, sm - 1), [True, False, False, False]),
             ("fresh copy 1978 ms (main74) is over the cold budget", (c - 1, 1978.6, f - 1, sm - 1), [1978.6 >= c, False, False, False]),
             ("cold at budget", (c, c - 1, f - 1, sm - 1), [False, False, True, False]),
             ("first frame at budget", (c - 1, c - 1, f, sm - 1), [False, False, False, True]),
             ("same copy at budget (its own row)", (c - 1, c - 1, f - 1, sm), [False, True, False, False]),
             ("same copy over, fresh copy under", (c - 1, c - 1, f - 1, sm + 500), [False, True, False, False]),
             ("same copy just under, fresh copy over", (c - 1, c + 500, f - 1, sm - 1), [True, False, False, False]),
             ("nothing measured", (None, None, None, None), [None, None, None, None])]
    # round 4: the hidden copy's own set-up is taken off the same-copy figure before it is judged; a missing set-up figure is NOT JUDGED
    ph = [("app-init", 40.0), ("nsapp-made", 500.0), ("rig-nsapp-end", 510.0), ("rig-system-ui-end", 515.0), ("app-init-end", 520.0),
          ("root-init", 700.0), ("first-frame", 760.0)]
    own_cases = [("set-up read from the phases (10 + 5 + 5 ms)", rig_own_ms(ph) == 20.0),
                 ("an older build without nsapp-made gives no set-up figure", rig_own_ms([p for p in ph if p[0] != "nsapp-made"]) is None),
                 ("first frame own share is first-frame minus root-init", first_frame_own_ms(ph) == 60.0),
                 ("same copy over by 10 raw, under once the set-up (20) is off",
                  [v[3] for v in judge(c - 1, c - 1, f - 1, sm + 10, 20.0, f - 1)] == [False, False, False, False]),
                 ("same copy over even without the set-up", [v[3] for v in judge(c - 1, c - 1, f - 1, sm + 30, 20.0, f - 1)] == [False, True, False, False]),
                 ("no set-up figure: the same-copy line is NOT JUDGED", [v[3] for v in judge(c - 1, c - 1, f - 1, sm - 1, None, f - 1)] == [False, None, False, False]),
                 ("the judged line is named for the set-up taken off", judge(c - 1, c - 1, f - 1, sm, 20.0, f - 1)[1][0] == "launch without the hidden copy's own set-up, same copy, median ms"),
                 ("first frame judged on its own share: whole launch over, share under", [v[3] for v in judge(c - 1, c - 1, 1886.3, sm - 1, 0.0, f - 1)] == [False, False, False, False]),
                 ("first frame share at budget", [v[3] for v in judge(c - 1, c - 1, 1.0, sm - 1, 0.0, f)] == [False, False, False, True]),
                 ("no root-init: first frame share NOT JUDGED", [v[3] for v in judge(c - 1, c - 1, 1.0, sm - 1, 0.0, None)] == [False, False, False, None])]
    bad = 0
    for name, (cold, warm, ff, same), want in cases:
        got = [v[3] for v in judge(cold, warm, ff, same)]       # order: fresh warm, same-copy warm, cold, first frame
        ok = got == want
        print("%-4s %-40s over=%s" % ("ok" if ok else "BAD", name, got)); bad += not ok
    # the pair parser: both numbers, a missing second launch stays None, no marker = both None
    out = 'launch-one: run 1 of 2 (same copy x, pid 1)\n{"ms": 1700.5}\nlaunch-one: run 2 of 2 (same copy x, pid 2)\n{"ms": 900.25}\n'
    for name, pr, want in [("pair parsed", parse_pair(out, "", 0), [1700.5, 900.25]),
                           ("second launch without a number", parse_pair(out.rsplit("launch-one: run 2", 1)[0] + "launch-one: run 2 of 2\nno json\n", "", 0), [1700.5, None]),
                           ("no marker at all", parse_pair('{"ms": 5}', "", 4), [None, None])]:
        ok = pr == want
        print("%-4s %-40s %s" % ("ok" if ok else "BAD", name, pr)); bad += not ok
    # the launch phase lines (perf-archive r2): the second run's phases, in order, with the step from the one before
    ph = ('launch-one: run 1 of 2 (same copy x, pid 1)\n{"ok": true, "value": {"phases": [{"phase": "app-init", "ms": 1.0}]}}\n'
          'launch-one: run 2 of 2 (same copy x, pid 2)\n{"ok": true, "value": {"phases": [{"phase": "app-init", "ms": 300.0}, {"phase": "first-frame", "ms": 512.5}]}}\n')
    got = parse_phases(ph)
    lines = phase_lines(got)
    pcases = [("phases read from the second launch only", got == [("app-init", 300.0), ("first-frame", 512.5)]),
              ("one line per phase with its step", lines == ["launch: phase %-26s at %8.1f ms, +%7.1f ms" % ("app-init", 300.0, 300.0),
                                                              "launch: phase %-26s at %8.1f ms, +%7.1f ms" % ("first-frame", 512.5, 212.5)]),
              ("a second launch with no phases says not served", parse_phases(ph.split("launch-one: run 2")[0] + "launch-one: run 2 of 2\nno json\n") is None
               and phase_lines(None)[0].startswith("launch: phases not served"))]
    for name, ok in pcases:
        print("%-4s %-40s" % ("ok" if ok else "BAD", name)); bad += not ok
    hi = (loadgate.load_limit() or 12.0) + 4
    own_cases += [("load: every launch figure inside its budget on a loaded Mac passes", [v[3] for v in loaded(judge(c - 1, c - 1, f - 1, sm - 1), hi)] == [False] * 4),
                  ("load: a launch over its budget on a loaded Mac is NOT JUDGED", [v[3] for v in loaded(judge(c - 1, c - 1, f - 1, sm + 500), hi)] == [False, None, False, False]),
                  ("load: the same at PERF_LOAD_MAX is OVER", [v[3] for v in loaded(judge(c - 1, c - 1, f - 1, sm + 500), hi - 4)] == [False, True, False, False])]
    own_cases += loadgate.selftest_cases()
    for name, ok in own_cases:
        print("%-4s %-40s" % ("ok" if ok else "BAD", name)); bad += not ok
    n = len(cases) + 3 + len(pcases) + len(own_cases)
    print("%d checks, %d failed, 0 not judged" % (n, bad)); return 1 if bad else 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    bundle = argv[argv.index("--bundle") + 1] if "--bundle" in argv else os.environ.get("WHOOSH_BUNDLE")
    n = int(argv[argv.index("--samples") + 1]) if "--samples" in argv else 5
    refusal = header("launch", bundle)
    if refusal:
        return finish("launch", refusal, [])
    if not hidden_ok(bundle):
        return finish("launch", "HELD: %s has no Contents/Resources/rig-hidden.ok, so no rig copy may be launched (owner 2026-11-08 16:30); nothing was launched" % bundle, [])
    ms = [sample(bundle) for _ in range(n)]
    cold, warm = ms[0], [x for x in ms[1:] if x is not None]
    ff = sample(bundle, "mac-main.first-frame")      # not served until AppModel.firstFrameMs lands (appshell): None = NOT JUDGED, never a pass
    pairs = [sample_pair(bundle) for _ in range(int(argv[argv.index("--pairs") + 1]) if "--pairs" in argv else 3)]
    for i, (a, b) in enumerate(pairs, 1):
        print("launch: same-copy pair %d: first launch %s ms, second launch of the same copy %s ms" % (i, a, b))
    # one more same-copy pair: where the second launch's time went (perf-archive r2), always run since round 4: the judged same-copy line
    # takes the hidden copy's own set-up off, and the first frame is judged on root-init to first-frame (lead's ruling, round 4)
    phases = sample_phases(bundle)
    for line in phase_lines(phases):
        print(line)
    own = rig_own_ms(phases)
    seconds = [b for _, b in pairs if b is not None]
    same = statistics.median(seconds) if seconds else None
    print("launch: warm, same copy, median ms (raw, with the hidden copy's own set-up) %s, not judged" % same)
    print("launch: the hidden copy's own set-up ms %s (%s), not judged" % (own, " + ".join("%s to %s" % ab for ab in RIG_OWN)))
    print("launch: first frame ms (raw, process start to first frame) %s, not judged" % ff)
    return finish("launch", None, loaded(judge(cold, statistics.median(warm) if warm else None, ff, same, own, first_frame_own_ms(phases)), os.getloadavg()[0]))


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
