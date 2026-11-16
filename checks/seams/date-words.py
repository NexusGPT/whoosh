#!/usr/bin/env python3
"""Seam con-seams-s2/date-words: the snooze picker's date words are words search's date filter accepts, and both
resolve to the same time (docs/CONSTANTS.md SNOOZE_LATER_TODAY · TOMORROW · NEXT_WEEK; contract C-J2.2 "each
recognised filter becomes exactly one chip"). The picker is mac-main.snooze.options (served: title, resolves_to);
search is read through its chips (mac-search.chips) after typing "before <word>". Positive control: "before <a
month>" (the date word search is known to parse) must make a chip, else the search probe is broken and every
absence of a chip is not judged."""
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

CONTROL = "before June"

def expected():
    names, vals = constant_row("SNOOZE_LATER_TODAY")
    words = [" ".join(norm_words(n.replace("SNOOZE_", "").replace("_", " "))) for n in names]
    return {"words": words}

def _chip(c, text):
    c.act("key", "Escape"); c.act("key", "/"); c.act("key", "⌘A"); c.act("type", text)
    ch = c.rows("search_chips")
    if nj(ch):
        return ch, ch
    hit = [r for r in ch if r.get("kind") == "before"]
    return bool(hit), (hit[0].get("resolves_to", NJ("chip serves no resolves_to")) if hit else NJ("no chip"))

def observe(c, exp):
    c.act("key", "H")
    sn = c.rows("snooze")
    c.act("key", "Escape")
    picker = {}
    for w in exp["words"]:
        if nj(sn):
            picker[w] = {"has": sn, "time": sn}
        else:
            hit = [r for r in sn if contains_words(r.get("title", ""), w)]
            picker[w] = {"has": bool(hit), "time": hit[0].get("resolves_to") if hit else NJ("picker lacks the word")}
    control, _ = _chip(c, CONTROL)
    search = {}
    for w in exp["words"]:
        has, t = _chip(c, "before " + w)
        search[w] = {"has": has, "time": t}
    return {"picker": picker, "control": control, "search": search}

def judge(o, exp):
    out = equal("control-before-month", True, {"search": o["control"]})
    for w in exp["words"]:
        out += equal("word-in-picker:%s" % w, True, {"picker": o["picker"][w]["has"]})
        s = o["search"][w]["has"]
        if o["control"] is not True and not nj(s):
            s = NJ("control failed: the search probe is broken, a missing chip means nothing")
        out += equal("word-in-search:%s" % w, True, {"search": s})
        a, b = o["picker"][w]["time"], o["search"][w]["time"]
        if nj(a) or nj(b):
            out.append(("same-time:%s" % w, None, (a if nj(a) else b).why))
        else:
            out.append(("same-time:%s" % w, a == b, "picker %r, search %r" % (a, b)))
    return out

def oracle(exp):
    return {"picker": {w: {"has": True, "time": "t:" + w} for w in exp["words"]}, "control": True,
            "search": {w: {"has": True, "time": "t:" + w} for w in exp["words"]}}

# MUTANTS
# next-7-days   | search refuses "next week" (it only knows its own wording)               | word-in-search:next week
# tomorrow-9    | search resolves "tomorrow" to a different time than the picker           | same-time:tomorrow
# picker-drops  | the picker no longer offers "later today"                                | word-in-picker:later today
# probe-dead    | search makes no chip at all, not even for a month: the control goes red   | control-before-month
MUTANTS = {
    "next-7-days": ("other words", "word-in-search:next week", setk(["search", "next week", "has"], False)),
    "tomorrow-9": ("other time", "same-time:tomorrow", setk(["search", "tomorrow", "time"], "09:00")),
    "picker-drops": ("picker word gone", "word-in-picker:later today", setk(["picker", "later today", "has"], False)),
    "probe-dead": ("dead probe", "control-before-month", setk(["control"], False)),
}
if __name__ == "__main__":
    sys.exit(run("date-words", expected, observe, judge, oracle, MUTANTS))
