#!/usr/bin/env python3
"""Seam con-seams-s4/undo-window: the quick reply, the full composer and the notch count down the SAME undo
window, UNDO_SEND_DEFAULT_S from docs/CONSTANTS.md, and the message is "Sent" only after it (C-J3.5, C-J6.4
"same undo window as every composer")."""
import sys, os, time
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


def expected():
    return {"s": constant_int("UNDO_SEND_DEFAULT_S")}

# The three doors, as the parity checks reach them (par-quickreply R then ⌘Enter; par-composermac the reader's
# "open full composer" click then ⌘Enter; par-notch the stage clicks, a row click, then the Send click).
# The window is read where each screen serves it: the quick reply's own bar mac-main.undo.bar (QuickReplyFeature.swift:48,
# field seconds), the composer's mac-composer.undo.bar (ComposerMacFeature.swift:67, field seconds_left), the notch's
# mac-notch.undo (NotchRows.swift:75: text, progress, action; no seconds field, so the seconds come from the countdown
# the notch bar draws, the quick reply model's, served at mac-main.undo.bar).
SURF = ("composer", "quick-reply", "notch")
OPEN_FULL = "mac-main.reader.open-full-composer"
NOTCH_OPEN = ("mac-notch.stage.reset", "mac-notch.stage.sample", "mac-notch.stage.hover", "mac-notch.list.row.1")


def open_composer(c):
    if not c.act("click", OPEN_FULL):          # no reader up yet: Enter opens the focused thread from the list, then the same click
        c.act("key", "Enter")
        c.act("click", OPEN_FULL)


SENT_AT = {}          # monotonic time taken just before the send key or click: the window cannot start earlier


def send_on(c, surf):
    """Open the surface, type, send. Returns the rows of the bar that counts the window."""
    if surf == "composer":
        open_composer(c)
        c.act("type", "seam check"); SENT_AT["t"] = time.monotonic(); c.act("key", "⌘Enter")
        return first(c.rows("composer_undo"), "seconds_left")
    if surf == "quick-reply":
        c.act("key", "R")
        c.act("type", "seam check"); SENT_AT["t"] = time.monotonic(); c.act("key", "⌘Enter")
        return first(c.rows("undo_bar"), "seconds")
    for ax in NOTCH_OPEN:
        c.act("click", ax)
    c.act("type", "seam check"); SENT_AT["t"] = time.monotonic(); c.act("click", "mac-notch.open.send")
    bar = c.rows("mac-notch.undo")
    if nj(bar):
        return bar
    if not bar:
        return NJ("region empty: mac-notch.undo has no row after the Send click")
    sec = bar[0].get("seconds")
    return sec if sec is not None else first(c.rows("undo_bar"), "seconds")


def observe(c, exp):
    o = {"window": {}, "sent-after": {}}
    for s in SURF:
        n0 = c.core("outbox_entry/count")        # this send is entry n0: entry 0 is whatever an earlier surface sent
        SENT_AT["t"] = time.monotonic()
        o["window"][s] = send_on(c, s)
        # The fixture clock is the wall clock plus the advanced seconds (FixtureClock.nowMs, FixtureCore.swift:2204), so the
        # real seconds the send and the bar reads took count too (main88: they ate the 1 s margin on the composer and the
        # quick reply). The early read stands 1 s before the window ends counting that real time; the late read at its end.
        lag = time.monotonic() - SENT_AT["t"]
        if lag >= exp["s"] - 1:
            o["sent-after"][s] = NJ("the send and its bar read took %.1f s of real time, past the window less 1 s" % lag)
        else:
            first_move = exp["s"] - 1 - lag
            c.act("clock", "advance", "%.3f" % first_move)
            early = n0 if nj(n0) else c.core("outbox_entry/%d/state" % int(n0))
            c.act("clock", "advance", "%.3f" % (exp["s"] - first_move))
            late = n0 if nj(n0) else c.core("outbox_entry/%d/state" % int(n0))
            got = early if nj(early) else (late if nj(late) else [early != "sent", late == "sent"])
            if isinstance(got, list) and got != [True, True]:     # a miss carries the real seconds counted, so a slow read is told from a short window
                got = got + ["real seconds from the send to the first move: %.2f" % lag]
            o["sent-after"][s] = got
        if s == "notch":
            c.act("click", "mac-notch.collapse")
        else:
            c.act("key", "Escape")
    return o

def judge(o, exp):
    return equal("window-seconds", exp["s"], o["window"]) + equal("sent-only-after-window", [True, True], o["sent-after"])

def oracle(exp):
    return {"window": {s: exp["s"] for s in SURF}, "sent-after": {s: [True, True] for s in SURF}}

# MUTANTS
# notch-8s     | the notch counts the archive bar's 8 s (fixture states.undo_bar), not the send window | window-seconds@notch
# composer-5   | the composer hard-codes the minimum, 5 s                                              | window-seconds@composer
# qr-instant   | the quick reply sends before the window ends                                          | sent-only-after-window@quick-reply
MUTANTS = {
    "notch-8s": ("archive window reused", "window-seconds", setk(["window", "notch"], 8)),
    "composer-5": ("minimum hard-coded", "window-seconds", setk(["window", "composer"], constant_int("UNDO_SEND_DEFAULT_S", 1))),
    "qr-instant": ("no window", "sent-only-after-window", setk(["sent-after", "quick-reply"], [False, True])),
}
if __name__ == "__main__":
    sys.exit(run("undo-window", expected, observe, judge, oracle, MUTANTS))
