#!/usr/bin/env python3
"""thirty-one: success criterion 5, adding the 31st account changes nothing the owner can feel (fn-perf-thirty-one).

    sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 perf/thirty-one.py'
    python3 perf/thirty-one.py --selftest        every cell against a model, then against each MUTANT: each must go red

With the 30 fixture accounts fully synced, the 31st is added while an interaction run (J/K) is going. Budgets are read from
docs/CONSTANTS.md and never typed here: FRAME_MS (every frame), SEARCH_FIRST_SCREEN_MS (search), MEM_MAC_MB (memory with
30 accounts, held at 31). Prints the build stamp and the machine load first. Exit 1 when a cell is over budget or not judged.
Prints one `PASS <cell>` line per passing cell, `FAIL <cell>` or `NOT JUDGED <cell>` (stderr) per other cell, then
`N checks, F failed, U not judged`. A reading the app does not serve is `not judged`, never a pass.

Read verbs the app serves (Sources/ControlSocket/WindowVerbs.swift measure handler, Sources/App/InteractionProbe.swift):
  `measure mac-main.interaction.gap` -> {"ms"}  after each J/K key: the longest gap between two presented frames (the dropped-frame
                                                measure); the worst over the run is judged under 1.5 x FRAME_MS (lead's ruling, 2026-11-09,
                                                docs/CONSTANTS.md FRAME_MS, as perf/interaction.py does)
  `measure mac-search.first-screen` -> {"ms"}   after the Enter that launches the search: key to the first presented frame (what J2.1 judges)
  `measure app.memory` -> {"mb"}                the app's physical footprint now (task_vm_info.phys_footprint), also {"rss_mb"}
Action verb: `fixture add-account <address> [name] [colour]` (Sources/ControlSocket/FixtureEvents.swift:31, one to three arguments).
"""
import importlib.util
import json
import os
import plistlib
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
_sp = importlib.util.spec_from_file_location("kit", os.path.join(ROOT, "checks", "welcomemac-check.py"))
K = importlib.util.module_from_spec(_sp)
_sp.loader.exec_module(K)
const, FX, EXTRA, ACCOUNT_COUNT, Unreadable = K.const, K.FX, K.EXTRA, K.ACCOUNT_COUNT, K.Unreadable
T31 = EXTRA["thirty_first"]
FRAME, SEARCH, MEM = const("FRAME_MS"), const("SEARCH_FIRST_SCREEN_MS"), const("MEM_MAC_MB")
MODEL_MEM = const("RSS_MODEL_MAX_MB")   # round 10 (lead's ruling): the memory with the meaning model in it, read after the search cell
KEYS = 40
FRAME_GAP = FRAME * 1.5   # a perfect 60 Hz run gaps 16.7 ms, a dropped frame 33 ms: the gap is judged under 1.5 x FRAME_MS
CELLS = []   # (name, mutant, words, fn)


def cell(name, mutant, words):
    def deco(fn):
        CELLS.append((name, mutant, words, fn))
        return fn
    return deco


def prepare(r):
    r.act("fixture", "reset")
    r.act("clock", "advance", 600)                      # the 30 accounts are fully synced
    r.act("click", "mac-main.status")
    r.act("click", "mac-main.status")


def reading(r, what, field):
    """One served measure as a number; a reading that is not a positive finite number is not a reading of a running app."""
    v = float(r.send("measure", what)[field])
    if not (v > 0 and v == v and v != float("inf")):
        raise Unreadable("%s read %s, which is not a reading of a running app" % (what, v))
    return v


def interaction(r, add):
    """J/K keys with the 31st account added in the middle; returns the worst presented-frame gap over the run (one read per key)."""
    worst = 0.0
    for i in range(KEYS):
        if i == KEYS // 2 and add:                      # the 31st arrives in the middle of the run
            r.act("fixture", "add-account", T31["address"], T31["name"], T31["colour"])
        r.act("key", "j" if i % 2 == 0 else "k")
        worst = max(worst, reading(r, "mac-main.interaction.gap", "ms"))
    return worst


@cell("thirty-first-is-there", "t31-missing", "the 31st account is never added")
def _(r):
    prepare(r); interaction(r, True)
    return [x["name"] for x in r.rows("mac-settings-accounts.accounts")][-1], T31["name"]   # control: the cells below ran with 31


@cell("every-frame-under-budget", "frame-slow", "a frame takes longer than the frame budget")
def _(r):
    prepare(r)
    gap = interaction(r, True)
    return gap <= FRAME_GAP, True, "worst presented-frame gap %.1f ms over %d keys, budget %.1f ms (1.5 x FRAME_MS %s)" % (gap, KEYS, FRAME_GAP, FRAME)


@cell("memory-under-budget", "mem-high", "memory goes over the memory budget")
def _(r):
    """The app with 31 accounts and NO search model (lead's ruling, round 10): read BEFORE the search cell, judged against MEM_MAC_MB. A
    meaning model in memory here is a finding (something loaded it without a search), never a pass."""
    prepare(r); interaction(r, True)
    return memory_cell(r, MEM, "MEM_MAC_MB", "with %d accounts, before any search" % (ACCOUNT_COUNT + 1), model_is_a_finding=True)


@cell("search-under-budget", "search-slow", "search takes longer than the search budget")
def _(r):
    prepare(r); interaction(r, True)
    r.act("key", "/")
    r.act("type", "supplier")
    r.act("key", "Enter")                               # the search is launched: its first screen is the next presented frame
    ms = reading(r, "mac-search.first-screen", "ms")
    return ms <= SEARCH, True, "first results screen %.1f ms, budget %s ms (SEARCH_FIRST_SCREEN_MS)" % (ms, SEARCH)


@cell("memory-with-search-model-under-budget", "model-mem-high", "memory with the meaning model goes over the model budget")
def _(r):
    """The same copy AFTER the search cell (the meaning model may be in memory; EMBED_IDLE_UNLOAD_S 300 s has not passed), judged against
    RSS_MODEL_MAX_MB, the ledger's row for the model loaded for a meaning search (lead's ruling, round 10)."""
    prepare(r); interaction(r, True)
    return memory_cell(r, MODEL_MEM, "RSS_MODEL_MAX_MB", "with %d accounts, after the search cell" % (ACCOUNT_COUNT + 1))


def memory_cell(r, budget, row, when, model_is_a_finding=False):
    """(within budget, True, the line): the physical footprint (`measure app.memory`), its budget and ledger row, and the model's state."""
    mb = reading(r, "app.memory", "mb")
    embedder, words = model_state(r)
    line = "physical footprint %.0f MB %s, budget %s MB (%s)%s" % (mb, when, budget, row, words)
    if model_is_a_finding and embedder == "handed":
        return False, True, "FINDING: the meaning model was in memory before any search; " + line
    return mb <= budget, True, line


def model_state(r):
    """What the copy says about the meaning model when the memory is read (`rows mac-main.background`: embedder handed|held), and its
    resident size beside the footprint: the reading names whether the 1.49 GB model was in the process. Information, never a verdict."""
    words, embedder = [], None
    try:
        row = (r.rows("mac-main.background") or [{}])[0]
        embedder = row.get("embedder")
        words.append("meaning model %s" % (embedder or "not said"))
    except Exception as e:      # a copy that does not serve the row: say so, the verdict stands on the number
        words.append("meaning model state unreadable (%s)" % type(e).__name__)
    try:
        rss = r.send("measure", "app.memory").get("rss_mb")
        if rss is not None:
            words.append("resident %.0f MB" % float(rss))
    except Exception as e:
        words.append("resident size unreadable (%s)" % type(e).__name__)
    return embedder, "; " + ", ".join(words)


@cell("budget-can-fail", "control", "control: the 30-account run is measured with the same probe")
def _(r):
    prepare(r)                                          # positive control: the probe returns a number without the 31st
    return isinstance(interaction(r, False), float), True


def stamp():
    d = os.environ.get("WHOOSH_SOCKET_DIR") or os.environ.get("D") or ""
    p = os.path.join(d, "rig", "Whoosh.app", "Contents", "Info.plist")
    try:
        info = plistlib.load(open(p, "rb"))
        return " ".join("%s=%s" % (k.replace("NexusBuild", "").lower(), info.get(k)) for k in
                        ("NexusBuildLabel", "NexusBuildCommit", "NexusBuiltAt", "NexusBuildDirtyFiles"))
    except (OSError, ValueError) as e:
        return "unreadable (%s)" % type(e).__name__


def load_line():
    try:
        la = os.getloadavg()
        cpus = os.cpu_count() or 1
        try:
            gate = "PERF_LOAD_MAX=%s" % const("PERF_LOAD_MAX")
        except KeyError:
            gate = "PERF_LOAD_MAX has no number in docs/CONSTANTS.md: the load gate is not applied"
        return "load %.2f %.2f %.2f on %d cores; %s" % (la[0], la[1], la[2], cpus, gate)
    except OSError:
        return "load unreadable"


def run(r):
    out = []
    for name, _m, _w, fn in CELLS:
        try:
            res = fn(r)
            got, want = res[0], res[1]
            # round 9: a cell that measured a number says it, with its budget and ledger row, pass or fail
            detail = res[2] if len(res) > 2 else "expected %r, got %r" % (want, got)
            out.append((name, "pass" if got == want else "fail", detail))
        except Unreadable as e:
            out.append((name, "unjudged", str(e)))
        except (KeyError, IndexError, TypeError) as e:
            out.append((name, "fail", "reply shape: %s %s" % (type(e).__name__, e)))
    return out


class Model:
    def __init__(self, mut):
        self.m = mut
        self.reset()

    def reset(self):
        self.n, self.frame = ACCOUNT_COUNT, 5
        if not hasattr(self, "model"):
            self.model = "model-at-launch" in self.m          # a `fixture reset` does not unload the meaning model, as in the app

    def handle(self, verb, a):
        if verb == "fixture":
            if a[0] == "reset": self.reset()
            if a[0] == "add-account" and "t31-missing" not in self.m: self.n += 1
            if a[0] == "add-account" and "frame-slow" in self.m: self.frame = FRAME_GAP + 1
            return {"ok": True}
        if verb == "key" and a[0] == "Enter": self.model = True   # a launched search loads the meaning model
        if verb in ("clock", "click", "key", "type"): return {"ok": True}   # a key resets nothing: the model keeps the worst gap the 31st caused
        if verb == "measure":
            if a[0] == "mac-main.interaction.gap":
                return {"ok": True, "ms": None if "ctl-null" in self.m else 0.0 if "gap-zero" in self.m else self.frame}
            if a[0] == "mac-search.first-screen": return {"ok": True, "ms": SEARCH + 40 if "search-slow" in self.m else 30}
            if a[0] == "app.memory":
                if "mem-zero" in self.m: return {"ok": True, "mb": 0.0, "rss_mb": 0.0}
                if self.model:
                    return {"ok": True, "mb": const("RSS_MODEL_MAX_MB") + 50 if "model-mem-high" in self.m else 300 + self.n + 1500, "rss_mb": 1780.0}
                return {"ok": True, "mb": const("MEM_MAC_MB") + 50 if "mem-high" in self.m else 300 + self.n, "rss_mb": 280.0}   # ledger numbers, so a swapped budget cannot move the fake
        if verb == "rows" and a[0] == "mac-main.background":
            return {"ok": True, "rows": [{"embedder": "handed" if self.model else "held"}]}
        if verb == "rows" and a[0] == "mac-settings-accounts.accounts":
            return {"ok": True, "rows": [{"name": x["name"]} for x in FX["accounts"]] + ([{"name": T31["name"]}] if self.n > ACCOUNT_COUNT else [])}
        return {"ok": False, "error": "no %s %s" % (verb, a)}


# round 8 (lead 2026-11-13, journeys/lib/loadgate.py): the timing cells pass inside their budget at any load and are NOT JUDGED over it above
# PERF_LOAD_MAX; the memory cell, the 31st account and the control do not depend on other processes' load and are judged at any load.
TIMING_CELLS = ("every-frame-under-budget", "search-under-budget")


def apply_load(res, load):
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib import loadgate    # noqa: E402
    out = []
    for n, st, d in res:
        if n in TIMING_CELLS and st in ("pass", "fail"):
            v = loadgate.gate(st == "fail", load)
            st = {True: "fail", False: "pass", None: "unjudged"}[v]
            d = d + loadgate.note(load)
        out.append((n, st, d))
    return out


NOT_JUDGED_MUTANTS = [("every-frame-under-budget", "gap-zero", "unjudged"), ("memory-under-budget", "mem-zero", "unjudged"),
                      ("memory-with-search-model-under-budget", "mem-zero", "unjudged")]


def order_ok(cells):
    """Round 10: the 600 MB cell runs before the search cell, the model cell after it."""
    names = [c[0] for c in cells]
    try:
        return names.index("memory-under-budget") < names.index("search-under-budget") < names.index("memory-with-search-model-under-budget")
    except ValueError:
        return False


def selftest():
    global CELLS, MEM, MODEL_MEM          # round 10: the broken copies swap the order and the budgets, then put them back
    bad = 0
    base = K.with_fake(Model(set()), run)
    print("base model: %d cells, %d not passing" % (len(base), sum(1 for x in base if x[1] != "pass")))
    bad += any(x[1] != "pass" for x in base)
    print("| cell | mutant | mutation | seen red |\n|---|---|---|---|")
    for name, mut, words, _ in CELLS:
        mut = "ctl-null" if mut == "control" else mut
        res = K.with_fake(Model({mut}), run)
        ok = any(n == name and st == "fail" for n, st, _d in res)
        bad += not ok
        print("| %s | %s | %s | %s |" % (name, mut, words, "red on " + name if ok else "NOT RED"))
    for name, mut, want in NOT_JUDGED_MUTANTS:          # a reading of 0 is not a pass: the cell must say not judged
        res = K.with_fake(Model({mut}), run)
        ok = any(n == name and st == want for n, st, _d in res)
        bad += not ok
        print("| %s | %s | a reading of 0 | %s |" % (name, mut, "%s on %s" % (want, name) if ok else "NOT %s" % want.upper()))
    high = K.with_fake(Model({"mem-high"}), run)
    mline = next((d for n, st, d in high if n == "memory-under-budget"), "")
    sline = next((d for n, st, d in K.with_fake(Model({"search-slow"}), run) if n == "search-under-budget"), "")
    for name, ok in (("a failing memory cell prints its number, its budget and its ledger row", ("%.0f MB" % (MEM + 50)) in mline and "MEM_MAC_MB" in mline and ("budget %s MB" % MEM) in mline),
                     ("the memory line says whether the meaning model was in the process", "meaning model held" in mline and "resident 280 MB" in mline),
                     ("a failing search cell prints its time and SEARCH_FIRST_SCREEN_MS", ("%.1f ms" % (SEARCH + 40)) in sline and "SEARCH_FIRST_SCREEN_MS" in sline),
                     ("a passing cell prints its number too", any(n == "memory-under-budget" and st == "pass" and "MB" in d for n, st, d in base))):
        bad += not ok
        print("%-4s %s" % ("ok" if ok else "BAD", name))
    # round 10: the order and each budget, each with its broken copy
    first = lambda res, name: next((st, d) for n, st, d in res if n == name)
    swapped = [c for c in CELLS if c[0] != "memory-under-budget"]
    swapped.insert([c[0] for c in swapped].index("search-under-budget") + 1, next(c for c in CELLS if c[0] == "memory-under-budget"))
    saved = CELLS
    try:
        CELLS = swapped
        late = first(K.with_fake(Model(set()), run), "memory-under-budget")
    finally:
        CELLS = saved
    cases10 = [("order: the 600 MB cell runs before the search cell, the model cell after it", order_ok(CELLS)),
               ("order, broken copy: the 600 MB cell moved after the search is caught by the order check", not order_ok(swapped)),
               ("order, broken copy: read after the search the 600 MB cell sees the model and fails as a FINDING", late[0] == "fail" and "FINDING" in late[1]),
               ("the 600 MB line names MEM_MAC_MB and the model held", "MEM_MAC_MB" in first(base, "memory-under-budget")[1] and "meaning model held" in first(base, "memory-under-budget")[1]),
               ("the model line names RSS_MODEL_MAX_MB and the model handed", "RSS_MODEL_MAX_MB" in first(base, "memory-with-search-model-under-budget")[1] and "meaning model handed" in first(base, "memory-with-search-model-under-budget")[1]),
               ("a model in memory before any search is a FINDING, not a pass", first(K.with_fake(Model({"model-at-launch"}), run), "memory-under-budget")[0] == "fail")]
    try:
        MEM, MODEL_MEM = MODEL_MEM, MEM                  # broken copy: each memory cell judged against the other's row
        bent = K.with_fake(Model(set()), run)
        bent_high = K.with_fake(Model({"mem-high"}), run)
    finally:
        MEM, MODEL_MEM = const("MEM_MAC_MB"), const("RSS_MODEL_MAX_MB")
    cases10 += [("budget, broken copy: the model cell judged against MEM_MAC_MB goes red on a normal model footprint", first(bent, "memory-with-search-model-under-budget")[0] == "fail"),
                ("budget, broken copy: the 600 MB cell judged against RSS_MODEL_MAX_MB lets a 650 MB app pass (the real budget catches it)",
                 first(bent_high, "memory-under-budget")[0] == "pass" and first(K.with_fake(Model({"mem-high"}), run), "memory-under-budget")[0] == "fail")]
    for name, ok in cases10:
        bad += not ok
        print("%-4s %s" % ("ok" if ok else "BAD", name))
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib import loadgate    # noqa: E402
    hi = (loadgate.load_limit() or 12.0) + 4
    slow = K.with_fake(Model({"frame-slow"}), run)
    for name, ok in (("load: every cell inside its budget on a loaded Mac still passes", all(st == "pass" for _n, st, _d in apply_load(base, hi))),
                     ("load: a slow frame on a loaded Mac is NOT JUDGED (with its number)", any(n == "every-frame-under-budget" and st == "unjudged" for n, st, _d in apply_load(slow, hi))),
                     ("load: memory over budget on a loaded Mac stays FAIL", any(n == "memory-under-budget" and st == "fail" for n, st, _d in apply_load(K.with_fake(Model({"mem-high"}), run), hi))),
                     ("load: a slow frame at PERF_LOAD_MAX fails", any(n == "every-frame-under-budget" and st == "fail" for n, st, _d in apply_load(slow, hi - 4)))) + tuple(loadgate.selftest_cases()):
        bad += not ok
        print("%-4s %s" % ("ok" if ok else "BAD", name))
    print("selftest: %s" % ("every cell seen red on its mutant" if not bad else "%d problem(s)" % bad))
    return 1 if bad else 0


def main():
    if "--selftest" in sys.argv:
        return selftest()
    print("build stamp: %s" % stamp())
    print("machine: %s" % load_line())
    try:
        r = K.Rig(os.environ.get("WHOOSH_SOCKET_DIR") or os.environ.get("D") or "").connect()
    except (Unreadable, OSError) as e:
        print("could not drive: %s" % e, file=sys.stderr)
        res = [(n, "unjudged", str(e)) for n, *_ in CELLS]
    else:
        res = apply_load(run(r), os.getloadavg()[0])
    for n, st, d in res:
        if st == "pass":
            print("PASS %s: %s" % (n, d))
        else:
            print("  %s %s: %s" % ({"fail": "FAIL", "unjudged": "NOT JUDGED"}[st], n, d), file=sys.stderr)
    f = sum(1 for x in res if x[1] == "fail"); u = sum(1 for x in res if x[1] == "unjudged")
    print("%d checks, %d failed, %d not judged" % (len(res), f, u))
    return 1 if f or u else 0


if __name__ == "__main__":
    sys.exit(main())
