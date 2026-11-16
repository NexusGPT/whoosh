#!/usr/bin/env python3
"""Memory budget (fn-perf-mac): resident memory of the rig copy with the 30-account full-history fixture, below MEM_MAC_MB.

  sh scripts/launch-one.sh <bundle> -- sh -c 'WHOOSH_SOCKET_DIR="$D" python3 perf/memory-mac.py --accounts 30 --bundle <bundle>'
  python3 perf/memory-mac.py --selftest

Loads the fixture with `fixture load full-history-30` (fixtures/tests-j1.json "perf"); a core that does not serve it makes the
cell NOT JUDGED, never a pass. Measures the summed resident set (ps rss) of the test copy: the instance pid launch-one.sh started (RIG_PID or $D/rig.pid) and processes running from $D/rig/
(the bundle is COPIED there; matching on --bundle's path found nothing and read 0 MB, main83). A 0, a missing reading or a pid that is not the copy is NOT JUDGED.
Exit 1 above MEM_MAC_MB, or when nothing was judged.
"""
import json, os, plistlib, re, statistics, subprocess, sys, time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable, TIMEOUT_S      # noqa: E402
from lib.fixture import const                            # noqa: E402
from footprint import FIXTURE_LOAD_WAIT_S                # noqa: E402  the one number for how long the large fixture may take to load

# launch-r3: the load verb used the socket's usual TIMEOUT_S (5 s, journeys/lib/ctl.py). The app answers it after generating 300,000
# messages and reading the list and the counts twice, all on its main thread: longer than 5 s on a loaded Mac, so main57 and main58
# both read "timed out" and the cell was never judged. perf/footprint.py waited FIXTURE_LOAD_WAIT_S for the same verb all along.


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
    if ld > lim:   # lead 2026-11-13 (journeys/lib/loadgate.py): memory does not depend on other processes' load, so it is judged at any load
        print("%s: machine load %.2f is above PERF_LOAD_MAX %s: memory is judged at any load (it does not depend on other processes)" % (name, ld, lim))
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


def ps_table():
    """[(pid, ppid, rss_kb, command)] of every process, from one `ps` call."""
    out = subprocess.run(["ps", "-axo", "pid=,ppid=,rss=,command="], capture_output=True, text=True).stdout
    rows = []
    for l in out.splitlines():
        f = l.split(None, 3)
        if len(f) == 4 and f[0].isdigit() and f[1].isdigit() and f[2].isdigit():
            rows.append((int(f[0]), int(f[1]), int(f[2]), f[3]))
    return rows


def rig_target(env=None):
    """(pid, run folder) of the test copy, as scripts/launch-one.sh hands them to the driven command: RIG_PID (the instance it started) or $D/rig.pid, and
    $D (the run folder; the copy runs from $D/rig/<App>.app). (None, None) parts when the environment does not say."""
    env = os.environ if env is None else env
    d = env.get("D") or env.get("WHOOSH_SOCKET_DIR") or None
    pid = env.get("RIG_PID")
    if not pid and d:
        try:
            with open(os.path.join(d, "rig.pid"), encoding="utf-8") as f:
                pid = f.read().strip()
        except OSError:
            pid = None
    return (int(pid) if pid and pid.isdigit() else None), (d.rstrip("/") if d else None)


def rss_mb(env=None, table=None):
    """(resident MB, None) summed over the test copy's processes, or (None, the reason in words). The test copy is the instance launch-one.sh started: a COPY of
    the bundle under $D/rig/ (not the bundle passed with --bundle: main83 matched on that path, found no process and read 0 MB, which passed). A reading of 0,
    a pid that is not running, and a pid whose executable is not under $D/rig/ are NOT measures."""
    pid, d = rig_target(env)
    if pid is None:
        return None, "the test copy's pid is not known (no RIG_PID and no rig.pid in the run folder), so no process can be read"
    rows = ps_table() if table is None else table
    mine = [r for r in rows if r[0] == pid]
    if not mine:
        return None, "pid %d is not running" % pid
    cmd = mine[0][3]
    if d is None or not cmd.startswith(d + "/rig/"):
        return None, "pid %d runs %s, which is not the test copy (the test copy runs from %s)" % (pid, " ".join(cmd.split())[:100], (d + "/rig/") if d else "the run folder, which is unknown")
    prefix = d + "/rig/"
    kb = sum(r[2] for r in rows if r[0] == pid or r[3].startswith(prefix))
    if kb <= 0:
        return None, "pid %d read 0 MB resident, which is not a reading of a running app" % pid
    return kb / 1024.0, None


def judge(mb, accounts, why=None):
    lim = const("MEM_MAC_MB")
    if mb is not None and not (mb > 0 and mb == mb and mb != float("inf")):
        why, mb = "the reading was %s MB, which is not a reading of a running app" % mb, None
    if accounts != 30:
        return [("resident MB at %s accounts" % accounts, mb if mb is not None else "not measured" + (" (%s)" % why if why else ""), "< %d at 30 accounts" % lim, None)]
    return [("resident MB, 30 accounts full history", "%.0f" % mb if mb is not None else "not measured" + (" (%s)" % why if why else ""), "< %d" % lim, None if mb is None else mb >= lim)]


def selftest():
    lim = const("MEM_MAC_MB")
    cases = [("just under", judge(lim - 1, 30), False), ("at budget", judge(float(lim), 30), True), ("not measured", judge(None, 30), None), ("wrong fixture size", judge(10.0, 5), None),
             ("a reading of 0 MB is not a pass", judge(0.0, 30), None), ("a plausible figure over the budget fails", judge(lim + 300.0, 30), True),
             ("a negative reading is not a pass", judge(-5.0, 30), None), ("a NaN reading is not a pass", judge(float("nan"), 30), None)]
    bad = 0
    for name, v, want in cases:
        good = v[0][3] == want; bad += not good
        print("%-4s %-18s over=%s" % ("ok" if good else "BAD", name, v[0][3]))
    # launch-r3: the load verb waits the large fixture's time, then the socket goes back to its usual wait (a fake socket records both)
    class FakeSock:
        def __init__(self): self.waits = []
        def settimeout(self, s): self.waits.append(s)
    class FakeCtl:
        def __init__(self): self.sock = FakeSock(); self.sent = None
        def send(self, *a): self.sent = (self.sock.waits[-1], a); return {"ok": True}
    c = FakeCtl(); load(c, "full-history-30")
    waited, sent = c.sent
    good = waited == load_wait_s() and waited > TIMEOUT_S and sent == ("fixture", "load", "full-history-30") and c.sock.waits[-1] == TIMEOUT_S
    bad += not good
    print("%-4s %-18s waited %s s for the load (socket default %s s), then %s s" % ("ok" if good else "BAD", "load wait", waited, TIMEOUT_S, c.sock.waits[-1]))
    # main83: the rig copy runs from $D/rig/, not from the --bundle path; a stand-in process table shows the reading, and each way it cannot be read
    d, other = "/r/Whoosh-rig-hidden/run-9", "/Applications/Whoosh.app/Contents/MacOS/Whoosh"
    app = d + "/rig/Whoosh.app/Contents/MacOS/Whoosh"
    env = {"D": d, "RIG_PID": "100"}
    def tab(*rows): return [r for r in rows]
    cases2 = [("test copy found by its pid under $D/rig/ (not by the bundle path)", rss_mb(env, tab((100, 1, 307200, app), (101, 100, 51200, app + " helper"), (7, 1, 999999, other))), (350.0, None)),
              ("a reading of 0 MB of the test copy is not measured", rss_mb(env, tab((100, 1, 0, app))), (None, "pid 100 read 0 MB resident, which is not a reading of a running app")),
              ("the pid is not running", rss_mb(env, tab((7, 1, 5000, other))), (None, "pid 100 is not running")),
              ("a pid that is not the test copy is not measured", rss_mb(env, tab((100, 1, 5000, other))), (None, "pid 100 runs %s, which is not the test copy (the test copy runs from %s/rig/)" % (other, d))),
              ("no pid known", rss_mb({"D": d}, tab((100, 1, 5000, app))), (None, "the test copy's pid is not known (no RIG_PID and no rig.pid in the run folder), so no process can be read")),
              ("a 0 MB reading from the table is NOT JUDGED, never ok", judge(*rss_mb(env, tab((100, 1, 0, app))), 30)[0][3:], (None,))]
    for name, got, want in cases2:
        good = got == want; bad += not good
        print("%-4s %-70s %s" % ("ok" if good else "BAD", name, "" if good else "got %r want %r" % (got, want)))
    # round 8: the load no longer refuses; memory over budget on a loaded Mac stays OVER, inside it a PASS
    lim = load_limit()
    saved = os.getloadavg
    try:
        os.getloadavg = lambda: ((lim or 12.0) + 4,) * 3
        import io, contextlib
        with contextlib.redirect_stdout(io.StringIO()):
            refused = header("memory-mac", None) not in (None, "not judged: no build stamp, so the number cannot be tied to a build")
    finally:
        os.getloadavg = saved
    mb_lim = const("MEM_MAC_MB")
    cases3 = [("load above PERF_LOAD_MAX no longer refuses the run (memory is judged at any load)", not refused)]
    if True:
        cases3 += [("memory inside its budget is a PASS whatever the load", judge(mb_lim - 1, 30)[0][3] is False),
                   ("memory over its budget is OVER whatever the load", judge(mb_lim + 1, 30)[0][3] is True)]
    for name, good in cases3:
        bad += not good
        print("%-4s %s" % ("ok" if good else "BAD", name))
    n = len(cases) + 1 + len(cases2) + len(cases3)
    print("%d checks, %d failed, 0 not judged" % (n, bad)); return 1 if bad else 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    bundle = argv[argv.index("--bundle") + 1] if "--bundle" in argv else os.environ.get("WHOOSH_BUNDLE")
    accounts = int(argv[argv.index("--accounts") + 1]) if "--accounts" in argv else 30
    refusal = header("memory-mac", bundle)
    if refusal:
        return finish("memory-mac", refusal, [])
    ctl = connect()
    if ctl is None:
        return finish("memory-mac", "not judged: no socket", [])
    t0 = time.monotonic()
    try:
        load(ctl, "full-history-%d" % accounts)
    except Unreadable as e:
        return finish("memory-mac", "not judged: the fixture did not load within %d s (%s)" % (load_wait_s(), e), [])
    print("memory-mac: info, not judged: the fixture full-history-%d loaded in %.1f s" % (accounts, time.monotonic() - t0))
    mb, why = rss_mb()
    print("memory-mac: info, not judged: the test copy is pid %s under %s" % (rig_target()[0], rig_target()[1]))
    return finish("memory-mac", None, judge(mb, accounts, why))


def load_wait_s():
    """How long the load verb may take: the large fixture's own wait, never the socket's usual one."""
    return FIXTURE_LOAD_WAIT_S


def load(ctl, name):
    """`fixture load <name>` with the socket waiting load_wait_s() for the answer, then back to its usual wait."""
    ctl.sock.settimeout(load_wait_s())
    try:
        return ctl.send("fixture", "load", name)
    finally:
        ctl.sock.settimeout(TIMEOUT_S)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
