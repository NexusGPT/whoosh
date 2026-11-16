#!/usr/bin/env python3
"""Seam con-seams-s5/account-identity: account names, colours and order are equal on Welcome, the sidebar,
Settings: Accounts and (slice 7) the iPhone (C-J4.6, C-J4.10 "the same 30 accounts, names, colours, order").
The iPhone surface is read from a second rig socket, $WHOOSH_IPHONE_SOCKET_DIR; without it, not judged."""
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
    return {"accounts": [[a["id"], a["name"], a["colour"]] for a in fx["accounts"]], "addresses": [a["address"] for a in fx["accounts"]]}

WELCOME = "mac-welcome-accounts.accounts"
SETTINGS_ACCOUNTS = "mac-settings-accounts"

def _tab(rows, colour="colour"):
    # the Mac lists serve `colour`; the phone's mailboxes list serves `mark_colour` (InboxPhoneFeature.swift:93-94)
    return rows if nj(rows) else [[r.get("id"), r.get("name"), r.get(colour)] for r in rows]

def observe(c, exp):
    o = {"sidebar": _tab(c.rows("sidebar_accounts"))}
    c.act("key", "⌘,")
    o["settings-accounts"] = _tab(c.rows("settings_accounts"))
    # The Welcome list holds only rows added on Welcome (WelcomeModel.swift:12). Stage the fixture's addresses the way journeys/J4.py:411-418 does:
    # Settings > Accounts "Move accounts to iPhone" opens Welcome, reset, type the addresses, Paste list. A pasted row carries an address and a
    # default name and colour (WelcomeModel.swift:64-69), so the Welcome surface is judged on count and ORDER of addresses (label welcome-order),
    # not on name and colour: no door stages those (docs/lanes/gap-seams.md, finding A1).
    c.act("click", SETTINGS_ACCOUNTS + ".move-to-iphone"); c.act("click", "mac-welcome-accounts.reset")
    c.act("type", " ".join(exp["addresses"])); c.act("click", "mac-welcome-accounts.paste-list")
    w = c.rows(WELCOME)
    o["welcome"] = w if nj(w) else ([r.get("address") for r in w] if w else NJ("%s serves no row after the paste list of %d addresses" % (WELCOME, len(exp["addresses"]))))
    c.act("key", "Escape")
    ph = Ctl(("WHOOSH_IPHONE_SOCKET_DIR",))
    o["iphone"] = _tab(ph.rows("iphone_accounts"), "mark_colour") if ph.live else NJ("no iPhone rig socket (WHOOSH_IPHONE_SOCKET_DIR)")
    return o

def judge(o, exp):
    named = {k: v for k, v in o.items() if k != "welcome"}
    return equal("accounts", exp["accounts"], named) + equal("welcome-order", exp["addresses"], {"welcome": o["welcome"]})

def oracle(exp):
    o = {s: [list(a) for a in exp["accounts"]] for s in ("sidebar", "settings-accounts", "iphone")}
    o["welcome"] = list(exp["addresses"])
    return o

# MUTANTS
# settings-sorted | Settings: Accounts sorts by name instead of the owner's order       | accounts@settings-accounts
# iphone-colour   | the iPhone loses one colour in settings sync                         | accounts@iphone
# welcome-order   | the Welcome list reverses the pasted order                           | welcome-order@welcome
MUTANTS = {
    "settings-sorted": ("alphabetical order", "accounts", lambda o: (o.__setitem__("settings-accounts", sorted(o["settings-accounts"], key=lambda a: a[1])) or o)),
    "iphone-colour": ("colour lost", "accounts", lambda o: (o["iphone"][3].__setitem__(2, "#000000") or o)),
    "welcome-order": ("pasted order reversed", "welcome-order", lambda o: (o["welcome"].reverse() or o)),
}
if __name__ == "__main__":
    sys.exit(run("account-identity", expected, observe, judge, oracle, MUTANTS))
