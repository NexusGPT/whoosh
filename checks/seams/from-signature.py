#!/usr/bin/env python3
"""Seam con-seams-s4/from-signature: every composer path replying to the open thread shows the same From (the
account that received the thread) and the same signature (fixture open_thread.quick_reply; C-J1.7, C-J3.1
"From = the receiving account")."""
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


def expected():
    fx = fixture()
    t = fx["open_thread"]
    addr = next(a["address"] for a in fx["accounts"] if a["id"] == t["account"])
    assert addr == t["quick_reply"]["from"], "fixture: quick_reply.from is not the thread account's address"
    # The signature is the receiving account's own (accounts[].signature, closing line included): the core serves it by
    # address and it wins over the design mock's one-line quick_reply.signature (FixtureCore.signature(address:);
    # decided in docs/lanes/quickreply.md "Round 5": the PRD's "signature per address"). The mock line is its last line.
    sig = next(a["signature"] for a in fx["accounts"] if a["id"] == t["account"])
    assert sig.splitlines()[-1] == t["quick_reply"]["signature"], "fixture: the account signature does not end with the quick reply's"
    return {"from": addr, "sig": sig, "default_from": fx["accounts"][0]["address"]}

# Each path is read where the app serves it (ids proved in Sources/):
#   quick-reply  mac-main.quickreply.field   from, signature   QuickReplyFeature.swift:22 (QuickReplyIds.field)
#   composer     mac-composer.header         from, signature   ComposerMacFeature.swift:49-50 (ComposerIds.header)
#   notch        mac-notch.open.from         address           NotchRows.swift:65-68; the notch rows carry no signature (NotchRows.swift:71-73),
#                so its signature is read from the quick reply model's field row, the model the notch's reply field is drawn from.
# Doors as the parity checks reach them: R (par-quickreply), the reader's open-full-composer click (par-composermac),
# the notch stage clicks and a row click (par-notch J6.22). The notch sample mails belong to acc-04 (NotchSample.swift:10),
# the account of the fixture's open thread, so the same expected From and signature hold.
PATHS = ("quick-reply", "composer", "notch")
FIELD = "mac-main.quickreply.field"
OPEN_FULL = "mac-main.reader.open-full-composer"
NOTCH_OPEN = ("mac-notch.stage.reset", "mac-notch.stage.sample", "mac-notch.stage.hover", "mac-notch.list.row.1")


def read(c, p):
    if p == "quick-reply":
        c.act("key", "R")
        r = c.rows(FIELD)
        return first(r, "from"), first(r, "signature")
    if p == "composer":
        if not c.act("click", OPEN_FULL):          # no reader up: Enter opens the focused thread, then the same click
            c.act("key", "Enter")
            c.act("click", OPEN_FULL)
        r = c.rows("composer")
        return first(r, "from"), first(r, "signature")
    for ax in NOTCH_OPEN:
        c.act("click", ax)
    return first(c.rows("mac-notch.open.from"), "address"), first(c.rows(FIELD), "signature")


def observe(c, exp):
    c.act("key", "Enter")
    o = {"from": {}, "signature": {}}
    for p in PATHS:
        o["from"][p], o["signature"][p] = read(c, p)
        c.act("click", "mac-notch.collapse") if p == "notch" else c.act("key", "Escape")
    return o

def judge(o, exp):
    return equal("from", exp["from"], o["from"]) + equal("signature", exp["sig"], o["signature"])

def oracle(exp):
    return {"from": {p: exp["from"] for p in PATHS}, "signature": {p: exp["sig"] for p in PATHS}}

E = expected()
# MUTANTS
# default-account | the full composer opens From the first account, not the receiving one   | from@composer
# no-signature    | the notch reply omits the signature                                      | signature@notch
# last-line-only  | the composer shows only the name line, the closing line dropped           | signature@composer
MUTANTS = {
    "default-account": ("default From", "from", setk(["from", "composer"], E["default_from"])),
    "no-signature": ("signature dropped", "signature", setk(["signature", "notch"], "")),
    "last-line-only": ("closing line dropped", "signature", setk(["signature", "composer"], E["sig"].splitlines()[-1])),
}
if __name__ == "__main__":
    sys.exit(run("from-signature", expected, observe, judge, oracle, MUTANTS))
