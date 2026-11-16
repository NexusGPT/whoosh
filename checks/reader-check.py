#!/usr/bin/env python3
"""Reader check: ix-reader-thread (J1.6) on the fixture's open thread (msg-01).

Drives the RUNNING rig copy over the control socket (docs/CONTROL-SOCKET.md) through journeys/lib; expected
values come from docs/design/fixtures/mac-main-sample.json and docs/CONSTANTS.md, never from a built screen.
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/reader-check.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey                                  # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402
from lib.fixture import FX, ROWS, ACCOUNT, MSG, const               # noqa: E402

ID = "reader"
TITLE = "Reader thread view"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "Enter opens the conversation with folded history, the open message, attachments and the summary"
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
    t = FX["open_thread"]
    s = j.step("J1.6", "Open the first thread")
    s.arrive(("key", "Enter"), "mac-reader")
    s.expect("subject", lambda: s.ctl.rows("mac-main.reader.subject")[0]["subject"], t["subject"])
    s.expect("participants", lambda: s.ctl.rows("mac-main.reader.participants")[0]["names"], t["participants"])
    s.expect("message-count", lambda: len(s.ctl.rows("mac-main.reader.thread")), len(t["messages"]))
    s.expect("older-messages-folded", lambda: [r["folded"] for r in s.ctl.rows("mac-main.reader.thread")], [m["folded"] for m in t["messages"]])
    s.expect("open-message-body", lambda: s.ctl.rows("mac-main.reader.thread")[-1]["body"], t["messages"][-1]["body"])
    s.expect("attachments", lambda: [r["name"] for r in s.ctl.rows("mac-main.reader.attachments")], [re.sub(r" \(\d+(\.\d+)? [KMG]?B\)$", "", a) for a in t["messages"][-1]["attachments"]])   # the core serves bare names (reader lane); the fixture adds the size
    s.expect("summary-line", lambda: s.ctl.rows("mac-main.reader.summary")[0]["text"], t["messages"][-1]["agent_summary"])
    s.expect("tracker-count-zero-for-clean-thread", lambda: s.ctl.rows("mac-main.reader.trackers")[0]["count"], t["trackers_blocked"])
    s.expect("marked-read", lambda: s.ctl.core("message/%s/read" % ROWS[0]["id"]), True)
    s.expect("enter-mounts-the-quick-reply", lambda: len(s.ctl.rows("mac-main.quickreply.field")), 1)   # A102: Enter in the list = open + quick reply, like R
    s.key("Escape")
    s.expect("first-escape-closes-the-reply-field", lambda: (s.ctl.uistate()["surface"], len(s.ctl.rows("mac-main.quickreply.field"))), ("mac-reader", 0))
    s.key("Escape")
    s.expect("escape-returns-to-list", lambda: s.ctl.uistate()["surface"], "mac-main")


# MUTANTS: per check, the mutation of the app that must turn it red.
# J1.6/arrives-by-control | Enter opens nothing
# J1.6/subject | the reader shows the previous thread
# J1.6/participants | participants are dropped
# J1.6/message-count | only the last message is shown
# J1.6/older-messages-folded | older messages open unfolded
# J1.6/open-message-body | the body is empty until a second tap
# J1.6/attachments | attachments are hidden
# J1.6/summary-line | the summary shows another thread's
# J1.6/tracker-count-zero-for-clean-thread | a clean thread reports trackers
# J1.6/marked-read | opening does not mark read
# J1.6/enter-mounts-the-quick-reply | Enter opens the reader without the quick reply field
# J1.6/first-escape-closes-the-reply-field | Esc leaves the reply field mounted, or closes the reader at once
# J1.6/escape-returns-to-list | Esc leaves the reader open


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
