"""The one machine-load rule of every speed and memory check (lead's decision, 2026-11-13; docs/CONSTANTS.md PERF_LOAD_MAX).

Below or at PERF_LOAD_MAX a timing is judged as always. ABOVE it the check still runs and measures:
  - a timing inside its budget is a PASS, and the line says the load it was measured at (a busier machine only makes timings worse,
    so the pass holds);
  - a timing over its budget is NOT JUDGED, with the number printed (the load may be the cause).
Memory and the app's own CPU (its CPU seconds over a window) do not depend on other processes' load: they are judged at any load and
never pass through `gate`. No budget changes here.

    from lib.loadgate import load_limit, load_now, gate, under_load, note
"""
import os

from lib.fixture import const


def load_limit():
    """PERF_LOAD_MAX from docs/CONSTANTS.md; env PERF_LOAD_MAX overrides. None when the ledger holds no number."""
    if os.environ.get("PERF_LOAD_MAX"):
        return float(os.environ["PERF_LOAD_MAX"])
    try:
        return float(const("PERF_LOAD_MAX"))
    except (KeyError, ValueError, TypeError):
        return None


def load_now():
    """The 1-minute load average, read at the call (a selftest may replace os.getloadavg)."""
    return os.getloadavg()[0]


def busy(load=None, limit="ledger"):
    """True when the machine load is above PERF_LOAD_MAX (no number in the ledger: never busy)."""
    lim = load_limit() if limit == "ledger" else limit
    ld = load_now() if load is None else load
    return lim is not None and ld > lim


def gate(over, load=None, limit="ledger"):
    """The verdict of ONE timing: `over` is the check's own comparison (True over budget, False inside it, None not measured).
    Returns True (OVER), False (PASS) or None (NOT JUDGED): over budget above PERF_LOAD_MAX is NOT JUDGED; everything else unchanged."""
    if over is None:
        return None
    if over and busy(load, limit):
        return None
    return over


def note(load=None, limit="ledger"):
    """The words a line adds when it was measured above PERF_LOAD_MAX ('' below it)."""
    lim = load_limit() if limit == "ledger" else limit
    ld = load_now() if load is None else load
    if lim is None or ld <= lim:
        return ""
    return " (measured at machine load %.1f, above PERF_LOAD_MAX %s: a pass holds, an over is NOT JUDGED)" % (ld, lim)


# The words every load-gated NOT JUDGED starts with, so the journeys' harness can tell "the app was driven, a timing was measured on a
# loaded Mac" from "no app to drive" (perf-archive round 9: main112's J1 read COULD-NOT-DRIVE for two timings over budget at load 16.4).
LOAD_MARK = "[load-gate]"


class NotJudged(Exception):
    """Raised by `under_load` for a timing over its budget above PERF_LOAD_MAX; callers pass lib.ctl's Unreadable instead."""


def under_load(value, budget, strict=True, unreadable=NotJudged):
    """For the journeys and parity cases that compare one timing themselves: returns `value` when it is judged (any value at or below
    PERF_LOAD_MAX; a value inside `budget` above it, after printing the load), else raises `unreadable` (NOT JUDGED) with the number.
    `strict`: inside means value < budget; False: value <= budget."""
    if not busy():
        return value
    inside = value < budget if strict else value <= budget
    if inside:
        print("load-gate: %.1f ms inside its budget %s%s" % (value, budget, note()))
        return value
    raise unreadable("%s machine load %.1f is above PERF_LOAD_MAX %s and the timing %.1f ms is over its budget %s: NOT JUDGED" % (LOAD_MARK, load_now(), load_limit(), value, budget))


def wrapper_cases(fn, budget, unreadable, label):
    """Two cases for a check's own timing wrapper `fn(value, budget)` (J1, J2, par-searchmac): on a loaded Mac a timing inside its
    budget comes back, one over it raises `unreadable` (NOT JUDGED)."""
    lim = load_limit()
    if lim is None:
        return [("%s: PERF_LOAD_MAX has a number in docs/CONSTANTS.md" % label, False)]
    hi = lim + 4.0
    saved = os.getloadavg
    try:
        os.getloadavg = lambda: (hi, hi, hi)
        ok_pass = fn(budget - 1, budget) == budget - 1
        try:
            fn(budget + 1, budget)
            ok_nj = False
        except unreadable:
            ok_nj = True
    finally:
        os.getloadavg = saved
    return [("%s: a timing inside its budget on a loaded Mac (load %.1f) is a PASS" % (label, hi), ok_pass),
            ("%s: a timing over its budget on a loaded Mac is NOT JUDGED" % label, ok_nj)]


def selftest_cases(budget=50.0):
    """The cases every check's selftest adds: [(name, ok)]."""
    lim = load_limit()
    if lim is None:
        return [("load-gate: PERF_LOAD_MAX has a number in docs/CONSTANTS.md", False)]
    hi, lo = lim + 4.0, max(0.0, lim - 4.0)
    out = [("load-gate: inside the budget above PERF_LOAD_MAX is a PASS", gate(False, hi) is False),
           ("load-gate: over the budget above PERF_LOAD_MAX is NOT JUDGED", gate(True, hi) is None),
           ("load-gate: over the budget at or below PERF_LOAD_MAX is OVER", gate(True, lo) is True and gate(True, lim) is True),
           ("load-gate: the line names the load it was measured at", ("machine load %.1f" % hi) in note(hi) and note(lo) == "")]
    saved = os.getloadavg
    try:
        os.getloadavg = lambda: (hi, hi, hi)
        ok_pass = under_load(budget - 1, budget) == budget - 1
        try:
            under_load(budget + 1, budget)
            ok_nj = False
        except NotJudged:
            ok_nj = True
    finally:
        os.getloadavg = saved
    out += [("load-gate: under_load keeps a timing inside its budget on a loaded Mac", ok_pass),
            ("load-gate: under_load makes a timing over its budget on a loaded Mac NOT JUDGED", ok_nj)]
    return out
