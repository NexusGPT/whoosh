#!/usr/bin/env python3
"""Unified Inbox (iPhone): list, swipe archive, swipe snooze, undo (docs/JOURNEYS.md J7.3, J4.10; C-J7.3, C-PICK-mark).

Drives the RUNNING iPhone app in the simulator over the control socket ($WHOOSH_SOCKET_DIR, docs/CONTROL-SOCKET.md)
through journeys/lib. Expected values come from docs/design/fixtures/mac-main-sample.json, fixtures/tests-j7.json
and docs/CONSTANTS.md, never from a built screen. iPhone ax ids follow `<surface>.<region>[.<element>...][.<index>]`;
the lane that builds the surface registers them, and a delta is a finding for that lane.
Self-test: python3 checks/inboxphone-check.py --selftest   (a stand-in socket: every case passes on it and goes RED on its declared mutant)
Run: sh scripts/launch-one.sh <iphone-bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/inboxphone-check.py'
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
from lib.harness import run_journey, Journey, declared_mutants, print_cases     # noqa: E402
from lib.ctl import Unreadable                                                   # noqa: E402
from lib.fixture import FX, ROWS, ACCOUNT, MSG, const, served_unread             # noqa: E402
from lib.phonestart import leave_welcome                                          # noqa: E402

ID = "inboxphone"
TITLE = "Unified Inbox (iPhone): list, swipe archive, swipe snooze, undo"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "the phone lists the fixture mail with account marks; a swipe archives or snoozes a row and shows undo; a refused swipe brings the row back with the reason"

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
    L = "iphone-inbox.list"
    ids = lambda: [r["id"] for r in s.ctl.rows(L)]
    s = j.step("J7.3", "Inbox list and swipes")
    s._act(lambda: leave_welcome(s.j.ctl))      # a test copy starts its first launch (the welcome): leave it by the flow's own controls first
    s.expect("starts-on-inbox", lambda: s.ctl.uistate()["surface"], "iphone-inbox")
    s.expect("rows-in-fixture-order", lambda: ids(), [m["id"] for m in ROWS])
    s.expect("sender-subject-per-row", lambda: [(r["sender"], r["subject"]) for r in s.ctl.rows(L)], [(m["sender"], m["subject"]) for m in ROWS])
    s.expect("unread-flags", lambda: [r["unread"] for r in s.ctl.rows(L)], [m["unread"] for m in ROWS])
    s.expect("account-mark-colour-per-row", lambda: [r["mark_colour"].lower() for r in s.ctl.rows(L)], [ACCOUNT[m["account"]]["colour"].lower() for m in ROWS])
    s.expect("account-mark-label-per-row", lambda: [r["mark_label"] for r in s.ctl.rows(L)], [ACCOUNT[m["account"]]["name"] for m in ROWS])   # the label the mark is read out with: the account's name (PhoneAccounts.swift markLabel)
    ok = [k for k, m in enumerate(ROWS) if m["account"] != T["refused_account"]]
    a, b = ok[0], ok[1] - 1                      # the second index shifts up once the first row has left
    arch, snz = ROWS[ok[0]]["id"], ROWS[ok[1]]["id"]
    left = (1 if ROWS[ok[0]]["unread"] else 0) + (1 if ROWS[ok[1]]["unread"] else 0)
    s.click("iphone-inbox.list.row.%d.swipe-archive" % a)
    s.expect("archived-row-leaves", lambda: arch in ids(), False)       # read before the second swipe: that swipe lands on the row's place and would hide a row that stayed
    s.click("iphone-inbox.list.row.%d.swipe-snooze" % b)
    s.expect("snoozed-row-leaves", lambda: snz in ids(), False)
    s.expect("neighbours-stay", lambda: ids(), [m["id"] for m in ROWS if m["id"] not in (arch, snz)])
    s.expect("archived-in-core", lambda: s.ctl.core("message/%s/archived" % arch), True)
    s.expect("snoozed-in-core", lambda: s.ctl.core("message/%s/snoozed" % snz), True)
    s.expect("unread-count-drops", lambda: s.ctl.uistate()["counts"]["primary"], max(0, served_unread("primary", FX["tabs"][0]["unread"]) - left))
    s.expect("undo-control-shows", lambda: len(s.ctl.rows("iphone-inbox.undo")), 1)
    s.expect("two-undo-entries", lambda: s.ctl.core("undo_entry/count"), 2)
    s.click("iphone-inbox.undo.button")
    s.expect("undo-brings-the-snoozed-row-back", lambda: ids(), [m["id"] for m in ROWS if m["id"] != arch])
    s.expect("undo-consumed-an-entry", lambda: s.ctl.core("undo_entry/consumed_last"), True)
    refused = [m for m in ROWS if m["account"] == T["refused_account"]][0]
    s.click("iphone-inbox.list.row.%d.swipe-archive" % [m["id"] for m in ROWS if m["id"] != arch].index(refused["id"]))
    s.expect("refused-row-comes-back", lambda: refused["id"] in ids(), True)
    s.expect("refusal-names-the-account", lambda: ACCOUNT[refused["account"]]["name"] in s.ctl.rows("iphone-inbox.failure")[0]["text"], True)
    s.expect("refused-row-not-archived-in-core", lambda: s.ctl.core("message/%s/archived" % refused["id"]), False)


# MUTANTS: per check, the mutation of the app that must turn it red.
# J7.3/starts-on-inbox | the app opens on another screen
# J7.3/rows-in-fixture-order | pinned rows are not on top or the list is sorted by time
# J7.3/sender-subject-per-row | a row shows the sender of its neighbour
# J7.3/unread-flags | every row looks read
# J7.3/account-mark-colour-per-row | every row carries the same account colour
# J7.3/account-mark-label-per-row | a row's mark carries no account name or another account's name
# J7.3/archived-row-leaves | the archive swipe leaves the row
# J7.3/snoozed-row-leaves | the snooze swipe leaves the row
# J7.3/neighbours-stay | a swipe removes a neighbouring row too
# J7.3/archived-in-core | the row is hidden but not archived
# J7.3/snoozed-in-core | the row is hidden but not snoozed
# J7.3/unread-count-drops | the unread count ignores the swipes
# J7.3/undo-control-shows | no undo control
# J7.3/two-undo-entries | a swipe writes no undo entry
# J7.3/undo-brings-the-snoozed-row-back | undo restores nothing or the wrong row
# J7.3/undo-consumed-an-entry | undo does not consume its entry
# J7.3/refused-row-comes-back | a refused archive drops the row anyway
# J7.3/refusal-names-the-account | the refusal shows no reason
# J7.3/refused-row-not-archived-in-core | the refused row is archived locally


# ---------------------------------------------------------------- the selftest: a stand-in socket that serves the phone inbox as the brief says
# Written from docs/JOURNEYS.md J7.3, fixtures/tests-j7.json and the board defaults (swipe right archives, left snoozes), never from the app's reply.
# One mutant per declared check label (the MUTANTS table above); a mutant is a behaviour a wrong build would have. checks/parity/par-inboxphone.py
# loads this model too (its swipe settings and refresh stamp are served here).
class NoControl(Unreadable):
    """A click on a control the model does not have: the real socket answers ok:false."""


class InboxModel:
    LIST = "iphone-inbox.list"
    BASE_UNREAD = served_unread("primary", FX["tabs"][0]["unread"])

    def __init__(self, mut=()):
        self.mut = set(mut)
        self.gone, self.archived, self.snoozed, self.pinned = [], set(), set(), set()    # gone: row ids off the list, in the order they left
        self.undo, self.pending, self.consumed, self.failure, self.stamp = [], [], False, None, 1000     # pending: what the undo button would reverse
        self.leading = "pin" if "par-inboxphone-swipe/default-leading-archives" in self.mut else "archive"
        self.trailing = "archive" if "par-inboxphone-swipe/default-trailing-snoozes" in self.mut else "snooze"
        self.order = list(ROWS)
        if "J7.3/rows-in-fixture-order" in self.mut:
            self.order = sorted(ROWS, key=lambda m: m["subject"])

    def has(self, label):
        return label in self.mut

    def visible(self):
        return [m for m in self.order if m["id"] not in self.gone]

    def send(self, verb, *args):
        raise Unreadable("%s %s: not modelled" % (verb, args))

    def key(self, chord):
        raise Unreadable("key %s: not modelled" % chord)

    def type(self, text):
        raise Unreadable("type %s: not modelled" % text)

    def close(self):
        pass

    def uistate(self):
        left = sum(1 for m in ROWS if m["id"] in self.gone and m["unread"])
        n = self.BASE_UNREAD if self.has("J7.3/unread-count-drops") else max(0, self.BASE_UNREAD - left)
        return {"surface": "iphone-thread" if self.has("J7.3/starts-on-inbox") else "iphone-inbox", "counts": {"primary": n}}

    def core(self, path):
        parts = path.split("/")
        if parts[0] == "message" and parts[2] == "archived":
            return parts[1] in self.archived
        if parts[0] == "message" and parts[2] == "snoozed":
            return parts[1] in self.snoozed
        if parts[0] == "pin":
            return parts[1] in self.pinned
        if path == "undo_entry/count":
            return len(self.undo)
        if path == "undo_entry/consumed_last":
            return self.consumed
        raise Unreadable("core %s: not modelled" % path)

    def rows(self, ax):
        if ax == self.LIST:
            if self.has("par-inboxphone-refresh/rows-before-refresh"):
                return [{"ax": ax}]
            vis = self.visible()
            out = []
            for i, m in enumerate(vis):
                nb = vis[(i + 1) % len(vis)]
                colour = "#123456" if self.has("J7.3/account-mark-colour-per-row") else ACCOUNT[m["account"]]["colour"]
                lead = "archive" if self.has("par-inboxphone-swipe/rows-follow-the-pick") else self.leading
                out.append({"ax": "%s.row.%d" % (ax, i), "id": m["id"], "sender": nb["sender"] if self.has("J7.3/sender-subject-per-row") else m["sender"],
                            "subject": m["subject"], "unread": False if self.has("J7.3/unread-flags") else m["unread"], "mark_colour": colour,
                            "mark_label": "" if self.has("J7.3/account-mark-label-per-row") else ACCOUNT[m["account"]]["name"],
                            "swipe_leading": lead, "swipe_trailing": self.trailing})
            if "par-inboxphone-refresh/refresh-keeps-the-rows" in self.mut and self.stamp > 1000:
                out.reverse()
            return out
        if ax == "iphone-inbox.undo":
            return [] if self.has("J7.3/undo-control-shows") or not self.pending else [{"ax": ax, "count": len(self.pending)}]
        if ax == "iphone-inbox.failure":
            return [{"ax": ax, "text": self.failure}] if self.failure else []
        if ax == "iphone-inbox.swipe-settings":
            return [{"ax": ax, "leading": self.leading, "trailing": self.trailing}]
        if ax == "iphone-inbox.refresh":
            return [{"ax": ax}] if self.has("par-inboxphone-refresh/stamp-before") else [{"ax": ax, "refreshed_at": self.stamp}]
        raise Unreadable("rows %s: not modelled" % ax)

    def _leave(self, mid, kind):
        refused = next(m for m in ROWS if m["id"] == mid)["account"] == T["refused_account"]
        if kind == "archive" and refused:
            name = ACCOUNT[T["refused_account"]]["name"]
            self.failure = "Archive was refused" if self.has("J7.3/refusal-names-the-account") else "%s: the mail server refused it" % name
            if self.has("J7.3/refused-row-comes-back"):
                self.gone.append(mid)
            if self.has("J7.3/refused-row-not-archived-in-core"):
                self.archived.add(mid)
            return
        if kind == "pin":
            self.pinned.add(mid)
            if self.has("par-inboxphone-swipe/pinned-row-stays"):
                self.gone.append(mid)
            return
        leave_key = "J7.3/archived-row-leaves" if kind == "archive" else "J7.3/snoozed-row-leaves"
        if not self.has(leave_key):
            self.gone.append(mid)
        if self.has("J7.3/neighbours-stay"):
            vis = self.visible()
            if vis:
                self.gone.append(vis[0]["id"] if vis[0]["id"] != mid else vis[-1]["id"])
        if not self.has("J7.3/%s-in-core" % ("archived" if kind == "archive" else "snoozed")):
            (self.archived if kind == "archive" else self.snoozed).add(mid)
        self.pending.append((kind, mid))
        if not self.has("J7.3/two-undo-entries"):
            self.undo.append((kind, mid))

    def click(self, ax):
        r = ax[len("iphone-inbox."):]
        if r.startswith("list.row."):
            i, act = r[len("list.row."):].split(".", 1)
            vis = self.visible()
            mid = vis[int(i)]["id"]
            kind = {"swipe-archive": "archive", "swipe-snooze": "snooze",
                    "swipe-leading": "archive" if self.has("par-inboxphone-swipe/leading-swipe-pins") else self.leading,
                    "swipe-trailing": self.trailing}.get(act)
            if kind is None:
                raise NoControl("no control %s" % ax)
            self.failure = None
            return self._leave(mid, kind)
        if r == "undo.button":
            if self.has("J7.3/undo-control-shows") or not self.pending:
                raise Unreadable("click %s: no control" % ax)
            kind, mid = self.pending.pop()
            if self.undo:
                self.undo.pop()
            self.consumed = not self.has("J7.3/undo-consumed-an-entry")
            if not self.has("J7.3/undo-brings-the-snoozed-row-back") and mid in self.gone:
                self.gone.remove(mid)
            (self.archived if kind == "archive" else self.snoozed).discard(mid)
            return
        if r.startswith("swipe-settings.leading."):
            pick = r.rsplit(".", 1)[1]
            if not self.has("par-inboxphone-swipe/pick-is-kept") and not (pick == "archive" and self.has("par-inboxphone-swipe/pick-restored")):
                self.leading = pick
            return
        if r == "list.refresh":
            if not self.has("par-inboxphone-refresh/refresh-reads-again"):
                self.stamp += 1
            return
        raise NoControl("no control %s" % ax)


def model_run(mutants=(), model_cls=None, drive_fn=None, jid=None):
    """The journey on the stand-in: (labels failed, labels not judged, labels run, checks)."""
    j = Journey(jid or ID, (model_cls or InboxModel)(mutants))
    (drive_fn or drive)(j)
    cs = j.checks()
    return {c.label for c in cs if c.state == "fail"}, {c.label for c in cs if c.state == "unjudged"}, {c.label for c in cs}, cs


def selftest_run(path, model_cls=None, drive_fn=None, jid=None, prefix=None):
    """Clean model green on every case; every declared mutant red on its own case. Prints the clean cases, the table and the totals line."""
    labels = [l for l in declared_mutants(path) if prefix is None or l.startswith(prefix)]
    base_f, base_u, base_run, base_cs = model_run((), model_cls, drive_fn, jid)
    rows, missed = [], []
    for label in labels:
        f, _, _, _ = model_run((label,), model_cls, drive_fn, jid)
        rows.append((label, label in f))
        if label not in f:
            missed.append(label)
    stray = sorted(base_run - set(labels))
    from lib.harness import print_model_cases
    print_model_cases(base_cs)
    print("clean model: failed=%s unjudged=%s; labels without a mutant=%s" % (sorted(base_f), sorted(base_u), stray))
    print("| check | mutant (declared) | seen red |")
    print("|---|---|---|")
    for label, red in rows:
        print("| %s | the model carries this mutant | %s |" % (label, "RED on this case" if red else "NOT RED"))
    bad = len(base_f) + len(base_u) + len(missed) + len(stray)
    print("%d mutants, %d not seen red; %d clean failures, %d clean not judged, %d cases without a mutant" % (
        len(rows), len(missed), len(base_f), len(base_u), len(stray)))
    return 0 if bad == 0 else 1


def main():
    path = os.path.abspath(__file__)
    if "--selftest" in sys.argv:
        return selftest_run(path, prefix="J7.3/")
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    rows = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# J") and " | " in l]
    print_cases(r)
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
