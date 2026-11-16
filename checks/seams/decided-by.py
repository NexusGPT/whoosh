#!/usr/bin/env python3
"""Seam con-seams-pick/decided-by: what a row says on hover about its placement agrees with the tab it sits in
and with the store (C-J1.13 "every placement writes decided_by"; C-J5.8 "decided_by on hover").
Since contract deltas 6 to 12, decided_by names WHO placed the row (Laya, Jev, Owner, Rule: <id>, Default) and
the hover line names the tab ("Placed in <tab> by ..."). Per fixture message:
  why-names-tab   the row's hover line (mac-main.list field why) names the fixture tab's served title
  decider@row     the row's decided_by equals the fixture's decided_by
  tab@core        core message/<id>/category equals the fixture tab
  decider@core    core message/<id>/label/<category>/decided_by equals the fixture decider (lower case,
                  'rule:<id>'); a default row must hold nothing or 'default', judged only when some decided
                  row's entry holds a value (positive control).
  model-name      SPEC A133: a row an on-device model other than Laya placed names that model by its catalogue name
                  (Sources/Features/OnDeviceAI/Core/DecisionModelCatalogue.swift, read here): the row's decided_by is the name,
                  the hover line holds the name, and the core serves 'model:<catalogue id>'. A fixture row names such a model by
                  its catalogue name. The sample fixture holds no such row (docs/design/fixtures is not this check's to change),
                  so the live drive judges it only on a fixture that has one; --selftest adds one made-up row, 'probe-model',
                  so the judge's model case and its broken copies are exercised."""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from seamlib import *  # noqa: E402,F401,F403
from seamlib import NJ, nj, pick, equal, fixture, constant_row, constant_int, contract_row, contains_words, norm_words, run  # noqa: E402


def first(rows, field):
    if nj(rows):
        return rows
    return rows[0].get(field, NJ("no field %s" % field)) if rows else NJ("region empty")


def setk(path, val):
    def f(o):
        d = o
        for k in path[:-1]:
            d = d[k]
        d[path[-1]] = val
        return o
    return f

CATALOGUE = "Sources/Features/OnDeviceAI/Core/DecisionModelCatalogue.swift"

def models():
    """{catalogue name: id} of every on-device model other than Laya, read from the catalogue's `id: "...", name: "..."` lines."""
    import re
    with open(os.path.join(ROOT, CATALOGUE), encoding="utf-8") as f:
        src = f.read()
    return {name: mid for mid, name in re.findall(r'id: "([^"]+)",\s*name: "([^"]+)"', src) if mid != "laya"}

def expected():
    fx = fixture()
    rows = [(m["id"], m["tab"], m.get("decided_by")) for m in fx["messages"]]
    names = models()
    if "--selftest" in sys.argv[1:] and names and not any(d in names for _, _, d in rows):
        rows.append(("probe-model", "primary", sorted(names)[0]))   # made up, selftest only: see the docstring
    return {"rows": rows, "models": names}

def _core_words(d, names=None):
    if d is None:
        return None
    if names and d in names:
        return "model:" + names[d]
    return "rule:" + d.split(":", 1)[1].strip() if d.lower().startswith("rule:") else d.lower()

def observe(c, exp):
    rows, tabs = c.rows("list"), c.rows("tabs")
    o = {}
    for mid, tab, _ in exp["rows"]:
        title = pick(tabs, "id", tab, "title")
        cat = c.core("message/%s/category" % mid)
        dec = c.core("message/%s/label/%s/decided_by" % (mid, cat if not nj(cat) else tab))
        if nj(dec) and "no core value" in dec.why:
            dec = None  # the store answered: nothing there (a real absence, judged below)
        o[mid] = {"why": pick(rows, "id", mid, "why"), "tab_title": title, "row_decider": pick(rows, "id", mid, "decided_by"),
                  "core_tab": cat, "core_decider": dec}
    return o

def judge(o, exp):
    out = []
    # positive control for the default rows: their empty core entry means "default" only when the same path
    # holds a value for at least one row a decider placed; otherwise the path itself may be dead.
    held = any(dec is not None and not nj(o[mid]["core_decider"]) and o[mid]["core_decider"] is not None
               for mid, _, dec in exp["rows"])
    for mid, tab, dec in exp["rows"]:
        r = o[mid]
        if nj(r["why"]) or nj(r["tab_title"]):
            out.append(("why-names-tab:%s" % mid, None, (r["why"] if nj(r["why"]) else r["tab_title"]).why))
        else:
            out.append(("why-names-tab:%s" % mid, contains_words(r["why"], r["tab_title"]), "why %r, tab %r" % (r["why"], r["tab_title"])))
        out += equal("decider:%s" % mid, dec, {"row": r["row_decider"]})
        out += equal("tab:%s" % mid, tab, {"core": r["core_tab"]})
        if dec is None:
            v = r["core_decider"]
            if nj(v) or not held:
                out.append(("decider:%s@core" % mid, None, v.why if nj(v) else "no decided row holds a core value: an empty entry proves nothing"))
            else:
                out.append(("decider:%s@core" % mid, v in (None, "default"), "default placement, core %r" % (v,)))
        else:
            out += equal("decider:%s" % mid, _core_words(dec, exp["models"]), {"core": r["core_decider"]})
        if dec in exp["models"] and not nj(r["why"]):
            out.append(("model-name:%s" % mid, contains_words(r["why"], dec), "why %r, model %r" % (r["why"], dec)))
    return out

def oracle(exp):
    return {mid: {"why": "Placed in T-%s by %s" % (tab, dec if dec in exp["models"] else "x"), "tab_title": "T-%s" % tab, "row_decider": dec,
                  "core_tab": tab, "core_decider": _core_words(dec, exp["models"])} for mid, tab, dec in exp["rows"]}

# MUTANTS
# why-wrong-tab | the hover line names the rule's first choice, not the tab the row sits in   | why-names-tab:msg-07
# row-decider   | the row says Laya where the fixture placement is Jev                        | decider:msg-06
# core-unset    | the placement writes no decided_by in the store                             | decider:msg-02
# core-tab      | the store files msg-01 under another tab than the one it shows               | tab:msg-01
# default-filed | a default placement is stored as a rule's decision                           | decider:msg-01
# model-as-laya | the hover line of a row a catalogue model placed says Laya, not the model     | model-name:<that row>
# model-core    | the store serves 'laya' for a row a catalogue model placed                   | decider:<that row>
# (the two model mutants are added when the rows hold one a catalogue model placed: always under --selftest, see `model_mutants`)
MUTANTS = {
    "why-wrong-tab": ("hover names another tab", "why-names-tab:msg-07", setk(["msg-07", "why"], "Placed in T-needs-reply by Laya")),
    "row-decider": ("wrong decider", "decider:msg-06", setk(["msg-06", "row_decider"], "Laya")),
    "core-unset": ("not written", "decider:msg-02", setk(["msg-02", "core_decider"], None)),
    "default-filed": ("default stored as a rule", "decider:msg-01", setk(["msg-01", "core_decider"], "rule:GitHub")),
    "core-tab": ("store disagrees", "tab:msg-01", setk(["msg-01", "core_tab"], "updates")),
}

def model_mutants(exp):
    """The model case's broken copies, on the first row a catalogue model placed (none: the case has no row to break)."""
    row = next(((mid, tab) for mid, tab, dec in exp["rows"] if dec in exp["models"]), None)
    if row is None:
        return {}
    mid, tab = row
    return {
        "model-as-laya": ("model not named on hover", "model-name:%s" % mid, setk([mid, "why"], "Placed in T-%s by Laya" % tab)),
        "model-core": ("model stored as laya", "decider:%s" % mid, setk([mid, "core_decider"], "laya")),
    }

if __name__ == "__main__":
    sys.exit(run("decided-by", expected, observe, judge, oracle, dict(MUTANTS, **model_mutants(expected()))))
