#!/usr/bin/env python3
"""Ask my inbox (iPhone): cited answer, citation opens the mail, proposal waits for approval (docs/JOURNEYS.md J5.4-J5.6; C-P-ask).

Drives the RUNNING iPhone app in the simulator over the control socket ($WHOOSH_SOCKET_DIR, docs/CONTROL-SOCKET.md)
through journeys/lib. Expected values come from docs/design/fixtures/mac-main-sample.json, fixtures/tests-j7.json
and docs/CONSTANTS.md, never from a built screen. iPhone ax ids follow `<surface>.<region>[.<element>...][.<index>]`;
the lane that builds the surface registers them, and a delta is a finding for that lane.
Run: sh scripts/launch-one.sh <iphone-bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/askphone-check.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
When nothing can be driven, stderr names the one reason (no iOS simulator runtime: owner installs iOS 26.5 in Xcode > Settings > Components).
"""
import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey                                              # noqa: E402
from lib.ctl import Unreadable                                                   # noqa: E402
from lib.fixture import FX, ROWS, ACCOUNT, MSG, const                           # noqa: E402

ID = "askphone"
TITLE = "Ask my inbox (iPhone): cited answer, citation opens the mail, proposal waits for approval"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "the phone answers with cited mail, a citation opens its message, and a proposed label changes nothing until approved"

with open(os.path.join(ROOT, "fixtures", "tests-j7.json"), encoding="utf-8") as _f:
    T = json.load(_f)
R1 = MSG["msg-01"]
THREAD = FX["open_thread"]


def norm(text):
    return re.sub(r"[^a-z0-9 ]", "", text.lower()).strip()


def why_not_driven():
    d = os.environ.get("WHOOSH_SOCKET_DIR", "")
    if d and os.path.exists(os.path.join(d, "whoosh.sock")):
        return ""
    try:
        out = subprocess.run(["xcrun", "simctl", "list", "runtimes"], capture_output=True, text=True, timeout=20).stdout
    except (OSError, subprocess.SubprocessError):
        out = ""
    if "iOS" not in out:
        return "the iPhone simulator cannot run: Xcode reports iOS 26.5 is not installed (owner step: Xcode > Settings > Components > iOS 26.5)"
    return "no iPhone app socket: launch the iPhone app in the simulator, then set WHOOSH_SOCKET_DIR"


def drive(j):
    if j.ctl is None and why_not_driven():
        j.notes.append("could-not-drive: " + why_not_driven())
    s = j.step("J5.4", "Ask my inbox")
    s.arrive(("click", "iphone-palette.result.ask-my-inbox"), "iphone-ask")
    s.type(T["ask"]["question"])
    s.key("send")
    cites = lambda: s.ctl.rows("iphone-ask.citations")
    s.expect("answer-cites-account-sender-date", lambda: len(cites()) > 0 and all(c["account"] and c["sender"] and c["date"] for c in cites()), True)
    s.expect("record1-cited", lambda: R1["subject"] in [c["subject"] for c in cites()], True)
    s.expect("conversation-in-core", lambda: s.ctl.core("agent_conversation/last/turns") >= 1, True)
    s.expect_le("steps-within-limit", lambda: s.ctl.core("agent_conversation/last/steps"), const("AGENT_MAX_STEPS"))
    s.expect_le("context-chunks-within-limit", lambda: s.ctl.core("agent_conversation/last/chunks"), const("AGENT_CONTEXT_CHUNKS"))
    s = j.step("J5.5", "Open a citation")
    s.arrive(("click", "iphone-ask.citation.1"), "iphone-thread")
    s.expect("cited-message-opens", lambda: s.ctl.rows("iphone-thread.subject")[0]["subject"], R1["subject"])
    s.expect("quote-matches", lambda: s.ctl.core("agent_conversation/last/quoted_message"), R1["id"])
    s = j.step("J5.6", "Propose a label, approve")
    s.arrive(("click", "iphone-thread.back"), "iphone-ask")
    s.type(T["ask"]["propose"])
    s.key("send")
    s.expect("proposal-names-the-label", lambda: s.ctl.rows("iphone-ask.proposal")[0]["label"], T["labels"]["smart_folder"])
    s.expect("proposal-lists-the-messages", lambda: len(s.ctl.rows("iphone-ask.proposal.items")) > 0, True)
    s.expect("nothing-changed-before-approve", lambda: s.ctl.core("label_assignment/%s/count" % T["labels"]["smart_folder"]), 0)
    s.click("iphone-ask.proposal.approve")
    s.expect("labels-applied-after-approve", lambda: s.ctl.core("label_assignment/%s/count" % T["labels"]["smart_folder"]) > 0, True)


# MUTANTS: per check, the mutation of the app that must turn it red.
# J5.4/arrives-by-control | Ask my inbox is not reachable from the phone palette
# J5.4/answer-cites-account-sender-date | citations miss a field or are empty
# J5.4/record1-cited | the answer leaves out record 1
# J5.4/conversation-in-core | the chat is not saved
# J5.4/steps-within-limit | the agent loops past its step limit
# J5.4/context-chunks-within-limit | the agent reads more chunks than the limit
# J5.5/arrives-by-control | a citation chip does nothing
# J5.5/cited-message-opens | the chip opens another message
# J5.5/quote-matches | the quote is from a message the agent did not read
# J5.6/arrives-by-control | back does not return to the panel
# J5.6/proposal-names-the-label | the proposal hides the label
# J5.6/proposal-lists-the-messages | the proposal hides what it will change
# J5.6/nothing-changed-before-approve | the label is applied before approval
# J5.6/labels-applied-after-approve | approval applies nothing


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
    if r["unjudged"] and r["notes"]:
        sys.stderr.write("%s\n" % r["notes"][0])
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    ok = r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
