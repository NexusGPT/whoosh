#!/usr/bin/env python3
"""Welcome and Accounts (iPhone): scan the code from the Mac, 30 accounts arrive, one notification ask, the inbox (docs/JOURNEYS.md J4.10).

Drives the RUNNING iPhone app in the simulator over the control socket ($WHOOSH_SOCKET_DIR, docs/CONTROL-SOCKET.md) through journeys/lib,
the same harness as checks/inboxphone-check.py. Expected values come from docs/design/fixtures/mac-main-sample.json (the Mac fixture's
accounts: names, colours, order) and docs/CONSTANTS.md (QR_EXPIRY_S), never from a built screen. The code reaches the phone through `type`
(a test copy takes `fixture:` words instead of the camera); no password is ever typed or read.
Run: sh scripts/launch-one.sh <iphone-bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/welcomephone-check.py'
Self-test: python3 checks/welcomephone-check.py --selftest   (every case must pass on a stand-in socket and go RED on its mutant)
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
"""
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, Journey, declared_mutants                    # noqa: E402
from lib.ctl import Unreadable                                                    # noqa: E402
from lib.fixture import FX, ROWS, const                                           # noqa: E402

ID = "welcomephone"
TITLE = "Welcome and Accounts (iPhone): scan the code, the accounts arrive, notifications, the inbox"
TARGET = "LOCAL"
PLATFORM = "iphone"
VISIBLE_OUTCOME = ("a code from the Mac brings the 30 accounts with their names, colours and order; an expired or wrong code says why and "
                   "scanning works again; the phone asks for notifications once and lands on the inbox; no password field is ever shown")

ACCOUNTS = FX["accounts"]
EXPIRY_S = const("QR_EXPIRY_S")
SURFACE = "iphone-welcome-accounts"
SCAN, NO_MAC, CANCEL = "iphone-welcome.scan", "iphone-welcome.no-mac", "iphone-welcome.cancel"
AGAIN, NEXT, ALLOW, NOT_NOW = "iphone-welcome.scan-again", "iphone-welcome.continue", "iphone-welcome.allow", "iphone-welcome.not-now"
CONTROLS = {SCAN, NO_MAC, CANCEL, AGAIN, NEXT, ALLOW, NOT_NOW}      # every control the brief draws, across the steps
RESTART = "iphone-welcome.test-restart"                               # the test door, not a control


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
    s = j.step("J4.10", "Welcome and accounts")
    live = j.ctl is not None
    rows = lambda ax: s.ctl.rows(ax)
    step = lambda: rows("iphone-welcome.step")[0]["step"]
    ctrls = lambda: [r["ax"] for r in rows("iphone-welcome.controls")]
    refusal = lambda: rows("iphone-welcome.refusal")[0]
    listed, tapped, fields_seen = set(), [], []

    def look():
        """Record what the step on screen draws: its controls (for the tap census) and its text inputs (for the password check)."""
        if live:
            listed.update(ctrls())
            fields_seen.append(rows("iphone-welcome.fields"))

    def tap(ax):
        tapped.append(ax)
        s.click(ax)

    def reached(word, name):
        s.expect(name, step, word)
        look()

    # 1. first launch
    s.expect("starts-on-welcome", lambda: s.ctl.uistate()["surface"], SURFACE)
    reached("first", "first-launch-asks-for-the-code")
    s.expect("first-controls", ctrls, [SCAN, NO_MAC])
    # 2. no Mac yet
    tap(NO_MAC)
    reached("noMac", "no-mac-step")
    s.expect("no-mac-offers-scanning", ctrls, [SCAN])
    # 3. scanning, cancel, scan again
    tap(SCAN)
    reached("scanning", "scan-opens-scanning")
    s.expect("scanning-controls", ctrls, [CANCEL])
    tap(CANCEL)
    reached("first", "cancel-returns")
    tap(SCAN)
    # 4. an expired code: issued, the clock moves past QR_EXPIRY_S, then scanned
    s.type("fixture:issue")
    s.raw("clock", "advance", EXPIRY_S + 1)
    s.type("fixture:scan-issued")
    reached("refused", "expired-code-refused")
    s.expect("expired-reason", lambda: refusal()["reason"], "expired")
    s.expect("expired-reason-words", lambda: "expired" in refusal()["text"].lower() and str(EXPIRY_S // 60) in refusal()["text"], True)
    s.expect("refused-controls", ctrls, [AGAIN, NO_MAC])
    tap(AGAIN)
    reached("scanning", "scan-again-works")
    # 5. a code the Mac never issued, a code that is not Whoosh's, a code already used: each refused with its reason, scanning again each time
    s.type("fixture:wrong")
    s.expect("wrong-code-refused", lambda: (step(), refusal()["reason"]), ("refused", "unknown"))
    tap(AGAIN)
    s.type("fixture:foreign")
    s.expect("foreign-code-refused", lambda: (step(), refusal()["reason"]), ("refused", "not-ours"))
    tap(AGAIN)
    s.type("fixture:used")
    s.expect("used-code-refused", lambda: (step(), refusal()["reason"]), ("refused", "already-used"))
    tap(AGAIN)
    # 6. a valid code: the 30 accounts arrive with the Mac's names, colours and order
    s.type("fixture:valid")
    reached("arriving", "valid-code-arrives")
    arrived = lambda: rows("iphone-welcome.accounts")
    s.expect("thirty-accounts", lambda: len(arrived()), len(ACCOUNTS))
    s.expect("same-names", lambda: [r["name"] for r in arrived()], [a["name"] for a in ACCOUNTS])
    s.expect("same-order", lambda: [r["id"] for r in arrived()], [a["id"] for a in ACCOUNTS])
    s.expect("same-colours", lambda: [r["mark_colour"].lower() for r in arrived()], [a["colour"].lower() for a in ACCOUNTS])
    s.expect("count-matches", lambda: tuple(rows("iphone-welcome.count")[0][k] for k in ("arrived", "total", "complete")),
             (len(ACCOUNTS), len(ACCOUNTS), True))
    s.expect("arriving-controls", ctrls, [NEXT])
    # 7. notifications: asked once, the test copy never reaches the system
    tap(NEXT)
    reached("notify", "continue-opens-notifications")
    s.expect("notify-controls", ctrls, [ALLOW, NOT_NOW])
    s.expect("notification-not-asked-yet", lambda: rows("iphone-welcome.notify")[0]["asked"], 0)
    tap(ALLOW)
    s.expect("notification-asked-once", lambda: rows("iphone-welcome.notify")[0]["asked"], 1)
    s.expect("test-copy-asks-no-system", lambda: rows("iphone-welcome.notify")[0]["answer"], "skipped-in-test")
    # 8. the inbox
    s.expect("lands-on-inbox", lambda: s.ctl.uistate()["surface"], "iphone-inbox")
    s.expect("inbox-has-the-mail", lambda: [r["id"] for r in rows("iphone-inbox.list")][:1], [ROWS[0]["id"]])
    # 9. the other answer: a second first launch, Not now lands on the inbox too
    s.click(RESTART)
    reached("first", "restart-shows-the-first-launch")
    tap(SCAN)
    s.type("fixture:valid")
    tap(NEXT)
    tap(NOT_NOW)
    s.expect("not-now-lands-on-inbox", lambda: s.ctl.uistate()["surface"], "iphone-inbox")
    s.expect("not-now-answer", lambda: rows("iphone-welcome.notify")[0]["answer"], "declined")
    # 10. the census
    s.expect("no-password-field", lambda: (s.ctl.uistate(), [f for f in fields_seen if f] if len(fields_seen) >= 8 else ["only %d steps read" % len(fields_seen)])[1], [])
    s.expect("every-control-tapped", lambda: (s.ctl.uistate(), sorted(set(tapped) & listed), sorted(listed), sorted(CONTROLS))[1:],
             (sorted(CONTROLS), sorted(CONTROLS), sorted(CONTROLS)))


# MUTANTS: per check, the mutation of the app that must turn it red.
# J4.10/starts-on-welcome | the app opens on the inbox and never shows the welcome
# J4.10/first-launch-asks-for-the-code | the first launch opens on a step other than the code ask
# J4.10/first-controls | the first screen lacks the scan button or the no-Mac link
# J4.10/no-mac-step | the no-Mac link does nothing
# J4.10/no-mac-offers-scanning | the no-Mac screen gives no way back to scanning
# J4.10/scan-opens-scanning | the scan button does not open the scanner
# J4.10/scanning-controls | the scanning step has no way back
# J4.10/cancel-returns | Back on the scanning step stays on scanning
# J4.10/expired-code-refused | an expired code is accepted
# J4.10/expired-reason | the refusal names another reason than expiry
# J4.10/expired-reason-words | the refusal shows no words, or no minutes
# J4.10/refused-controls | the refusal offers no scan again
# J4.10/scan-again-works | scan again does not reopen the scanner
# J4.10/wrong-code-refused | a code the Mac never issued is accepted
# J4.10/foreign-code-refused | a code that is not Whoosh's is accepted
# J4.10/used-code-refused | a used code is accepted a second time
# J4.10/valid-code-arrives | a valid code does not start the arrival
# J4.10/thirty-accounts | fewer or more than 30 accounts arrive
# J4.10/same-names | an account arrives under another name
# J4.10/same-order | the accounts arrive in another order
# J4.10/same-colours | an account arrives in another colour
# J4.10/count-matches | the count line disagrees with the list
# J4.10/arriving-controls | the arriving step has no Continue
# J4.10/continue-opens-notifications | Continue does not reach the notifications step
# J4.10/notify-controls | the notifications step lacks Allow or Not now
# J4.10/notification-not-asked-yet | the permission is asked before the step
# J4.10/notification-asked-once | the permission is asked twice, or not recorded
# J4.10/test-copy-asks-no-system | a test copy reaches the system prompt
# J4.10/lands-on-inbox | Allow does not land on the inbox
# J4.10/inbox-has-the-mail | the inbox opens empty
# J4.10/restart-shows-the-first-launch | the test door does not restart the first launch
# J4.10/not-now-lands-on-inbox | Not now does not land on the inbox
# J4.10/not-now-answer | Not now is recorded as an allow
# J4.10/no-password-field | a step shows a password field (or fewer than 8 steps were read)
# J4.10/every-control-tapped | a step lists a control the check never taps


# ---------------------------------------------------------------- the selftest: a stand-in socket that behaves as the brief says
# Written from docs/JOURNEYS.md J4.10 and the brief, never from the app's reply. Its mutants are the declared MUTANTS above (one per label);
# a second table deletes one ACT of the journey (the `skip` hook drops it) and demands the check that reads its effect to go red,
# because a check already true before its act is decoration.
class ModelCtl:
    instant = True
    CONTROLS_OF = {"first": [SCAN, NO_MAC], "scanning": [CANCEL], "refused": [AGAIN, NO_MAC], "arriving": [NEXT], "notify": [ALLOW, NOT_NOW],
                   "noMac": [SCAN], "done": []}

    def __init__(self, mut=(), skip=None):
        self.mut, self.skip = set(mut), skip
        self.step = "done" if "J4.10/starts-on-welcome" in self.mut else "first"
        self.reason, self.arrived, self.total = "", [], 0
        self.asked, self.answer = 0, ""
        self.issued_at, self.clock, self.issued_used = None, 0, set()
        self.token_n = 0

    def has(self, label):
        return label in self.mut

    # -- the pairing: a code is valid once, for EXPIRY_S seconds
    def _refuse(self, why):
        self.reason, self.step = why, "refused"

    def _accept(self):
        accounts = list(ACCOUNTS)
        if self.has("J4.10/thirty-accounts"):
            accounts = accounts[:-1]
        if self.has("J4.10/same-names"):
            accounts = [dict(a, name=(a["name"] if i else "Somebody Else")) for i, a in enumerate(accounts)]
        if self.has("J4.10/same-order"):
            accounts = list(reversed(accounts))
        if self.has("J4.10/same-colours"):
            accounts = [dict(a, colour="#000000") for a in accounts]
        self.arrived, self.total = accounts, len(ACCOUNTS)
        self.step = "arriving"

    def _scan(self, word):
        if word in ("fixture:valid", "fixture:scan-issued"):
            if word == "fixture:scan-issued":
                if self.issued_at is None:
                    return
                if self.clock - self.issued_at >= EXPIRY_S and not self.has("J4.10/expired-code-refused"):
                    return self._refuse("unknown" if self.has("J4.10/expired-reason") else "expired")
                if self.clock - self.issued_at >= EXPIRY_S:
                    return self._accept()
            return self._accept() if not self.has("J4.10/valid-code-arrives") else None
        if word == "fixture:wrong":
            return self._accept() if self.has("J4.10/wrong-code-refused") else self._refuse("unknown")
        if word == "fixture:foreign":
            return self._accept() if self.has("J4.10/foreign-code-refused") else self._refuse("not-ours")
        if word == "fixture:used":
            return self._accept() if self.has("J4.10/used-code-refused") else self._refuse("already-used")

    def type(self, text):
        if self.skip and self.skip("type", text, self):
            return {"ok": True}
        if self.step != "scanning":
            return {"ok": True}
        if text == "fixture:issue":
            self.issued_at = self.clock
            return {"ok": True}
        self._scan(text)
        return {"ok": True}

    def send(self, verb, *args):
        if self.skip and self.skip(verb, args, self):
            return {"ok": True}
        if verb == "clock" and args[0] == "advance":
            self.clock += int(args[1])
            return {"ok": True}
        raise Unreadable("%s %s: not modelled" % (verb, args))

    def _controls(self):
        c = list(self.CONTROLS_OF[self.step])
        if self.step == "first" and self.has("J4.10/first-controls"):
            c = [SCAN]
        if self.step == "noMac" and self.has("J4.10/no-mac-offers-scanning"):
            c = []
        if self.step == "scanning" and self.has("J4.10/scanning-controls"):
            c = []
        if self.step == "refused" and self.has("J4.10/refused-controls"):
            c = [NO_MAC]
        if self.step == "arriving" and self.has("J4.10/arriving-controls"):
            c = []
        if self.step == "notify" and self.has("J4.10/notify-controls"):
            c = [ALLOW]
        if self.step == "notify" and self.has("J4.10/every-control-tapped"):
            c = c + ["iphone-welcome.extra"]
        return c

    def click(self, ax):
        if self.skip and self.skip("click", ax, self):
            return {"ok": True}
        if ax == RESTART:
            if self.has("J4.10/restart-shows-the-first-launch"):
                return {"ok": True}
            self.step, self.reason, self.arrived, self.total, self.asked, self.answer = "first", "", [], 0, 0, ""
            return {"ok": True}
        if ax not in self._controls():
            raise Unreadable("click %s: nothing to click" % ax)
        if ax == NO_MAC:
            self.step = "first" if self.has("J4.10/no-mac-step") else "noMac"
        elif ax in (SCAN, AGAIN):
            self.step = "first" if (self.has("J4.10/scan-opens-scanning") and ax == SCAN) or (self.has("J4.10/scan-again-works") and ax == AGAIN) else "scanning"
        elif ax == CANCEL:
            self.step = "scanning" if self.has("J4.10/cancel-returns") else "first"
        elif ax == NEXT:
            self.step = "arriving" if self.has("J4.10/continue-opens-notifications") else "notify"
        elif ax in (ALLOW, NOT_NOW):
            self.asked += 1 if not self.has("J4.10/notification-asked-once") else 0
            if self.has("J4.10/notification-asked-once"):
                self.asked += 2
            self.answer = "allowed" if (ax == ALLOW and self.has("J4.10/test-copy-asks-no-system")) or (ax == NOT_NOW and self.has("J4.10/not-now-answer")) \
                else ("skipped-in-test" if ax == ALLOW else "declined")
            self.step = "arriving" if (ax == ALLOW and self.has("J4.10/lands-on-inbox")) or (ax == NOT_NOW and self.has("J4.10/not-now-lands-on-inbox")) else "done"
        return {"ok": True}

    def rows(self, ax):
        if ax == "iphone-welcome.step":
            return [{"ax": ax, "step": "scanning" if self.has("J4.10/first-launch-asks-for-the-code") and self.step == "first" else self.step, "scans": 0}]
        if ax == "iphone-welcome.controls":
            return [{"ax": a, "label": a, "enabled": True} for a in self._controls()]
        if ax == "iphone-welcome.fields":
            pw = self.has("J4.10/no-password-field") and self.step in ("scanning", "refused")
            return [{"ax": "iphone-welcome.fields.0", "kind": "password"}] if pw else []
        if ax == "iphone-welcome.refusal":
            if self.step != "refused":
                return []
            text = "" if self.has("J4.10/expired-reason-words") else {
                "expired": "This code expired after %d minutes. Your Mac shows a new one." % (EXPIRY_S // 60),
                "unknown": "This code is not from your Mac.", "not-ours": "This is not a Whoosh code.", "already-used": "This code was already used."}[self.reason]
            return [{"ax": ax, "reason": self.reason, "text": text, "scan_again": True}]
        if ax == "iphone-welcome.count":
            n = len(self.arrived) + (1 if self.has("J4.10/count-matches") else 0)
            return [{"ax": ax, "arrived": n, "total": self.total, "complete": n == self.total}]
        if ax == "iphone-welcome.accounts":
            return [{"ax": "iphone-welcome.accounts.%d" % i, "id": a["id"], "name": a["name"], "address": a["address"], "mark_colour": a["colour"]}
                    for i, a in enumerate(self.arrived)]
        if ax == "iphone-welcome.notify":
            asked = 1 if (self.has("J4.10/notification-not-asked-yet") and self.step == "notify") else self.asked
            return [{"ax": ax, "asked": asked, "answer": self.answer}]
        if ax == "iphone-inbox.list":
            if self.step != "done" or self.has("J4.10/inbox-has-the-mail"):
                return []
            return [{"ax": "iphone-inbox.list.row.%d" % i, "id": m["id"]} for i, m in enumerate(ROWS)]
        raise Unreadable("rows %s: not modelled" % ax)

    def uistate(self):
        return {"surface": "iphone-inbox" if self.step == "done" else SURFACE}

    def close(self):
        pass


def model_run(mutants=(), skip=None):
    """The journey on the stand-in: (labels failed, labels not judged, labels run)."""
    j = Journey(ID, ModelCtl(mutants, skip))
    drive(j)
    cs = j.checks()
    return {c.label for c in cs if c.state == "fail"}, {c.label for c in cs if c.state == "unjudged"}, {c.label for c in cs}


def nth(n, pred):
    """A drop rule that fires on the n-th time `pred` matches (1-based), once."""
    seen = {"k": 0}

    def rule(v, a, m):
        if not pred(v, a, m):
            return False
        seen["k"] += 1
        return seen["k"] == n
    return rule


# Deleting one act of the journey must turn the check that reads its effect red: label -> (what is deleted, the drop rule)
ACT_DELETED = {
    "J4.10/valid-code-arrives": ("the first valid code", nth(1, lambda v, a, m: v == "type" and a == "fixture:valid")),
    "J4.10/expired-code-refused": ("the clock moving past the expiry", lambda v, a, m: v == "clock"),
    "J4.10/scan-again-works": ("the tap on scan again after the expired code", lambda v, a, m: v == "click" and a == AGAIN and m.reason == "expired"),
    "J4.10/lands-on-inbox": ("the tap on Allow", lambda v, a, m: v == "click" and a == ALLOW),
    "J4.10/not-now-lands-on-inbox": ("the tap on Not now", lambda v, a, m: v == "click" and a == NOT_NOW),
}


def selftest(path):
    labels = declared_mutants(path)
    base_f, base_u, base_run = model_run()
    rows, missed = [], []
    for label in labels:
        f, _, _ = model_run((label,))
        rows.append((label, label in f))
        if label not in f:
            missed.append(label)
    acts, act_missed = [], []
    for label, (what, rule) in ACT_DELETED.items():
        f, _, _ = model_run((), rule)
        acts.append((label, what, label in f))
        if label not in f:
            act_missed.append(label)
    stray = sorted(base_run - set(labels))
    print("clean model: failed=%s unjudged=%s; labels without a mutant=%s" % (sorted(base_f), sorted(base_u), stray))
    print("| check | mutant (declared) | seen red |")
    print("|---|---|---|")
    for label, red in rows:
        print("| %s | the model carries this mutant | %s |" % (label, "RED on this case" if red else "NOT RED"))
    print("| check | act deleted | seen red |")
    print("|---|---|---|")
    for label, what, red in acts:
        print("| %s | %s | %s |" % (label, what, "RED on this case" if red else "NOT RED"))
    bad = len(base_f) + len(base_u) + len(missed) + len(act_missed) + len(stray)
    print("%d mutants, %d not seen red; %d acts deleted, %d not seen red; %d clean failures, %d clean not judged" % (
        len(rows), len(missed), len(acts), len(act_missed), len(base_f), len(base_u)))
    return 0 if bad == 0 else 1


def main():
    path = os.path.abspath(__file__)
    if "--selftest" in sys.argv:
        return selftest(path)
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
