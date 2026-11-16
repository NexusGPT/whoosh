#!/usr/bin/env python3
"""Footprint gate (owner 2026-11-08 14:46, "beware of app speed and heavyness on ram and CPU"): memory, CPU and launch of the rig copy,
each against a ledger budget (docs/CONSTANTS.md). The embedding model must load on the first meaning search and leave when idle.

  python3 perf/footprint.py <App.app> [--weights DIR] [--measure footprint|rss] [--unload-s N] [--mutant launch|keep]   the real gate (about 7 minutes)
  python3 perf/footprint.py --fake [clean|launch|keep|busy]                                     dry run against a fake process
  python3 perf/footprint.py --selftest                                                           proves the judgement flips at each budget
  python3 perf/footprint.py crawl <App.app> [--weights DIR] [--allow-load]                      the background download (item f4e1fbe8), see CRAWL below

The real gate launches the rig copy once through scripts/launch-one.sh on the fixture core, then, inside that one run, measures:
  1. launch to first frame, ms (`measure mac-main.launch`, the method of perf/launch.py; ONE sample, so a cold launch if nothing primed it)
  2. resident memory (ps rss, summed over the rig copy's processes) IDLE_S after the first frame, at idle, no search done   RSS_IDLE_MAX_MB
  3. CPU of the rig copy averaged over the next CPU_S seconds, still idle, no sync (the fixture core has none)           CPU_IDLE_MAX_PCT
  4. resident memory right after a driven meaning search (the model is loaded)                                          RSS_MODEL_MAX_MB
  5. resident memory EMBED_IDLE_UNLOAD_S + 5 s later (the model is unloaded)                                            RSS_IDLE_MAX_MB
"The model is loaded" is read off check 4: the memory after the search must exceed RSS_IDLE_MAX_MB, the ledger's own line between "no model"
and "model"; else the model never loaded (no weights, or the search never reached it) and checks 4 and 5 are NOT JUDGED, never a pass.
A refusal before the launch, and NOT JUDGED rows, follow perf/launch.py: the machine load above PERF_LOAD_MAX gives no verdict; the end-of-run load
does the same for the two timing rows (CPU, launch). Last line: `N checks, F failed, U not judged`; exit 1 when F or U is above 0.

Test-only switches, all named in the output when used: --unload-s N sets WHOOSH_EMBED_IDLE_UNLOAD_S in the app (the unload wait is then N + 5 s,
not 305 s); --mutant sets WHOOSH_EMBED_MUTANT (`launch` loads the model at launch, `keep` never unloads it) to show the gate go red; the dry run
(`--fake`, with `PERF_LOAD_MAX=1000` in a loaded machine) scales every wait down and runs no app at all.
Raw samples (a reading every 5 s over the whole run) go to _captures/perf/footprint/<UTC time>-<mode>.json.

CRAWL (perf-archive round 5, item f4e1fbe8): the rig copy on the REAL engine (no --fixture-core) against the loopback server of
checks/realcore-check.py --slice-crawl (realcore_mailfake.CRAWL_PLAN: INBOX 25, Archive 65, Sent 10). Inside launch-one the local account
is added through the sheet (realcore-check's own mv_add_account), then every CRAWL_SAMPLE_S the app's physical footprint and CPU seconds
(summed over the rig copy's processes, proc_pid_rusage) and the engine's `core state reply/progress` are read. Each sample is in phase
`download` while fewer bodies than messages are stored, then `index` while fewer are indexed (only with a model folder: --weights or
WHOOSH_EMBED_WEIGHTS), then the run ends. Per phase it prints the peak and the median footprint, the average CPU and the highest CPU over
one sample interval, and one verdict line per budget:
  PASS|OVER crawl-<phase>-memory-peak   peak footprint < RSS_IDLE_MAX_MB + EMBED_INDEX_CEILING_MB
  PASS|OVER crawl-<phase>-cpu-average   average CPU over the phase < CRAWL_CPU_MAX_PCT (printed, NOT JUDGED while the ledger holds no such row)
A phase that never ran or never finished is NOT JUDGED, never a pass; the CPU rows are NOT JUDGED above PERF_LOAD_MAX at the end of the run.
"""
import datetime, json, os, plistlib, re, socket, subprocess, sys, threading, time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable      # noqa: E402
from lib.fixture import const                            # noqa: E402
from lib import loadgate                                 # noqa: E402  (the one load rule, journeys/lib/loadgate.py)

OUT_DIR = os.path.join(ROOT, "_captures", "perf", "footprint")
RESULTS = "mac-search.results"
SAMPLE_S = 5            # how often the run takes a reading; a sampling cadence, not a budget
IDLE_S, CPU_S, UNLOAD_MARGIN_S = 60, 30, 5      # the owner's protocol for the three states (lead's brief, 2026-11-08); the ledger holds the budgets, these are the waits
SEARCH_WAIT_S = 240     # the first search loads the model; a slow disk or a loaded Mac may need minutes. Past this the search is judged as never answered
MIN_UNLOAD_S = 15       # the shortest test-only unload wait: the model must still be loaded when the loaded-model reading is taken
FIXTURE_LOAD_WAIT_S = 180   # the longest the large fixture may take to load before the run is judged not the one asked for
SETTLE_S = 3            # after the first results, before reading the memory of the loaded model


# ---------------------------------------------------------------- stamp, load, rows

def stamp(bundle):
    try:
        with open(os.path.join(bundle, "Contents", "Info.plist"), "rb") as f:
            p = plistlib.load(f)
        return "label=%s commit=%s built=%s dirty=%s" % (p["NexusBuildLabel"], p["NexusBuildCommit"], p["NexusBuiltAt"], p.get("NexusBuildDirtyFiles", "?"))
    except (OSError, KeyError, ValueError):
        return None


def load_limit():
    if os.environ.get("PERF_LOAD_MAX"):
        return float(os.environ["PERF_LOAD_MAX"])
    try:
        return float(const("PERF_LOAD_MAX"))
    except KeyError:
        return None


def header(bundle, fake):
    st = ("fake process, no bundle (dry run)" if fake else stamp(bundle)) if (fake or bundle) else None
    ld, lim = os.getloadavg()[0], load_limit()
    print("footprint: build stamp: %s" % (st or "UNKNOWN (no bundle, or no stamp in its Info.plist)"))
    print("footprint: machine load (1 min) at the start: %.2f, limit PERF_LOAD_MAX: %s" % (ld, lim if lim is not None else "NONE (docs/CONSTANTS.md holds no number)"))
    if st is None:
        return "not judged: no build stamp, so the number cannot be tied to a build"
    if lim is None:
        return "not judged: PERF_LOAD_MAX has no number in docs/CONSTANTS.md"
    if ld > lim:   # lead 2026-11-13: the run goes on (memory and the app's own CPU are judged at any load; the launch by loadgate.gate)
        print("footprint: machine load %.2f is above PERF_LOAD_MAX %s: the run goes on; memory and the app's CPU are judged at any load, the launch passes inside its budget and is NOT JUDGED over it" % (ld, lim))
    return None


def judge(m, load_end=None):
    """m: measured values (None = not measured). Rows are (budget name, label, value text, budget text, over?) with over True/False/None."""
    lim = load_limit()
    busy = load_end is not None and lim is not None and load_end > lim
    # perf-archive round 8 (lead 2026-11-13, journeys/lib/loadgate.py): the app's own idle CPU (its CPU seconds over the window) and every
    # memory figure do not depend on other processes' load, so they are judged at any load; the launch (a timing) passes inside its budget
    # on a loaded Mac and is NOT JUDGED over it.
    launch_over = m.get("launch_ms") is not None and m["launch_ms"] >= const("LAUNCH_COLD_MS")
    # launch-r3: the gate's ONE launch is the first launch of a new rig copy (the OS scans the new file first), so it is judged against
    # LAUNCH_COLD_MS, as perf/launch.py judges a fresh copy; the owner's daily (second) launch is perf/launch.py's LAUNCH_SAME_COPY_MS line.
    idle, model, cpu, warm = const("RSS_IDLE_MAX_MB"), const("RSS_MODEL_MAX_MB"), const("CPU_IDLE_MAX_PCT"), const("LAUNCH_COLD_MS")
    loaded = m.get("rss_model") is not None and m["rss_model"] > idle

    def row(name, label, val, limit, unit, fmt, ok_to_judge=True):
        if val is not None and unit in ("MB", "ms") and not (val > 0 and val != float("inf")):
            # a 0 (or NaN, negative) memory or launch time is not a reading of a running app: NOT JUDGED in words, never ok (main83: memory-mac passed on 0 MB)
            return (name, label, "not measured: the reading was %s %s, which is not a reading of a running app" % (val, unit), "< %s %s" % (limit, unit), None)
        over = None if (val is None or not ok_to_judge) else val >= limit
        text = "not measured" if val is None else ("%s %s (machine load %.1f at the end, above PERF_LOAD_MAX %s)" % (fmt % val, unit, load_end, lim) if not ok_to_judge
                                                  else "%s %s%s" % (fmt % val, unit, " (measured at machine load %.1f, above PERF_LOAD_MAX %s)" % (load_end, lim) if busy else ""))
        return (name, label, text, "< %s %s" % (limit, unit), over)
    rows = [row("RSS_IDLE_MAX_MB", "memory at idle, no search yet", m.get("rss_idle"), idle, "MB", "%.0f"),
            row("CPU_IDLE_MAX_PCT", "CPU at idle, average", m.get("cpu_pct"), cpu, "%", "%.2f"),
            row("RSS_MODEL_MAX_MB", "memory, model loaded", m.get("rss_model") if loaded else None, model, "MB", "%.0f"),
            row("RSS_IDLE_MAX_MB", "memory, model unloaded", m.get("rss_unloaded") if loaded else None, idle, "MB", "%.0f"),
            row("LAUNCH_COLD_MS", "launch to first frame (a new copy's first launch)", m.get("launch_ms"), warm, "ms", "%.0f", not (busy and launch_over))]
    if "visible_windows" in m:
        v = m["visible_windows"]
        rows.append(("RIG_HIDDEN", "windows of the app on screen", "%d" % v, "0", v > 0))
    if m.get("rss_model") is not None and not loaded:
        rows[2] = rows[2][:2] + ("%.0f MB after the search, not above RSS_IDLE_MAX_MB %s: the model never loaded (no weights, or the search never reached it)" % (m["rss_model"], idle),) + rows[2][3:]
        rows[3] = rows[3][:2] + ("not measured: there was no loaded model to unload",) + rows[3][3:]
    return rows


def finish(rows, refusal=None):
    if refusal:
        print("footprint: %s" % refusal); print("0 checks, 0 failed, 1 not judged"); return 1
    bad = [r for r in rows if r[4] is True]
    nj = [r for r in rows if r[4] is None]
    for name, label, text, budget, over in rows:
        print("footprint: %-38s %s (budget %s %s) %s" % (label, text, name, budget, {True: "OVER", False: "ok", None: "NOT JUDGED"}[over]))
    for name, label, text, budget, over in bad:
        print("RED: %s: %s measured %s, budget %s" % (name, label, text, budget))
    print("%d checks, %d failed, %d not judged" % (len(rows), len(bad), len(nj)))
    return 1 if bad or nj or not rows else 0


# ---------------------------------------------------------------- the meters

def pids_of(prefix=None, pid=None):
    """The pids whose executable starts with `prefix`, or `pid` itself while it runs."""
    out = subprocess.run(["ps", "-axo", "pid=,command="], capture_output=True, text=True).stdout
    found = []
    for line in out.splitlines():
        f = line.split(None, 1)
        if len(f) == 2 and ((pid is not None and int(f[0]) == pid) or (prefix and f[1].startswith(prefix))):
            found.append(int(f[0]))
    return found


def rusage(pid):
    """(resident MB, physical footprint MB, CPU seconds) of one process from proc_pid_rusage (flavor 2), or None. The footprint is what Activity
    Monitor shows as Memory: dirty and compressed pages count, so it does not shrink when the OS squeezes an idle process out of RAM."""
    try:
        import ctypes
        lib, tb = ctypes.CDLL("/usr/lib/libproc.dylib"), (ctypes.c_uint32 * 2)()
        ctypes.CDLL(None).mach_timebase_info(ctypes.byref(tb))
        buf = (ctypes.c_uint64 * 64)()
        if lib.proc_pid_rusage(pid, 2, ctypes.byref(buf)) != 0:
            return None
        return buf[8] / 1048576.0, buf[9] / 1048576.0, (buf[2] + buf[3]) * tb[0] / tb[1] / 1e9      # words: uuid x2, user, system, ..., resident (8), phys_footprint (9)
    except (OSError, AttributeError):
        return None


def visible_windows(pids):
    """How many windows of these pids the window server lists as on screen AND drawn (CGWindowListCopyWindowInfo, kCGWindowListOptionOnScreenOnly,
    kCGWindowAlpha above 0.01). The hidden rig keeps its window ordered in at alpha 0, click-through, so the server lists it; the owner sees nothing.
    A window at any visible alpha counts. None when the window list cannot be read."""
    try:
        import ctypes
        cf = ctypes.CDLL("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")
        cg = ctypes.CDLL("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics")
        cf.CFStringCreateWithCString.restype = ctypes.c_void_p
        cf.CFStringCreateWithCString.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_uint32]
        cf.CFArrayGetCount.restype = ctypes.c_long
        cf.CFArrayGetCount.argtypes = [ctypes.c_void_p]
        cf.CFArrayGetValueAtIndex.restype = ctypes.c_void_p
        cf.CFArrayGetValueAtIndex.argtypes = [ctypes.c_void_p, ctypes.c_long]
        cf.CFDictionaryGetValue.restype = ctypes.c_void_p
        cf.CFDictionaryGetValue.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
        cf.CFNumberGetValue.restype = ctypes.c_bool
        cf.CFNumberGetValue.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p]
        cf.CFRelease.argtypes = [ctypes.c_void_p]
        cg.CGWindowListCopyWindowInfo.restype = ctypes.c_void_p
        cg.CGWindowListCopyWindowInfo.argtypes = [ctypes.c_uint32, ctypes.c_uint32]
        key = cf.CFStringCreateWithCString(None, b"kCGWindowOwnerPID", 0x08000100)
        akey = cf.CFStringCreateWithCString(None, b"kCGWindowAlpha", 0x08000100)
        arr = cg.CGWindowListCopyWindowInfo(1, 0)         # kCGWindowListOptionOnScreenOnly, kCGNullWindowID
        if not arr:
            return None
        n, mine = 0, set(pids)
        for i in range(cf.CFArrayGetCount(arr)):
            val = cf.CFDictionaryGetValue(cf.CFArrayGetValueAtIndex(arr, i), key)
            pid = ctypes.c_int(0)
            if val and cf.CFNumberGetValue(val, 9, ctypes.byref(pid)) and pid.value in mine:      # 9 = kCFNumberIntType
                av, alpha = cf.CFDictionaryGetValue(cf.CFArrayGetValueAtIndex(arr, i), akey), ctypes.c_double(1.0)
                if av:
                    cf.CFNumberGetValue(av, 6, ctypes.byref(alpha))                                  # 6 = kCFNumberFloat64Type
                if alpha.value > 0.01:
                    n += 1
        cf.CFRelease(arr); cf.CFRelease(key); cf.CFRelease(akey)
        return n
    except (OSError, AttributeError):
        return None


def app_reads(bundle, word):
    """True when an executable of the bundle carries `word` (the name of an environment switch the app reads); False when none does; None
    when the bundle cannot be read. Round 11: WHOOSH_EMBED_MUTANT and WHOOSH_EMBED_IDLE_UNLOAD_S are set by this gate but no file under
    Sources/ reads them (grep at 7e9f24a3a), so main112's `--mutant keep --unload-s 20` ran an app with the ledger's 300 s unload and no mutant."""
    macos = os.path.join(bundle, "Contents", "MacOS")
    needle = word.encode()
    try:
        for name in sorted(os.listdir(macos)):
            path = os.path.join(macos, name)
            if not os.path.isfile(path):
                continue
            tail = b""
            with open(path, "rb") as f:
                while chunk := f.read(8 * 1024 * 1024):
                    if needle in tail + chunk:
                        return True
                    tail = chunk[-len(needle):]
    except OSError:
        return None
    return False


def switch_refusal(bundle, unload, mutant):
    """The words that refuse a run whose test-only switch the app does not read (None when every switch asked for is read)."""
    for asked, word, what in ((unload, "WHOOSH_EMBED_IDLE_UNLOAD_S", "--unload-s"), (mutant, "WHOOSH_EMBED_MUTANT", "--mutant")):
        if asked and app_reads(bundle, word) is not True:
            return ("not judged: %s sets %s, which this app does not read (no such string in %s/Contents/MacOS): the run would measure the "
                    "ledger's unload and no mutant, and a broken copy that cannot break proves nothing" % (what, word, bundle))
    return None


def supports_hidden(bundle):
    """True when the bundle declares the hidden rig mode: Contents/Resources/rig-hidden.ok (written by the build; what scripts/launch-one.sh tests)
    OR an executable in Contents/MacOS that carries the WHOOSH_RIG_HIDDEN string. Either one passes, so the two guards cannot disagree here;
    the launcher still exits 6 (HELD) when the marker is missing."""
    if os.path.isfile(os.path.join(bundle, "Contents", "Resources", "rig-hidden.ok")):
        return True
    macos = os.path.join(bundle, "Contents", "MacOS")
    try:
        for name in sorted(os.listdir(macos)):
            path = os.path.join(macos, name)
            if not os.path.isfile(path):
                continue
            tail = b""
            with open(path, "rb") as f:
                while chunk := f.read(8 * 1024 * 1024):
                    if b"WHOOSH_RIG_HIDDEN" in tail + chunk:
                        return True
                    tail = chunk[-32:]
    except OSError:
        return False
    return False


class Meter:
    def __init__(self, prefix=None, pid=None):
        self.prefix, self.pid = prefix, pid

    def read(self):
        """(resident MB, footprint MB, CPU seconds) summed over the app's processes, or None when there are none."""
        r = [x for x in (rusage(p) for p in pids_of(self.prefix, self.pid)) if x]
        return tuple(sum(x[i] for x in r) for i in range(3)) if r else None


def rig_prefix():
    d = os.environ.get("D") or os.environ.get("WHOOSH_SOCKET_DIR") or ""
    apps = [a for a in (os.listdir(os.path.join(d, "rig")) if os.path.isdir(os.path.join(d, "rig")) else []) if a.endswith(".app")]
    return os.path.join(d, "rig", apps[0], "Contents", "MacOS") + "/" if apps else None


# ---------------------------------------------------------------- the drive

class Run:
    def __init__(self, connect, meter, idle_s, cpu_s, unload_s, query, margin_s=UNLOAD_MARGIN_S, measure="rss"):
        self.connect, self.timeouts, self.guard_windows, self.fixture, self.late_fixture = connect, 0, False, None, None
        self.ctl, self.meter, self.idle_s, self.cpu_s, self.unload_s, self.query, self.margin_s, self.measure = connect(), meter, idle_s, cpu_s, unload_s, query, margin_s, measure
        self.samples, self.t0, self.m = [], time.monotonic(), {}

    def read(self, phase):
        r = self.meter.read()
        if r is None:
            raise Unreadable("the app process is gone (no process under the rig copy)")
        if not r[0] > 0:
            raise Unreadable("the app's resident size read %s MB, which is not a reading of a running app" % r[0])
        if self.guard_windows:
            pids = pids_of(self.meter.prefix, self.meter.pid)
            n = visible_windows(pids)
            self.m["visible_windows"] = n if n is not None else self.m.get("visible_windows", 0)
            if n is None:
                raise Unreadable("the window list cannot be read, so 'no window on screen' cannot be shown")
            if n:
                for p in pids:
                    try:
                        os.kill(p, 15)
                    except OSError:
                        pass
                raise Unreadable("%d window(s) of the app are on screen: the rig is not hidden; killed pid(s) %s" % (n, pids))
        self.samples.append({"t": round(time.monotonic() - self.t0, 1), "phase": phase, "rss_mb": round(r[0], 1), "footprint_mb": round(r[1], 1), "cpu_s": round(r[2], 3)})
        return (r[0] if self.measure == "rss" else r[1]), r[2]

    def load_fixture(self, name):
        m, t0 = self.m, time.monotonic()
        m["fixture"] = name
        try:
            self.ctl.sock.settimeout(FIXTURE_LOAD_WAIT_S)      # 300,000 messages: the ledger says 0.7 s on a quiet Mac; a loaded one takes far longer than the socket's usual 5 s
            self.ctl.send("fixture", "load", name)
            self.ctl.sock.settimeout(5.0)
        except Unreadable as e:
            raise Unreadable("the fixture did not load, so the run is not the one asked for: %s" % e)
        m["fixture_load_s"] = time.monotonic() - t0

    def tolerant(self, ask):
        """One socket call that may meet a busy main thread (the first search loads the model on it, and every verb runs there). A timeout opens a
        fresh connection, because the late reply would otherwise answer the next call; the call returns None."""
        try:
            return ask(self.ctl)
        except Unreadable:
            self.timeouts += 1
            self.ctl.close()
            self.ctl = self.connect()
            return None

    def wait(self, seconds, phase):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            time.sleep(min(SAMPLE_S, max(0.0, end - time.monotonic()))); self.read(phase)

    def go(self):
        m = self.m
        m["launch_ms"] = float(self.ctl.measure("mac-main.launch")["ms"])
        m["footprint_first_frame"] = self.read("first-frame")[0]
        if self.fixture:
            # a large corpus (docs/CONTROL-SOCKET.md `fixture load full-history-<n>`): memory before and right after it loads, then the idle protocol as usual
            self.load_fixture(self.fixture)
            self.wait(SAMPLE_S, "after-fixture")
            m["footprint_after_fixture"] = self.samples[-1]["footprint_mb" if self.measure == "footprint" else "rss_mb"]
        self.wait(self.idle_s, "idle")
        m["rss_idle"], cpu0 = self.read("idle-end")[0], self.read("cpu-start")[1]
        w0 = time.monotonic()
        self.wait(self.cpu_s, "cpu-window")
        cpu1 = self.read("cpu-end")[1]
        m["cpu_pct"] = (cpu1 - cpu0) / (time.monotonic() - w0) * 100.0
        t_type = time.monotonic()
        self.tolerant(lambda c: c.key("/")); self.tolerant(lambda c: c.key("⌘A")); self.tolerant(lambda c: c.type(self.query))
        self.tolerant(lambda c: c.key("Enter"))        # results come on Enter, not per keystroke (searchmac S1)
        t_end = t_type + SEARCH_WAIT_S
        while not self.tolerant(lambda c: c.rows(RESULTS)):
            if time.monotonic() > t_end:
                raise Unreadable("the search answered no result in %d s" % SEARCH_WAIT_S)
            time.sleep(0.5); self.read("search-wait")
        t_found = time.monotonic()
        m["search_s"] = t_found - t_type
        m["t_found"] = t_found - self.t0
        self.wait(SETTLE_S, "search-settle")
        m["rss_model"] = max(s["rss_mb" if self.measure == "rss" else "footprint_mb"] for s in self.samples[-2:])
        self.tolerant(lambda c: c.key("Escape"))
        if self.late_fixture:
            # the large fixture arrives AFTER the model is loaded: a meaning search over 300,000 rows would embed every row (hours), so the model states are
            # taken on the small corpus and the large corpus is added on top; the unloaded reading then holds the large corpus and no model
            self.load_fixture(self.late_fixture)
            self.wait(SAMPLE_S, "fixture-late")
            m["footprint_model_plus_fixture"] = self.samples[-1]["footprint_mb" if self.measure == "footprint" else "rss_mb"]
        self.wait(max(0.0, self.unload_s + self.margin_s - (time.monotonic() - t_found)), "unload-wait")
        m["rss_unloaded"] = self.read("unloaded")[0]
        return m


def write_samples(mode, run, measured, rows):
    os.makedirs(OUT_DIR, exist_ok=True)
    path = os.path.join(OUT_DIR, "%s-%s.json" % (datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%SZ"), mode))
    with open(path, "w", encoding="utf-8") as f:
        json.dump({"mode": mode, "measured": measured, "load_start": os.getloadavg(), "verdicts": [list(r) for r in rows], "samples": run.samples if run else []}, f, indent=1)
    print("footprint: samples written to %s" % os.path.relpath(path, ROOT))


def query_text():
    with open(os.path.join(ROOT, "fixtures", "tests-j2.json"), encoding="utf-8") as f:
        return json.load(f)["expect"]["meaning_query"]


def inside(argv):
    """Runs inside scripts/launch-one.sh: the app is up, $D is its run directory."""
    unload_s = int(os.environ.get("WHOOSH_EMBED_IDLE_UNLOAD_S") or const("EMBED_IDLE_UNLOAD_S"))
    prefix = rig_prefix()
    if prefix is None:
        return finish([], "not judged: no rig copy under $D/rig (run this inside scripts/launch-one.sh)")
    try:
        Ctl().connect().close()
    except CouldNotDrive as e:
        return finish([], "not judged: could not drive: %s" % e)
    measure = os.environ.get("FOOTPRINT_MEASURE", "footprint")
    print("footprint: memory figure judged: %s (every sample also records the other one)" % {"rss": "resident size (ps rss; the ledger's word)", "footprint": "physical footprint (Activity Monitor's Memory)"}[measure])
    run = Run(lambda: Ctl().connect(), Meter(prefix=prefix), IDLE_S, CPU_S, unload_s, query_text(), measure=measure)
    run.guard_windows = True
    run.fixture = os.environ.get("FOOTPRINT_FIXTURE") or None
    if os.environ.get("FOOTPRINT_FIXTURE_LATE"):
        run.late_fixture, run.fixture = run.fixture, None
    run.m["visible_windows"] = 0
    try:
        run.go()
    except (Unreadable, OSError) as e:
        print("footprint: the run stopped: %s (what was measured before it is judged; the rest is NOT JUDGED)" % e)
    finally:
        run.ctl.close()
    m = run.m
    rows = judge(m, os.getloadavg()[0] if not os.environ.get("FOOTPRINT_ALLOW_LOAD") else float("inf"))
    if "footprint_model_plus_fixture" in m:
        print("footprint: info, not judged: fixture %s loaded AFTER the search (model resident): %.0f MB with the model and the fixture together (load took %.1f s); the unloaded row above holds the fixture and no model; an UPPER BOUND, the fixture core keeps its mail in memory" % (m["fixture"], m["footprint_model_plus_fixture"], m.get("fixture_load_s", float("nan"))))
    elif "fixture" in m:
        print("footprint: info, not judged: fixture %s: memory at first frame %.0f MB, %.0f MB right after it loaded (load took %.1f s); this fixture holds its mail in memory, so every figure here is an UPPER BOUND for a real core" % (m["fixture"], m["footprint_first_frame"], m.get("footprint_after_fixture", float("nan")), m.get("fixture_load_s", float("nan"))))
    early = [x for x in run.samples if x["phase"] == "unload-wait" and "t_found" in m and x["t"] < m["t_found"] + unload_s - SAMPLE_S]
    if early:
        key = "footprint_mb" if measure == "footprint" else "rss_mb"
        low = min(x[key] for x in early)
        print("footprint: info, not judged: lowest memory BEFORE the unload deadline (%d s after the results) %.0f MB%s" % (
            unload_s, low, "; it is already under RSS_IDLE_MAX_MB, so the drop is not the idle unload and the 'model unloaded' row cannot show it" if low < const("RSS_IDLE_MAX_MB") else ""))
    if "search_s" in m:
        print("footprint: info, not judged: the first meaning search returned its results after %.1f s; %d control-socket reads timed out meanwhile (the app's main thread was busy loading the model)" % (m["search_s"], run.timeouts))
    write_samples("app", run, m, rows)
    return finish(rows)


# ---------------------------------------------------------------- the real gate (outer) and the fake process

def find_weights(arg):
    if arg:
        return arg
    if os.environ.get("WHOOSH_EMBED_WEIGHTS"):
        return os.environ["WHOOSH_EMBED_WEIGHTS"]
    d = ROOT
    for _ in range(6):
        cand = os.path.join(d, "rig", "embedding", "weights")
        if os.path.isdir(cand):
            return cand
        d = os.path.dirname(d)
    return None


def outer(argv):
    arg = lambda k: argv[argv.index(k) + 1] if k in argv else None
    bundle = next((a for a in argv if a.endswith(".app")), None) or arg("--bundle") or os.environ.get("WHOOSH_BUNDLE")
    unload, mutant = arg("--unload-s"), arg("--mutant")
    allow_load = "--allow-load" in argv
    refusal = header(bundle, False)   # round 8: the load no longer refuses; --allow-load is kept and changes nothing
    if refusal:
        return finish([], refusal)
    if not supports_hidden(bundle):
        return finish([], "HELD, not launched: %s has no hidden rig mode (no Contents/Resources/rig-hidden.ok, and no WHOOSH_RIG_HIDDEN in its executables). A rig window may never show on the owner's screen (owner 2026-11-08 16:30); build from a tree with appshell round 3 (--rig-hidden)" % bundle)
    weights = find_weights(arg("--weights"))
    env = dict(os.environ)
    kv = []
    if weights:
        kv.append("WHOOSH_EMBED_WEIGHTS=%s" % weights)
    print("footprint: embedding weights: %s" % (weights or "NONE FOUND (checks 4 and 5 will be NOT JUDGED); pass --weights DIR"))
    refused = switch_refusal(bundle, unload, mutant)
    if refused:
        return finish([], refused)
    if unload and int(unload) < MIN_UNLOAD_S:
        return finish([], "not judged: --unload-s %s is under %d s, which a search plus the settle time already uses up" % (unload, MIN_UNLOAD_S))
    if unload:
        kv.append("WHOOSH_EMBED_IDLE_UNLOAD_S=%d" % int(unload))
        print("footprint: TEST-ONLY: WHOOSH_EMBED_IDLE_UNLOAD_S=%s in the app (ledger EMBED_IDLE_UNLOAD_S %s); the unload wait is %d s, not %d s" % (unload, const("EMBED_IDLE_UNLOAD_S"), int(unload) + UNLOAD_MARGIN_S, const("EMBED_IDLE_UNLOAD_S") + UNLOAD_MARGIN_S))
    if mutant:
        kv.append("WHOOSH_EMBED_MUTANT=%s" % mutant)
        print("footprint: MUTANT %s (WHOOSH_EMBED_MUTANT): this run is meant to go RED" % mutant)
    kv.append("WHOOSH_RIG_HIDDEN=1")
    env["LAUNCH_ENV"] = " ".join(kv)
    env["WHOOSH_RIG_HIDDEN"] = "1"          # scripts/launch-one.sh refuses (exit 6) without it
    if allow_load:
        env["FOOTPRINT_ALLOW_LOAD"] = "1"
    if arg("--fixture-load"):
        env["FOOTPRINT_FIXTURE"] = arg("--fixture-load")
        if "--after-search" in argv:
            env["FOOTPRINT_FIXTURE_LATE"] = "1"
    if arg("--measure"):
        env["FOOTPRINT_MEASURE"] = arg("--measure")
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), os.path.abspath(bundle), "--", "--fixture-core", "--rig-hidden", sys.executable, os.path.abspath(__file__), "--inside"]
    rc = subprocess.run(cmd, cwd=ROOT, env=env).returncode
    if rc == 6:
        return finish([], "HELD: hidden mode not available (scripts/launch-one.sh exited 6); nothing was launched. Not a red.")
    if rc not in (0, 1):
        return finish([], "not judged: scripts/launch-one.sh exited %d (3 no instance or no socket, 4 died, 5 a foreign copy runs, 75 busy)" % rc)
    return rc


def fake_server(sock_dir, mutant, unload_s):
    """A stand-in for the app: a control socket with the verbs this gate uses, a resident size that moves like the real one."""
    held, last, lock = [], [0.0], threading.Lock()

    def alloc():
        with lock:
            if not held:
                import mmap          # an anonymous map, so closing it returns the pages at once (freed malloc memory can linger in the footprint)
                m = mmap.mmap(-1, 500 * 1024 * 1024)
                for _ in range(500 // 10):
                    m.write(os.urandom(10 * 1024 * 1024))     # random bytes: the OS cannot compress them away
                held.append(m)

    def unloader():
        while True:
            time.sleep(0.5)
            with lock:
                if held and last[0] and mutant != "keep" and time.monotonic() - last[0] >= unload_s:
                    for m in held:
                        m.close()
                    held.clear()
    base = os.urandom(40 * 1024 * 1024)      # the resident size of the app itself
    if mutant == "launch":
        alloc()
    if mutant == "busy":
        def spin():
            while True:
                sum(range(2000000)); time.sleep(0.05)
        threading.Thread(target=spin, daemon=True).start()
    threading.Thread(target=unloader, daemon=True).start()
    path = os.path.join(sock_dir, "whoosh.sock")
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM); srv.bind(path); srv.listen(4)
    searched = [False]

    def serve(conn):
        buf = b""
        while True:
            while b"\n" not in buf:
                chunk = conn.recv(65536)
                if not chunk:
                    return
                buf += chunk
            line, buf = buf.split(b"\n", 1)
            req = json.loads(line)
            verb, reply = req["verb"], {"ok": True}
            if verb == "measure":
                reply["ms"] = 123.0
            elif verb == "type":
                searched[0] = True; time.sleep(1.0); alloc(); last[0] = time.monotonic()
            elif verb == "rows":
                reply["rows"] = [{"id": "m1"}] if searched[0] else []
            try:
                conn.sendall((json.dumps(reply) + "\n").encode())
            except OSError:
                return                                   # the reader closed before the reply left (its run is over): the stand-in's thread ends, with no trace in the output
    while True:
        c, _ = srv.accept()
        threading.Thread(target=serve, args=(c,), daemon=True).start()
    return len(base)


def fake(argv):
    mutant = argv[argv.index("--fake") + 1] if len(argv) > argv.index("--fake") + 1 and not argv[argv.index("--fake") + 1].startswith("--") else "clean"
    refusal = header(None, True)
    if refusal:
        return finish([], refusal)
    import tempfile
    d = tempfile.mkdtemp(prefix="footprint-fake-")
    unload = 8
    p = subprocess.Popen([sys.executable, os.path.abspath(__file__), "--fake-server", d, mutant, str(unload)])
    try:
        for _ in range(100):
            if os.path.exists(os.path.join(d, "whoosh.sock")):
                break
            time.sleep(0.1)
        os.environ["WHOOSH_SOCKET_DIR"] = d
        run = Run(lambda: Ctl().connect(), Meter(pid=p.pid), 4, 4, unload, "fake query", 2, "footprint")
        print("footprint: DRY RUN against a fake process (pid %d), memory figure: footprint, mutant %s: idle %d s, CPU window %d s, unload %d s + %d s" % (p.pid, mutant, 4, 4, unload, 2))
        m = run.go()
        rows = judge(m, os.getloadavg()[0])
        write_samples("fake-" + mutant, run, m, rows)
        return finish(rows)
    except (Unreadable, CouldNotDrive, OSError) as e:
        return finish([], "not judged: the fake run stopped: %s" % e)
    finally:
        p.kill()
        try:
            for f in os.listdir(d):
                os.remove(os.path.join(d, f))
            os.rmdir(d)
        except OSError:
            pass


# ---------------------------------------------------------------- crawl (perf-archive round 5)

CRAWL_SAMPLE_S = 0.5          # a sampling cadence (the plan's 100 bodies download in seconds over loopback), not a budget
CRAWL_DOWNLOAD_WAIT_S = 240   # a check bound: the sync, then every body of three folders over the local server
CRAWL_INDEX_WAIT_S = 300      # a check bound: the model loads, then every stored mail is embedded
CRAWL_PHASES = ("download", "index")


def crawl_phase(prog, total, weights):
    """The phase one sample belongs to from the engine's figures: setup until the engine reports a message (the account being added, round 11),
    download until every body is stored, then index (a model folder given)."""
    if prog is None or not prog.get("messages"):
        return "setup"
    if prog.get("bodies", 0) < total:
        return "download"
    if weights and prog.get("indexed", 0) < total:
        return "index"
    return "done"


def crawl_summary(samples, phase, peak=max):
    """(peak MB, median MB, average CPU %, highest CPU % over one interval, n) of one phase; None when the phase has no sample. `peak` is a
    parameter so the selftest can run the broken copy that hides a peak (the median in its place)."""
    import statistics
    idx = [i for i, x in enumerate(samples) if x["phase"] == phase]
    if not idx:
        return None
    mb = [samples[i]["footprint_mb"] for i in idx]
    cpu = []
    for i in idx:
        if i > 0 and samples[i]["t"] > samples[i - 1]["t"]:
            cpu.append((samples[i]["cpu_s"] - samples[i - 1]["cpu_s"]) / (samples[i]["t"] - samples[i - 1]["t"]) * 100.0)
    first = idx[0] - 1 if idx[0] > 0 else idx[0]
    span = samples[idx[-1]]["t"] - samples[first]["t"]
    avg = (samples[idx[-1]]["cpu_s"] - samples[first]["cpu_s"]) / span * 100.0 if span > 0 else None
    return peak(mb), statistics.median(mb), avg, (max(cpu) if cpu else None), len(idx)


def crawl_cpu_limit():
    try:
        return float(const("CRAWL_CPU_MAX_PCT"))
    except KeyError:
        return None


def crawl_judge(samples, finished, weights, load_end=None, peak=max):
    """Verdict rows (case, text, over True/False/None). `finished`: phase -> True when the phase reached its end inside its wait."""
    mem = const("RSS_IDLE_MAX_MB") + const("EMBED_INDEX_CEILING_MB")
    cpu_lim, lim = crawl_cpu_limit(), load_limit()
    busy = load_end is not None and lim is not None and load_end > lim
    rows = []
    for ph in CRAWL_PHASES:
        sm = crawl_summary(samples, ph, peak)
        why = None
        if ph == "index" and not weights:
            why = "no model folder given (--weights or WHOOSH_EMBED_WEIGHTS): the index phase does not run"
        elif sm is None:
            why = "no sample in this phase"
        elif not finished.get(ph):
            why = "the %s did not finish inside its wait, so its peak may be later" % ph
            if sm[0] >= mem:   # round 11: a peak already over the budget is OVER whatever comes later (it is a lower bound)
                rows.append(("crawl-%s-memory-peak" % ph, "peak %.0f MB over %d samples of an unfinished %s, already over the budget %s MB" % (sm[0], sm[4], ph, mem), True))
                rows.append(("crawl-%s-cpu-average" % ph, "not judged: %s" % why, None))
                continue
        if why:
            rows.append(("crawl-%s-memory-peak" % ph, "not judged: %s" % why, None))
            rows.append(("crawl-%s-cpu-average" % ph, "not judged: %s" % why, None))
            continue
        pk, med, avg, top, n = sm
        if not (pk > 0):
            rows.append(("crawl-%s-memory-peak" % ph, "not judged: the footprint read %s MB, not a reading of a running app" % pk, None))
        else:
            rows.append(("crawl-%s-memory-peak" % ph, "peak %.0f MB, median %.0f MB over %d samples (budget RSS_IDLE_MAX_MB %s + EMBED_INDEX_CEILING_MB %s = %s MB)" % (
                pk, med, n, const("RSS_IDLE_MAX_MB"), const("EMBED_INDEX_CEILING_MB"), mem), pk >= mem))
        cpu_text = "average %s %%, highest over one %.1f s interval %s %%" % ("%.1f" % avg if avg is not None else "?", CRAWL_SAMPLE_S, "%.1f" % top if top is not None else "?")
        if cpu_lim is None:
            rows.append(("crawl-%s-cpu-average" % ph, "%s; not judged: docs/CONSTANTS.md holds no CRAWL_CPU_MAX_PCT yet" % cpu_text, None))
        elif avg is None:
            rows.append(("crawl-%s-cpu-average" % ph, "not judged: the phase lasted one sample, no CPU average", None))
        else:
            rows.append(("crawl-%s-cpu-average" % ph, "%s (budget CRAWL_CPU_MAX_PCT %s %%)" % (cpu_text, cpu_lim), avg >= cpu_lim))
    return rows


def crawl_finish(rows, refusal=None):
    """One line per budget: PASS, OVER or NOT JUDGED; last line `N checks, F failed, U not judged`. Exit 1 on an OVER only: a CPU row
    waiting for its ledger number, or an index phase run without a model folder, is printed NOT JUDGED and does not fail the run."""
    if refusal:
        print("footprint crawl: %s" % refusal); print("0 checks, 0 failed, 1 not judged"); return 1
    for case, text, over in rows:
        print("%s %s: %s" % ({True: "OVER", False: "PASS", None: "NOT JUDGED"}[over], case, text))
    bad = [r for r in rows if r[2] is True]
    nj = [r for r in rows if r[2] is None]
    print("%d checks, %d failed, %d not judged" % (len(rows), len(bad), len(nj)))
    return 1 if bad or not any(r[2] is False for r in rows) else 0


def realcore_check():
    """checks/realcore-check.py as a module (its file name has a dash): its account sheet drive and its progress read, never copied here."""
    import importlib.util
    sys.path.insert(0, os.path.join(ROOT, "checks"))
    spec = importlib.util.spec_from_file_location("realcore_check", os.path.join(ROOT, "checks", "realcore-check.py"))
    mod = importlib.util.module_from_spec(spec); spec.loader.exec_module(mod)
    return mod


def inside_crawl(argv):
    """Inside scripts/launch-one.sh on the real engine: add the local account, then sample the footprint, CPU and the engine's figures."""
    rc = realcore_check()                 # puts checks/ on the path first
    import realcore_mailfake as fake      # noqa: E402
    prefix = rig_prefix()
    if prefix is None:
        return crawl_finish([], "not judged: no rig copy under $D/rig (run this inside scripts/launch-one.sh)")
    weights = bool(os.environ.get("WHOOSH_EMBED_WEIGHTS"))
    total = sum(n for _, n in fake.CRAWL_PLAN)
    meter = Meter(prefix=prefix)
    samples, finished, t0 = [], {}, time.monotonic()
    try:
        c = Ctl().connect()
        answer = {"driver_errors": []}
        # round 11 (main112 alone-010: "no sample in this phase"): the 100 small bodies were stored before the first sample, which came after
        # the account was added. Now the sampling runs from before the add, which runs on its own connection in a thread.
        import threading
        adder = threading.Thread(target=lambda: rc.mv_add_account(Ctl().connect(), fake, answer, 3), daemon=True)
        adder.start()
        ends = {"setup": t0 + CRAWL_DOWNLOAD_WAIT_S, "download": t0 + CRAWL_DOWNLOAD_WAIT_S}
        while True:
            r = meter.read()
            if r is None:
                raise Unreadable("the app process is gone (no process under the rig copy)")
            n = visible_windows(pids_of(prefix))
            if n:
                raise Unreadable("%d window(s) of the app are on screen: the rig is not hidden" % n)
            prog = rc.crawl_progress(c)
            ph = crawl_phase(prog, total, weights)
            now = time.monotonic()
            if ph not in ("setup", "download") and "download" not in finished:
                ph = "download"   # the sample that sees every body stored closes the download (it holds the download's last interval)
            samples.append({"t": round(now - t0, 2), "phase": ph, "rss_mb": round(r[0], 1), "footprint_mb": round(r[1], 1), "cpu_s": round(r[2], 3),
                            "bodies": (prog or {}).get("bodies"), "indexed": (prog or {}).get("indexed")})
            if samples[-1]["bodies"] is not None and samples[-1]["bodies"] >= total and "download" not in finished:
                finished["download"] = True
                ends["index"] = now + CRAWL_INDEX_WAIT_S
                ph = crawl_phase(prog, total, weights)
            if ph == "done":
                finished["index"] = weights
                break
            if now > ends.get(ph, now):
                print("footprint crawl: the %s did not finish in its wait (bodies %s, indexed %s of %d)" % (ph, (prog or {}).get("bodies"), (prog or {}).get("indexed"), total))
                break
            time.sleep(CRAWL_SAMPLE_S)
        # round 11 (main112: indexed 0 of 100 in 300 s with a model folder): the shell's own row says whether the engine had the model
        try:
            bg = (c.rows("mac-main.background") or [{}])[0]
            print("footprint crawl: info, not judged: mac-main.background embedder %s, handovers %s, crawl %s, reason %s; account add errors %s" % (
                bg.get("embedder"), bg.get("handovers"), bg.get("crawl"), bg.get("reason"), answer.get("driver_errors") or "none"))
        except (Unreadable, KeyError, TypeError, IndexError) as e:
            print("footprint crawl: info, not judged: mac-main.background unreadable (%s)" % e)
        c.close()
    except (Unreadable, CouldNotDrive, OSError) as e:
        print("footprint crawl: the run stopped: %s (what was measured before it is shown; unfinished phases are NOT JUDGED)" % e)
    print("footprint crawl: %d samples over %.1f s; model folder %s; account added by checks/realcore-check.py mv_add_account; %d messages in the plan" % (
        len(samples), samples[-1]["t"] if samples else 0.0, "given" if weights else "NOT given (the index phase does not run)", total))
    rows = crawl_judge(samples, finished, weights, os.getloadavg()[0] if not os.environ.get("FOOTPRINT_ALLOW_LOAD") else float("inf"))
    write_samples("crawl", type("R", (), {"samples": samples})(), {"finished": finished, "weights": weights, "total": total}, [list(r) for r in rows])
    return crawl_finish(rows)


def outer_crawl(argv):
    arg = lambda k: argv[argv.index(k) + 1] if k in argv else None
    bundle = next((a for a in argv if a.endswith(".app")), None) or arg("--bundle") or os.environ.get("WHOOSH_BUNDLE")
    refusal = header(bundle, False)   # round 8: the load no longer refuses; --allow-load is kept and changes nothing
    if refusal:
        return crawl_finish([], refusal)
    if not supports_hidden(bundle):
        return crawl_finish([], "HELD, not launched: %s has no hidden rig mode" % bundle)
    sys.path.insert(0, os.path.join(ROOT, "checks"))
    import realcore_mailfake as fake      # noqa: E402
    try:
        mail, imap, smtp = fake.start(0, 0, inbox=0, act=True)
    except OSError as e:
        return crawl_finish([], "not judged: the local servers did not start: %s" % e)
    fake.load_crawl_mail(mail)
    weights = find_weights(arg("--weights")) if (arg("--weights") or os.environ.get("WHOOSH_EMBED_WEIGHTS")) else None
    kv = ["WHOOSH_RIG_HIDDEN=1", "WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=%d:%d" % (imap.server_address[1], smtp.server_address[1])]
    if weights:
        kv.append("WHOOSH_EMBED_WEIGHTS=%s" % weights)
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(kv))
    if weights:
        env["WHOOSH_EMBED_WEIGHTS"] = weights
    if "--allow-load" in argv:
        env["FOOTPRINT_ALLOW_LOAD"] = "1"
    print("footprint crawl: the real engine against the local server (INBOX 25, Archive 65, Sent 10); model folder: %s" % (weights or "NONE (the index phase is NOT JUDGED)"))
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), os.path.abspath(bundle), "--", "--rig-hidden", sys.executable, os.path.abspath(__file__), "--inside-crawl"]
    try:
        rc = subprocess.run(cmd, cwd=ROOT, env=env).returncode
    finally:
        imap.shutdown(); smtp.shutdown()
    if rc == 6:
        return crawl_finish([], "HELD: hidden mode not available (scripts/launch-one.sh exited 6); nothing was launched. Not a red.")
    if rc not in (0, 1):
        return crawl_finish([], "not judged: scripts/launch-one.sh exited %d" % rc)
    return rc


def selftest_crawl():
    """Cases for the crawl reading; returns (checks, failed)."""
    mem = const("RSS_IDLE_MAX_MB") + const("EMBED_INDEX_CEILING_MB")
    def run(fp_download, fp_index, cpu_per_s=0.2, spike_at=None, spike=None):
        out, cpu, t = [], 0.0, 0.0
        for i, (ph, fp) in enumerate([("download", fp_download)] * 6 + [("index", fp_index)] * 6):
            t += CRAWL_SAMPLE_S; cpu += cpu_per_s * CRAWL_SAMPLE_S
            out.append({"t": t, "phase": ph, "footprint_mb": spike if i == spike_at else fp, "cpu_s": cpu})
        return out
    both = {"download": True, "index": True}
    verdict = lambda rows, case: next(r[2] for r in rows if r[0] == case)
    cases = [
        ("a crawl under the memory budget passes both phases", lambda: [verdict(crawl_judge(run(400, 900), both, True), "crawl-%s-memory-peak" % p) for p in CRAWL_PHASES] == [False, False]),
        ("the index phase at the memory budget is OVER", lambda: verdict(crawl_judge(run(400, mem), both, True), "crawl-index-memory-peak") is True),
        ("one sample's spike over the budget is OVER (the peak, not the median)", lambda: verdict(crawl_judge(run(400, 900, spike_at=2, spike=mem + 50), both, True), "crawl-download-memory-peak") is True),
        ("BROKEN COPY: a summary that takes the median hides that spike (must read PASS, proving the case above can see it)",
         lambda: verdict(crawl_judge(run(400, 900, spike_at=2, spike=mem + 50), both, True, peak=__import__("statistics").median), "crawl-download-memory-peak") is False),
        ("no model folder: the index phase is NOT JUDGED, never a pass", lambda: verdict(crawl_judge(run(400, 900), {"download": True}, False), "crawl-index-memory-peak") is None),
        ("a download that never finished is NOT JUDGED", lambda: verdict(crawl_judge(run(400, 900), {}, True), "crawl-download-memory-peak") is None),
        ("the CPU average is the phase's CPU seconds over its time (0.2 s per s = 20 %)", lambda: abs(crawl_summary(run(400, 900), "download")[2] - 20.0) < 1e-6),
        ("without a CRAWL_CPU_MAX_PCT row the CPU line is printed and NOT JUDGED", lambda: crawl_cpu_limit() is not None or verdict(crawl_judge(run(400, 900), both, True), "crawl-download-cpu-average") is None),
        ("a 0 MB footprint is NOT JUDGED (not a reading of a running app)", lambda: verdict(crawl_judge(run(0, 900), both, True), "crawl-download-memory-peak") is None),
        ("the phase follows the engine's figures (setup until a message is reported, round 11)",
         lambda: [crawl_phase(None, 100, True), crawl_phase({"messages": 0, "bodies": 0}, 100, True), crawl_phase({"messages": 100, "bodies": 99, "indexed": 0}, 100, True),
                  crawl_phase({"messages": 100, "bodies": 100, "indexed": 3}, 100, True), crawl_phase({"messages": 100, "bodies": 100, "indexed": 3}, 100, False),
                  crawl_phase({"messages": 100, "bodies": 100, "indexed": 100}, 100, True)] == ["setup", "setup", "download", "index", "done", "done"]),
        ("an unfinished index whose peak is already over the budget is OVER (a lower bound)", lambda: verdict(crawl_judge(run(400, mem + 10), {"download": True}, True), "crawl-index-memory-peak") is True),
        ("an unfinished index under the budget stays NOT JUDGED (its peak may come later)", lambda: verdict(crawl_judge(run(400, 900), {"download": True}, True), "crawl-index-memory-peak") is None),
        ("setup samples are judged in no phase", lambda: crawl_summary([{"t": 0.5, "phase": "setup", "footprint_mb": 5000, "cpu_s": 0.1}] + run(400, 900), "download")[0] == 400),
        ("only NOT JUDGED rows exit 1, an OVER exits 1, a PASS beside NOT JUDGED rows exits 0",
         lambda: [crawl_finish([("x", "t", None)]), crawl_finish([("x", "t", False), ("y", "t", True)]), crawl_finish([("x", "t", False), ("y", "t", None)])] == [1, 1, 0]),
    ]
    bad = 0
    for name, f in cases:
        ok = bool(f()); bad += not ok
        print("%-4s crawl: %s" % ("ok" if ok else "BAD", name))
    return len(cases), bad


def selftest():
    idle, model, cpu, warm = const("RSS_IDLE_MAX_MB"), const("RSS_MODEL_MAX_MB"), const("CPU_IDLE_MAX_PCT"), const("LAUNCH_COLD_MS")
    good = {"rss_idle": idle - 1, "cpu_pct": cpu - 0.01, "rss_model": model - 1, "rss_unloaded": idle - 1, "launch_ms": warm - 1}
    cases = [("all just under", good, [False] * 5), ("idle RSS at budget", dict(good, rss_idle=idle), [True, False, False, False, False]),
             ("CPU at budget", dict(good, cpu_pct=cpu), [False, True, False, False, False]), ("model RSS at budget", dict(good, rss_model=model), [False, False, True, False, False]),
             ("unloaded RSS at budget (model never left)", dict(good, rss_unloaded=idle), [False, False, False, True, False]),
             ("launch at budget", dict(good, launch_ms=warm), [False, False, False, False, True]),
             ("main58's 1739 ms launch of a new copy, judged as cold", dict(good, launch_ms=1739.0), [False, False, False, False, 1739.0 >= warm]),
             ("model loaded at launch (idle RSS holds the model)", dict(good, rss_idle=model - 1), [True, False, False, False, False]),
             ("model never unloads", dict(good, rss_unloaded=model - 1), [False, False, False, True, False]),
             ("search never reached the model", dict(good, rss_model=idle - 1), [False, False, None, None, False]),
             ("nothing measured", {}, [None] * 5),
             ("idle RSS read 0 MB (main83's memory-mac passed on 0)", dict(good, rss_idle=0.0), [None, False, False, False, False]),
             ("unloaded RSS read 0 MB", dict(good, rss_unloaded=0.0), [False, False, False, None, False]),
             ("launch read 0 ms", dict(good, launch_ms=0.0), [False, False, False, False, None])]
    bad = n_checks = 0
    for name, m, want in cases:
        got = [r[4] for r in judge(m)]
        ok = got == want; bad += not ok; n_checks += 1
        print("%-4s %-50s over=%s" % ("ok" if ok else "BAD", name, got))
    import tempfile
    with tempfile.TemporaryDirectory() as t:
        for name, want in (("bundle with rig-hidden.ok", True), ("bundle without it", False)):
            os.makedirs(os.path.join(t, name, "Contents", "Resources"))
            if want:
                open(os.path.join(t, name, "Contents", "Resources", "rig-hidden.ok"), "w").close()
            got = supports_hidden(os.path.join(t, name)); ok = got == want; bad += not ok; n_checks += 1
            print("%-4s %-50s supports=%s" % ("ok" if ok else "BAD", name, got))
        os.makedirs(os.path.join(t, "exe", "Contents", "MacOS"))
        with open(os.path.join(t, "exe", "Contents", "MacOS", "x"), "wb") as f:
            f.write(b"\0" * 100 + b"..WHOOSH_RIG_HIDDEN..")
        got = supports_hidden(os.path.join(t, "exe")); ok = got is True; bad += not ok; n_checks += 1
        print("%-4s %-50s supports=%s" % ("ok" if ok else "BAD", "bundle whose executable carries the string", got))
    allp = [int(x) for x in subprocess.run(["ps", "-axo", "pid="], capture_output=True, text=True).stdout.split()]
    n = visible_windows(allp); ok = bool(n); bad += not ok; n_checks += 1
    print("%-4s %-50s windows=%s" % ("ok" if ok else "BAD", "window detector sees on-screen windows of all pids", n))
    n = visible_windows([os.getpid()]); ok = n == 0; bad += not ok; n_checks += 1
    print("%-4s %-50s windows=%s" % ("ok" if ok else "BAD", "window detector sees none for this script", n))
    rows = judge(dict(good, visible_windows=1)); ok = rows[-1][4] is True; bad += not ok; n_checks += 1
    print("%-4s %-50s over=%s" % ("ok" if ok else "BAD", "one window on screen is red", rows[-1][4]))
    lim = load_limit()
    busy = [r[4] for r in judge(good, (lim or 0) + 1)]
    ok = lim is not None and busy == [False, False, False, False, False]
    bad += not ok; n_checks += 1
    print("%-4s %-50s over=%s" % ("ok" if ok else "BAD", "load above PERF_LOAD_MAX: every figure inside its budget passes", busy))
    busy = [r[4] for r in judge(dict(good, launch_ms=warm + 100), (lim or 0) + 1)]
    ok = lim is not None and busy == [False, False, False, False, None]
    bad += not ok; n_checks += 1
    print("%-4s %-50s over=%s" % ("ok" if ok else "BAD", "load above PERF_LOAD_MAX: a launch over its budget is NOT JUDGED", busy))
    busy = [r[4] for r in judge(dict(good, cpu_pct=cpu + 1, rss_idle=idle + 1), (lim or 0) + 1)]
    ok = lim is not None and busy == [True, True, False, False, False]
    bad += not ok; n_checks += 1
    print("%-4s %-50s over=%s" % ("ok" if ok else "BAD", "load above PERF_LOAD_MAX: memory and the app's CPU over budget stay OVER", busy))
    for name, ok in loadgate.selftest_cases():
        bad += not ok; n_checks += 1
        print("%-4s %s" % ("ok" if ok else "BAD", name))
    with tempfile.TemporaryDirectory() as t:
        os.makedirs(os.path.join(t, "reads.app", "Contents", "MacOS")); os.makedirs(os.path.join(t, "deaf.app", "Contents", "MacOS"))
        open(os.path.join(t, "reads.app", "Contents", "MacOS", "x"), "wb").write(b"\0" * 64 + b"WHOOSH_EMBED_MUTANT WHOOSH_EMBED_IDLE_UNLOAD_S")
        open(os.path.join(t, "deaf.app", "Contents", "MacOS", "x"), "wb").write(b"\0" * 64 + b"WHOOSH_RIG_HIDDEN")
        for name, ok in (("round 11: a mutant run on an app that does not read WHOOSH_EMBED_MUTANT is refused, never measured",
                          (switch_refusal(os.path.join(t, "deaf.app"), None, "keep") or "").startswith("not judged") and "WHOOSH_EMBED_MUTANT" in switch_refusal(os.path.join(t, "deaf.app"), None, "keep")),
                         ("round 11: a shortened unload on an app that does not read WHOOSH_EMBED_IDLE_UNLOAD_S is refused", "WHOOSH_EMBED_IDLE_UNLOAD_S" in (switch_refusal(os.path.join(t, "deaf.app"), "20", None) or "")),
                         ("round 11: an app that reads both switches runs the mutant", switch_refusal(os.path.join(t, "reads.app"), "20", "keep") is None),
                         ("round 11: the plain run (no switch) is never refused", switch_refusal(os.path.join(t, "deaf.app"), None, None) is None)):
            bad += not ok; n_checks += 1
            print("%-4s %s" % ("ok" if ok else "BAD", name))
    cn, cb = selftest_crawl()
    n_checks += cn; bad += cb
    print("%d checks, %d failed, 0 not judged" % (n_checks, bad)); return 1 if bad else 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    if "--fake-server" in argv:
        i = argv.index("--fake-server"); return fake_server(argv[i + 1], argv[i + 2], float(argv[i + 3]))
    if "--fake" in argv:
        return fake(argv)
    if "--inside-crawl" in argv:
        return inside_crawl(argv)
    if "--inside" in argv:
        return inside(argv)
    if argv and argv[0] == "crawl":
        return outer_crawl(argv[1:])
    return outer(argv)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
