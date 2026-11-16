"""Journey harness. A journey declares steps; a step declares checks as thunks.

Thunks run against the socket. With no socket the same code runs in dry mode:
every read raises Unreadable, every check is `not judged`, and the check count
is still exact. So N is the same dry or live.
"""
import sys
import re
import os
import json
import time
import subprocess
from .ctl import Ctl, CouldNotDrive, Unreadable


class Dry:
    """What a dry run's drive body (code OUTSIDE a check's thunk) sees where it would read the app: every read answers a permissive stand-in, so the drive
    runs to its end (par-macshell, par-sidebar and par-inboxlist crashed at their first `rows(...)[0]` or `next(...)` without a socket). A thunk never gets
    one from `s.ctl` (it raises Unreadable: NOT JUDGED), and a check whose thunk touched a stand-in that the body had kept (`ctl = s.ctl` before the
    thunk) and failed is NOT JUDGED too: a stand-in is not an app. `Dry.uses` counts every touch."""
    uses = 0
    def _t(self):
        Dry.uses += 1
        return self
    def __getattr__(self, name): return self._t()
    def __call__(self, *a, **k): return self._t()
    def __getitem__(self, k): return self._t()
    def __iter__(self): Dry.uses += 1; return iter([self] * 12)
    def __len__(self): Dry.uses += 1; return 12
    def __bool__(self): Dry.uses += 1; return True
    def __eq__(self, o): Dry.uses += 1; return True
    def __ne__(self, o): Dry.uses += 1; return True      # a maybe-value: both `x == a` and `x != a` find a row in the drive body's `next(...)` searches
    __hash__ = object.__hash__
    def __lt__(self, o): Dry.uses += 1; return False
    __le__ = __gt__ = __ge__ = __lt__
    def __contains__(self, o): Dry.uses += 1; return True
    # `open(stand_in)` reads a number as a FILE DESCRIPTOR (shutil.copy(stand_in, ...) closed stdout in par-attachments): run_journey wraps open in a dry run
    def __int__(self): Dry.uses += 1; return 1
    def __index__(self): Dry.uses += 1; return 1
    def __float__(self): Dry.uses += 1; return 1.0
    def __fspath__(self): Dry.uses += 1; return "/dev/null/dry-run-no-app"
    def __str__(self): return "dry"
    def __add__(self, o): return self._t()
    __radd__ = __sub__ = __rsub__ = __mul__ = __rmul__ = __mod__ = __truediv__ = __floordiv__ = __add__
    def __neg__(self): return self._t()
    def __enter__(self): return self
    def __exit__(self, *a): return False


def state_line(ctl):
    """One line (under 400 characters) of what the app showed when a case failed or was not judged, read through `uistate` (already served): the surface,
    the focused element (`first_responder`: list, reader, search-field, composer, other:<id> when a text field or another control holds the keys), the open
    overlay or sheet, the search state, the account filter, the tab and the focused message. Never raises: a failed read says why.
    Not served as their own fields (named, not invented): a flag "a text field is editing" (read it from first_responder: search-field, composer,
    quick-reply or other:<id> of a field) and an "open sheet" apart from `overlay`."""
    try:
        if ctl is None:
            return "state: unreadable (no socket)"
        s = ctl.uistate()
        fr = s.get("first_responder")
        parts = ["surface=%s" % s.get("surface"), "focus=%s" % fr, "editing=%s" % ("yes" if str(fr) in ("search-field", "composer", "quick-reply") or str(fr).startswith("other:") else "no"),
                 "overlay=%s" % s.get("overlay"), "search=%s" % ("active" if fr == "search-field" or s.get("surface") == "mac-search" else "off"),
                 "account_filter=%s" % s.get("account_filter"), "tab=%s" % s.get("tab"), "message=%s" % s.get("focus_message"), "empty=%s" % s.get("empty_state")]
        line = "state: " + ", ".join(parts)
    except BaseException as e:      # noqa: BLE001  a state read never hides the verdict it explains
        if isinstance(e, (KeyboardInterrupt, SystemExit)):
            raise
        line = "state: unreadable (%s: %s)" % (type(e).__name__, e)
    return line if len(line) < 400 else line[:396] + "..."


class Check:
    def __init__(self, label, state, detail, expected=None, got=None, seconds=None):
        self.label, self.state, self.detail = label, state, detail  # state: pass | fail | unjudged
        self.expected, self.got, self.seconds = expected, got, seconds


class Step:
    def __init__(self, journey, step_id, title):
        self.j, self.id, self.title = journey, step_id, title
        self.checks = []

    # -- actions (never checks; a dead action shows up as a failed check after it)
    def _act(self, fn, *a):
        if self.j.ctl is None:
            return
        try:
            fn(*a)
        except Unreadable as e:
            self.j.notes.append("%s action failed: %s" % (self.id, e))

    def key(self, chord):
        self._act(lambda c: self.j.ctl.key(c), chord)

    def type(self, text):
        self._act(lambda t: self.j.ctl.type(t), text)

    def click(self, ax):
        self._act(lambda a: self.j.ctl.click(a), ax)

    def raw(self, verb, *args):
        """Planned verbs outside the core list (fixture, clock, net contract rows); marked an assumption."""
        self._act(lambda: self.j.ctl.send(verb, *args))

    def shot(self, name):
        self._act(lambda n: self.j.ctl.shot(n), name)

    # -- reads, for use inside thunks
    @property
    def ctl(self):
        if self.j.ctl is None:
            if getattr(self.j, "in_thunk", 0) == 0:     # the drive body, not a check: it must run on to its end (see Dry)
                self.j.dry_handed = True
                return Dry()
            self.j.unread_count = getattr(self.j, "unread_count", 0) + 1     # a helper may swallow this Unreadable and answer False: _record reads the count
            raise Unreadable(self.j.dry_reason or "dry run: no socket")
        return self.j.ctl

    # -- checks
    def _record(self, name, fn):
        label = "%s/%s" % (self.id, name)
        self.j.in_thunk = getattr(self.j, "in_thunk", 0) + 1
        uses, unread = Dry.uses, getattr(self.j, "unread_count", 0)
        t0 = time.monotonic()
        self._eg = (None, None)
        try:
            ok, detail = fn()
            if not ok and self.j.ctl is None and (getattr(self.j, "dry_handed", False) or Dry.uses != uses or getattr(self.j, "unread_count", 0) != unread):
                # no socket and the drive body ran on stand-ins: a value a check compares may come from one (a variable the body kept), so a miss proves nothing
                self.checks.append(Check(label, "unjudged", "dry run: the check read a stand-in, not the app"))
            else:
                self.checks.append(Check(label, "pass" if ok else "fail", detail if ok else "%s | %s" % (detail, state_line(self.j.ctl))))
        except Unreadable as e:
            self.checks.append(Check(label, "unjudged", "%s | %s" % (e, state_line(self.j.ctl))))
        except (KeyError, IndexError, TypeError) as e:
            # a reply missing the field the check needs is a failed check, not a crash
            if self.j.ctl is None:       # no socket, so there is no reply whose shape could be wrong
                self.checks.append(Check(label, "unjudged", "dry run: no reply to read (%s)" % type(e).__name__))
            else:
                self.checks.append(Check(label, "fail", "reply shape: %s %s | %s" % (type(e).__name__, e, state_line(self.j.ctl))))
        finally:
            self.j.in_thunk -= 1
            if self.checks and self.checks[-1].label == label:     # the verdict just recorded carries what it compared and how long it took
                c = self.checks[-1]
                c.expected, c.got, c.seconds = self._eg[0], self._eg[1], round(time.monotonic() - t0, 3)

    def expect(self, name, thunk, expected):
        def run():
            got = thunk()
            self._eg = (expected, got)
            return got == expected, "expected %r, got %r" % (expected, got)
        self._record(name, run)

    def expect_le(self, name, thunk, limit):
        def run():
            got = thunk()
            return got <= limit, "expected <= %r, got %r" % (limit, got)
        self._record(name, run)

    def expect_in(self, name, thunk, member):
        def run():
            got = thunk()
            return member in got, "expected %r in %r" % (member, got)
        self._record(name, run)

    def expect_not_in(self, name, thunk, member):
        def run():
            got = thunk()
            return member not in got, "expected %r not in %r" % (member, got)
        self._record(name, run)

    def arrive(self, via, surface):
        """Arrive by the named control (("key", chord) or ("click", ax-id)), then check the surface."""
        kind, what = via
        (self.key if kind == "key" else self.click)(what)
        self.expect("arrives-by-control", lambda: self.ctl.uistate().get("surface"), surface)


class Journey:
    def __init__(self, jid, ctl):
        self.id, self.ctl, self.steps, self.notes = jid, ctl, [], []
        self.dry_reason = ""      # why the journey runs undriven although a socket answered (a journey for another platform)

    def step(self, step_id, title):
        s = Step(self, step_id, title)
        self.steps.append(s)
        return s

    def checks(self):
        return [c for s in self.steps for c in s.checks]


def declared_mutants(path):
    """Labels named in the file's `# MUTANTS` comment block: lines `# J1.2/label | mutation`."""
    out = []
    for line in open(path, encoding="utf-8"):
        # J-steps, parity checks (par-<lane>/<name>) and a check's own step prefix (keycaps-<screen>, brandmark-<case>, suggestion-<case>):
        # a step id with a letter after its number (J1.6b, J1.6c) made every such row invisible the same way (gap-mailview-checks; messagebody-check redid the count itself),
        # a prefix the regex did not know made every declared row invisible, so the check exited 1 on NO MUTANT with 0 failed (keycaps, main53 to main55)
        m = re.match(r"#\s+((?:J\d+\.\d+[a-z]?|[a-z][\w-]*)/[\w-]+)\s*\|\s*\S", line)
        if m:
            out.append(m.group(1))
    return out


# The platform a running app is, from the surface its uistate names: every surface id starts with its platform (docs/CONTROL-SOCKET.md,
# `<surface>.<region>...`: mac-main, mac-reader, iphone-inbox ...). A surface that names neither is not a platform verdict.
PLATFORM_PREFIX = {"mac": "mac-", "iphone": "iphone-"}


def platform_of(ctl):
    """'mac' | 'iphone' | None (unreadable or an unknown surface: the journey is driven as before, never skipped on a guess)."""
    try:
        surface = ctl.uistate().get("surface") or ""
    except (Unreadable, AttributeError):
        return None
    for name, prefix in PLATFORM_PREFIX.items():
        if surface.startswith(prefix):
            return name
    return None


def platform_mismatch(mod, ctl):
    """A journey that declares `PLATFORM` ("iphone") and meets an app of another platform cannot be driven there: the words say why,
    and every check is not judged (COULD-NOT-DRIVE, never RED). "" when the journey declares none, or the platforms agree, or unknown."""
    want = getattr(mod, "PLATFORM", None)
    if not want or ctl is None:
        return ""
    got = platform_of(ctl)
    if got is None or got == want:
        return ""
    return "%s journey on a %s build (the socket's app is the %s app)" % ({"iphone": "iPhone"}.get(want, want),
                                                                         {"mac": "Mac"}.get(got, got), {"mac": "Mac"}.get(got, got))


def _jsonable(v):
    try:
        json.dumps(v)
        return v
    except (TypeError, ValueError):
        return repr(v)


def results_path(check, build, folder=None):
    folder = folder or os.environ.get("WHOOSH_RESULTS_DIR") or os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "_captures", "results", check)
    return os.path.join(folder, "%s-%s.results.jsonl" % (check, build or "unlabelled"))


def write_results(check, journey, folder=None):
    """One JSON line per case of the run, written whole to a temp file and renamed (no torn file). Never raises: a failed write prints one line.
    build: the app serves no build label (no uistate or verb field: missing row), so it is $WHOOSH_BUILD_LABEL or null.
    The rig's captures folder is the default when WHOOSH_RESULTS_DIR is unset."""
    try:
        build = os.environ.get("WHOOSH_BUILD_LABEL") or None
        try:
            commit = subprocess.run(["git", "-C", os.path.dirname(os.path.abspath(__file__)), "rev-parse", "--short", "HEAD"], capture_output=True, text=True, timeout=10).stdout.strip() or None
        except Exception:      # noqa: BLE001
            commit = None
        verdict = {"pass": "PASS", "fail": "FAIL", "unjudged": "NOT JUDGED"}
        lines = []
        for s in journey.steps:
            for c in s.checks:
                lines.append(json.dumps({"check": check, "step": s.id, "case": c.label.split("/", 1)[-1], "verdict": verdict.get(c.state, c.state),
                                         "expected": _jsonable(c.expected), "got": _jsonable(c.got), "detail": c.detail if c.state != "pass" else "",
                                         "seconds": c.seconds, "build": build, "commit": commit}, ensure_ascii=False))
        path = results_path(check, build, folder)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        tmp = "%s.tmp-%d" % (path, os.getpid())
        with open(tmp, "w", encoding="utf-8") as f:
            f.write("\n".join(lines) + ("\n" if lines else ""))
        os.replace(tmp, path)
        return path
    except BaseException as e:      # noqa: BLE001
        if isinstance(e, (KeyboardInterrupt, SystemExit)):
            raise
        sys.stderr.write("results file not written: %s: %s\n" % (type(e).__name__, e))
        return None


CASE_WORD = {"pass": "pass", "fail": "fail", "unjudged": "unjudged"}


def case_line(label, state, detail, limit=300):
    """ONE printed line for one judged case, in the shape par-labelscontactsmac.py and par-palettemac.py print: the case name, the verdict word
    (pass | fail | unjudged) and the values that decided it (the check's own detail, one line, cut at `limit` characters). A case that passes is
    printed like one that fails, so a pass can be shown from the log."""
    text = " ".join(str(detail).split())
    if len(text) > limit:
        text = text[:limit - 3] + "..."
    return ("%-28s %-8s %s" % (label, CASE_WORD.get(state, state), text)).rstrip()


def print_cases(r, out=None):
    """Print the line of every case of a run_journey result, in the order the journey judged them (stdout, before the totals line)."""
    out = out or sys.stdout
    for label, state, detail in r.get("cases", []):
        out.write(case_line(label, state, detail) + "\n")


def print_model_cases(cases, out=None):
    """The same one line per case for a SELF-TEST's clean run on a model (not an app): `cases` is {label: (state, detail)} or a list of Check. Each line starts
    with "model " so it is never read as the result of a build."""
    out = out or sys.stdout
    items = [(c.label, c.state, c.detail) for c in cases] if isinstance(cases, (list, tuple)) else [(k, v[0], v[1]) for k, v in cases.items()]
    for label, state, detail in items:
        out.write("model " + case_line(label, state, detail) + "\n")


TIMING_NOT_JUDGED = "TIMING-NOT-JUDGED"


def journey_verdict(cs, why=""):
    """(verdict, evidence) of one journey's checks. RED: a check failed. TIMING-NOT-JUDGED (perf-archive round 9): the app WAS driven, every
    check ran, and the only checks not judged are timings over their budget measured above PERF_LOAD_MAX (journeys/lib/loadgate.py marks
    them): never COULD-NOT-DRIVE, which says no app was there to drive, and never GREEN (a not-judged timing is not a pass).
    COULD-NOT-DRIVE: no app, a read failed, or no checks. GREEN: every check passed."""
    from lib.loadgate import LOAD_MARK
    n = len(cs)
    f = [c for c in cs if c.state == "fail"]
    u = [c for c in cs if c.state == "unjudged"]
    if f:
        return "RED", "first failing check %s: %s" % (f[0].label, f[0].detail)
    if u and not why and all(str(c.detail).startswith(LOAD_MARK) for c in u):
        return TIMING_NOT_JUDGED, "the app was driven: %d of %d checks passed; %d timing(s) over budget on a loaded Mac not judged, first %s: %s" % (
            n - len(u), n, len(u), u[0].label, u[0].detail)
    if u or n == 0:
        evidence = why or (u[0].detail if u else "no checks")
        return "COULD-NOT-DRIVE", "%s; %d of %d checks not judged" % (evidence, len(u), n)
    return "GREEN", "all %d checks passed" % n


def verdict_cases():
    """Self-test cases of `journey_verdict` (J1 and J2 run them): [(name, ok)]."""
    from lib.loadgate import LOAD_MARK

    class C:
        def __init__(self, label, state, detail=""):
            self.label, self.state, self.detail = label, state, detail
    timing = C("J1.1/launch-budget-ms", "unjudged", LOAD_MARK + " machine load 16.4 is above PERF_LOAD_MAX 12.0 and the timing 1923.7 ms is over its budget 1500: NOT JUDGED")
    ok = [C("J1.1/row-count", "pass")] * 3
    return [("a timing not judged under load: the journey is TIMING-NOT-JUDGED, never COULD-NOT-DRIVE", journey_verdict(ok + [timing])[0] == TIMING_NOT_JUDGED),
            ("a timing not judged under load is never GREEN", journey_verdict(ok + [timing])[0] != "GREEN"),
            ("a broken pipe beside it is still COULD-NOT-DRIVE", journey_verdict(ok + [timing, C("J2.1/x", "unjudged", "uistate (): [Errno 32] Broken pipe")])[0] == "COULD-NOT-DRIVE"),
            ("no socket (a dry run) is COULD-NOT-DRIVE", journey_verdict(ok + [timing], "dry run")[0] == "COULD-NOT-DRIVE"),
            ("a failed check beside it is RED", journey_verdict(ok + [timing, C("J1.2/x", "fail", "expected 1, got 2")])[0] == "RED"),
            ("every check passed is GREEN", journey_verdict(ok)[0] == "GREEN")]


def run_journey(mod, path, force_dry=False):
    """Returns a result dict for one journey module."""
    ctl, why = None, ""
    if not force_dry:
        try:
            ctl = Ctl().connect()
        except CouldNotDrive as e:
            why = str(e)
    else:
        why = "dry run"
    mismatch = platform_mismatch(mod, ctl)
    if mismatch:                                 # same code as a dry run, so the check count is exact; the reason is the one evidence
        ctl.close()
        ctl, why = None, mismatch
    j = Journey(mod.ID, ctl)
    j.dry_reason = mismatch
    import time as _time
    _sleep = _time.sleep
    import builtins as _b
    _open = _b.open
    if ctl is None:
        _time.sleep = lambda *_a, **_k: None     # a dry drive waits for nothing (its settle loops would sleep their whole budget on a stand-in)
        def _safe_open(f, *a, **k):
            if isinstance(f, Dry):
                raise FileNotFoundError("dry run: no app, so no file at a stand-in's path")
            return _open(f, *a, **k)
        _b.open = _safe_open
    try:
        mod.drive(j)
    finally:
        _time.sleep = _sleep
        _b.open = _open
        if ctl:
            ctl.close()
    write_results(os.path.splitext(os.path.basename(path))[0], j)
    cs = j.checks()
    n = len(cs)
    f = [c for c in cs if c.state == "fail"]
    u = [c for c in cs if c.state == "unjudged"]
    labels = {c.label for c in cs}
    declared = set(declared_mutants(path))
    no_mutant = sorted(labels - declared)
    stray = sorted(declared - labels)
    for c in u:                                  # a red never hides WHY a check was not judged
        sys.stderr.write("NOT JUDGED %s: %s\n" % (c.label, c.detail))
    verdict, evidence = journey_verdict(cs, why)
    return {"id": mod.ID, "title": getattr(mod, "TITLE", ""), "target": getattr(mod, "TARGET", "LOCAL"),
            "visible_outcome": getattr(mod, "VISIBLE_OUTCOME", ""), "steps": len(j.steps),
            "checks": n, "failed": len(f), "unjudged": len(u), "verdict": verdict, "evidence": evidence,
            "no_mutant": no_mutant, "stray_mutant": stray, "notes": j.notes,
            "cases": [(c.label, c.state, c.detail) for c in cs],
            "fail_list": ["%s: %s" % (c.label, c.detail) for c in f],
            "unjudged_list": ["%s: %s" % (c.label, c.detail) for c in u]}
