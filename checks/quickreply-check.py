#!/usr/bin/env python3
"""Quick reply check: ix-quickreply (J1.7).

Drives the RUNNING rig copy over the control socket (docs/CONTROL-SOCKET.md) through journeys/lib; expected
values come from docs/design/fixtures/mac-main-sample.json and docs/CONSTANTS.md, never from a built screen.
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/quickreply-check.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
"""
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey                                  # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402
from lib.fixture import FX, ROWS, ACCOUNT, MSG, const               # noqa: E402

ID = "quickreply"
TITLE = "Quick reply"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "R opens a reply field from the receiving address with its signature; send shows the undo bar and cancelling stops the send"
LIST = "mac-main.list"


def ids(s):
    return [r["id"] for r in s.ctl.rows(LIST)]


def accepted(s, *args):
    """True when the app answers ok; ok:false is False so the check after it turns red (dry run: not judged)."""
    ctl = s.ctl
    try:
        ctl.send(*args)
        return True
    except Unreadable:
        return False


def drive(j):
    q = FX["open_thread"]["quick_reply"]
    # The signature is the receiving ACCOUNT's own (PRD: "signature per address"; the core serves signature(address:) from
    # fixture accounts[].signature, closing line included). The mock's open_thread.quick_reply.signature is the design
    # picture's name line only and is not what the core serves.
    sig = [a["signature"] for a in FX["accounts"] if a["address"] == q["from"]][0]
    s = j.step("J1.7", "Quick reply")
    s.key("Enter")
    s.arrive(("key", "R"), "mac-reader")
    s.expect("sent-from-receiving-address", lambda: s.ctl.rows("mac-main.quickreply.field")[0]["from"], q["from"])
    s.expect("signature", lambda: s.ctl.rows("mac-main.quickreply.field")[0]["signature"], sig)
    s.expect("placeholder", lambda: s.ctl.rows("mac-main.quickreply.field")[0]["placeholder"], "Reply to %s…" % FX["open_thread"]["messages"][-1]["from"])   # names the sender of the thread being answered (the design fixture's own "Karen" is a placeholder of the mock)
    s.key("⌘Enter")
    s.expect("empty-body-refused", lambda: s.ctl.core("outbox_entry/count"), 0)
    s.type("Two lines.\nSecond line.")
    s.key("⌘Enter")
    s.expect("outbox-entry-created", lambda: s.ctl.core("outbox_entry/count"), 1)
    s.expect("undo-bar-seconds", lambda: s.ctl.rows("mac-main.undo.bar")[0]["seconds"], const("UNDO_SEND_DEFAULT_S"))
    s.click("mac-main.undo.bar.undo")
    s.expect("undo-cancels-the-send", lambda: s.ctl.core("outbox_entry/0/state"), "cancelled")
    s.raw("clock", "advance", const("UNDO_SEND_DEFAULT_S") + 1)
    s.expect("cancelled-stays-cancelled", lambda: s.ctl.core("outbox_entry/0/state"), "cancelled")


# MUTANTS: per check, the mutation of the app that must turn it red.
# J1.7/arrives-by-control | R opens the full composer
# J1.7/sent-from-receiving-address | the reply uses the default account
# J1.7/signature | the signature is missing, or the closing line ('Best,') is dropped from it (mutant: ReplyContext.swift:36 returns text.split(separator: "\n").last)
# J1.7/placeholder | the placeholder names nobody
# J1.7/empty-body-refused | an empty reply is queued
# J1.7/outbox-entry-created | send writes nothing
# J1.7/undo-bar-seconds | the bar counts from 30
# J1.7/undo-cancels-the-send | Undo leaves the entry live
# J1.7/cancelled-stays-cancelled | a cancelled send goes out when the window ends


def main():
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    rows = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# J") and " | " in l]
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant |")
    print("|---|---|")
    for k, v in rows:
        print("| %s | %s |" % (k, v))
    for line in r["fail_list"]:
        sys.stderr.write("FAIL %s\n" % line)
    for line in r["no_mutant"]:
        sys.stderr.write("NO MUTANT %s\n" % line)
    for line in r["stray_mutant"]:
        sys.stderr.write("STRAY MUTANT %s\n" % line)
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    ok = r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
