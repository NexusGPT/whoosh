#!/usr/bin/env python3
"""Seam con-seams-s7/device-parity: Mac and iPhone agree. An archive on the iPhone is gone on the Mac, a
reply sent on the iPhone shows in the Mac thread, and accounts, colours and order are equal (C-J7.5,
C-J4.10). Needs two rigs: the Mac socket ($WHOOSH_SOCKET_DIR) and the iPhone simulator socket
($WHOOSH_IPHONE_SOCKET_DIR); without the second, every check is not judged."""
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
    return {"id": fx["focused_message"], "accounts": [[a["id"], a["name"], a["colour"]] for a in fx["accounts"]], "reply": "seam parity reply"}

def observe_shared(c, exp):
    """Both copies on the real core against ONE loopback mail server (scripts/iphone-rig.sh --with-mac --shared-server): the phone archives and replies, the Mac shows it.
    Pass or fail, never not judged: a step the app refused or a wait that ran out is the value False and its words go to stderr."""
    sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "journeys"))
    from lib.ctl import Ctl as JCtl, CouldNotDrive
    from lib import sharedserver as ss
    out = {"gone-on-mac": False, "reply-on-mac": False, "accounts": {"mac": [], "iphone": []}}
    try:
        mac = JCtl(os.environ.get("WHOOSH_SOCKET_DIR") or os.environ.get("D", "")).connect()
        phone = JCtl(os.environ.get("WHOOSH_IPHONE_SOCKET_DIR", "")).connect()
    except CouldNotDrive as e:
        sys.stderr.write("device-parity (shared server): a copy cannot be driven: %s\n" % e)
        exp["accounts"] = [["same-name-and-colour", True], ["listed-on-both", True]]
        return out
    try:
        r = ss.drive(mac, phone, exp["reply"])
        out["gone-on-mac"], out["reply-on-mac"] = r["gone_on_mac"], r["reply_in_thread"]
        # One seeded account, so no order to compare; what is judged is that both copies list the same name and colour, in two facts (a reversed list is then visible
        # to the mutant `order-lost`, which the one-element list of the account itself could not show).
        same = r["accounts"]["mac"] == r["accounts"]["iphone"]
        listed = bool(r["accounts"]["mac"]) and bool(r["accounts"]["iphone"])
        out["accounts"] = {"mac": [["same-name-and-colour", same], ["listed-on-both", listed]], "iphone": [["same-name-and-colour", same], ["listed-on-both", listed]]}
        if r.get("reply_note"):
            sys.stderr.write("device-parity (shared server): %s\n" % r["reply_note"])
        exp["accounts"] = [["same-name-and-colour", True], ["listed-on-both", True]]
    except ss.Failed as e:
        sys.stderr.write("device-parity (shared server): FAILED: %s\n" % e)
        exp["accounts"] = [["same-name-and-colour", True], ["listed-on-both", True]]
    finally:
        mac.close(); phone.close()
    return out


def observe(c, exp):
    from_shared = os.environ.get("WHOOSH_SHARED_SERVER", "").strip()
    if from_shared and c.live:
        return observe_shared(c, exp)
    ph = Ctl(("WHOOSH_IPHONE_SOCKET_DIR",))
    if not ph.live:
        why = NJ("no iPhone rig socket (WHOOSH_IPHONE_SOCKET_DIR)")
        return {"gone-on-mac": why, "reply-on-mac": why, "accounts": {"mac": why, "iphone": why}}
    # the Mac sidebar serves `colour`; the phone's mailboxes list serves `mark_colour` (InboxPhoneFeature.swift:93-94)
    tab = lambda r, f: r if nj(r) else [[x.get("id"), x.get("name"), x.get(f)] for x in r]
    accs = {"mac": tab(c.rows("sidebar_accounts"), "colour"), "iphone": tab(ph.rows("iphone_accounts"), "mark_colour")}
    # What joins two copies: the MAIL SERVER. An archive or a reply is a mail action the engine sends to the account's server; DeviceSync carries only settings-like
    # records (the Mac's `settings_sync/records_carried` is the constant list account, account_icon_file, decision_model, note, reminder, rule, setting, split_tab,
    # template, vip, voice_profile: no message and no action kind), so a loopback DeviceSync channel alone would not move the archive. Two fixture-core copies each own a
    # private mailbox with no server between them: the Mac cannot show what the phone did, which says nothing about the app (phone4: both red/not judged, accounts green).
    # Without the shared server the two copies are fixture cores with a private mailbox each: nothing the phone does can reach the Mac, so the two cases are not judged
    # (they used to act on the phone first and read the Mac: that proves nothing about the app). The whole answer is in journeys/lib/sharedserver.py WHY.
    sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "journeys"))
    from lib import sharedserver as ss
    why = NJ(ss.WHY)
    return {"gone-on-mac": why, "reply-on-mac": why, "accounts": accs}

def judge(o, exp):
    return (equal("archive-gone-on-mac", True, {"mac-list": o["gone-on-mac"]}) + equal("reply-in-mac-thread", True, {"mac-thread": o["reply-on-mac"]})
            + equal("accounts", exp["accounts"], o["accounts"]))

def oracle(exp):
    return {"gone-on-mac": True, "reply-on-mac": True, "accounts": {"mac": [list(a) for a in exp["accounts"]], "iphone": [list(a) for a in exp["accounts"]]}}

# MUTANTS
# phone-local-archive | the iPhone archives locally and never tells the server         | archive-gone-on-mac
# order-lost          | the QR transfer reverses the account order on the iPhone       | accounts@iphone
MUTANTS = {
    "phone-local-archive": ("archive not synced", "archive-gone-on-mac", setk(["gone-on-mac"], False)),
    "order-lost": ("order lost", "accounts", lambda o: (o["accounts"]["iphone"].reverse() or o)),
}
def selftest_shared():
    """observe_shared and judge on two stand-in copies and one stand-in server (journeys/lib/sharedserver.py --selftest has the drive itself): the clean pair is all PASS,
    each broken copy turns its own label red, and a copy that refuses a step is red (never not judged)."""
    sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "journeys"))
    from lib import ctl as ctlmod, sharedserver as ss
    ss.POLL_S, ss.THREAD_EVERY_S, ss.ADD_WAIT_S, ss.LIST_WAIT_S, ss.PROPAGATE_WAIT_S, ss.SEND_WAIT_S = 0.05, 0.1, 3, 3, 2, 3
    saved = (ctlmod.Ctl, dict(os.environ))
    n = bad = 0
    cases = [(None, set()), ("archive-not-propagated", {"archive-gone-on-mac@mac-list", "reply-in-mac-thread@mac-thread"}), ("reply-not-propagated", {"reply-in-mac-thread@mac-thread"}),
             ("reply-in-wrong-thread", {"reply-in-mac-thread@mac-thread"}), ("phone-refuses-archive", {"archive-gone-on-mac@mac-list", "reply-in-mac-thread@mac-thread", "accounts@mac", "accounts@iphone"})]
    for mutant, red in cases:
        w = ss._World(mutant)
        copies = iter([ss._Copy(w, "mac"), ss._Copy(w, "phone")])
        class Fake:
            def __init__(self, d): pass
            def connect(self): return next(copies)
        ctlmod.Ctl = Fake
        os.environ.update(WHOOSH_SOCKET_DIR="x", WHOOSH_IPHONE_SOCKET_DIR="y")
        exp = expected()
        got = judge(observe_shared(None, exp), exp)
        failed = {r[0] for r in got if r[1] is False}
        ok = failed == red and not any(r[1] is None for r in got)
        n += 1; bad += not ok
        print("%s %s: red labels %s (want %s)" % ("PASS" if ok else "FAIL", "clean pair" if mutant is None else "broken copy " + mutant, sorted(failed), sorted(red)))
    ctlmod.Ctl = saved[0]
    print("selftest-shared: %d cases, %d failed" % (n, bad))
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    if "--selftest-shared" in sys.argv:
        sys.exit(selftest_shared())
    sys.exit(run("device-parity", expected, observe, judge, oracle, MUTANTS))
