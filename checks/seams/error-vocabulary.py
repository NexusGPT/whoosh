#!/usr/bin/env python3
"""Seam con-seams-s1/error-vocabulary: one refusal, one set of words. A refused action names the
ACCOUNT by its name and the REASON in the same words on every surface (contract ActionFailed
{account_name, reason_words, retryable}; C-J1.2 "server refuses: row returns, line names account and
reason"). The failing account and its words come from the fixture (states.sync.failing_account, the
account's status); the FixtureCore fails acc-18 (docs/CONTROL-SOCKET.md).

    sh scripts/launch-one.sh <bundle> -- --fixture-core python3 checks/seams/error-vocabulary.py
"""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from seamlib import fixture, pick, nj, NJ, equal, contains_words, run  # noqa: E402
sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "journeys"))


def expected():
    fx = fixture()
    acc_id = fx["states"]["sync"]["failing_account"]
    acc = next(a for a in fx["accounts"] if a["id"] == acc_id)
    phrase = acc["status"]
    for line in (fx["states"]["sync"]["failing_line"], fx["states"]["error"]["line"]):
        assert contains_words(line, acc["name"]) and contains_words(line, phrase), "fixture line breaks its own vocabulary: %r" % line
    target = next(m["id"] for m in fx["messages"] if m["account"] == acc_id)
    return {"acc": acc_id, "name": acc["name"], "phrase": phrase, "target": target}


def _text(rows, field="text"):
    if nj(rows):
        return rows
    return rows[0].get(field, NJ("no %s" % field)) if rows else NJ("empty")


def observe(c, exp):
    t = {"sidebar-account": pick(c.rows("sidebar_accounts"), "id", exp["acc"], "line")}
    # The target row is addressed by its ID: its ax id is read from the list by the row's id, and the click focuses it. Counting J presses
    # from the first row (main55) landed on the flow's rejected row, which the core archives without a refusal, so no failure was ever recorded.
    ax = pick(c.rows("list"), "id", exp["target"], "ax")
    focused = ax if nj(ax) else (c.act("click", ax) and c.ui("focus_message"))
    if nj(focused) or focused != exp["target"]:
        refusal = NJ("the target row %s is not focused (focus %r): no archive is made, so no refusal can be read" % (exp["target"], focused))
    else:
        refusal = None
        c.act("key", "E")  # archive a row of the failing account: the core refuses it
    t["status-line"] = _text(c.rows("status"))
    lf = refusal or c.ui("last_failure")
    rows = c.rows("list")
    o = {"text": t, "focused": focused,
         "lf_account": lf if nj(lf) else (lf or {}).get("account_name"),
         "lf_reason": lf if nj(lf) else (lf or {}).get("reason_words"),
         "row_returns": rows if nj(rows) else (exp["target"] in [r.get("id") for r in rows]),
         "core_archived": c.core("message/%s/archived" % exp["target"])}
    if c.act("click", "mac-main.status.link"):  # C-J1.1: the line links to Settings: Accounts
        # main41: the Settings: Accounts row shows the account's name beside its `error` words
        rows = c.rows("settings_accounts")
        nm, er = pick(rows, "id", exp["acc"], "name"), pick(rows, "id", exp["acc"], "error")
        o["text"]["settings-accounts"] = nm if nj(nm) else (er if nj(er) else "%s %s" % (nm, er))
    else:
        o["text"]["settings-accounts"] = NJ("click mac-main.status.link refused")
    return o


def judge(o, exp):
    out = []
    for s, txt in o["text"].items():
        if nj(txt):
            out += [("names-account@%s" % s, None, txt.why), ("reason-words@%s" % s, None, txt.why)]
        else:
            out.append(("names-account@%s" % s, contains_words(txt, exp["name"]), "text %r" % txt))
            out.append(("reason-words@%s" % s, contains_words(txt, exp["phrase"]), "text %r, phrase %r" % (txt, exp["phrase"])))
    out += equal("target-focused", exp["target"], {"uistate": o["focused"]})     # the address of the archive: without it a green read below could be another row's
    out += equal("last-failure-account", exp["name"], {"uistate": o["lf_account"]})
    r = o["lf_reason"]
    out.append(("last-failure-reason@uistate", None if nj(r) else contains_words(r or "", exp["phrase"]),
                r.why if nj(r) else "reason_words %r" % r))
    out += equal("row-returns", True, {"list": o["row_returns"]})
    out += equal("not-archived", False, {"core": o["core_archived"]})
    return out


def oracle(exp):
    fx = fixture()
    return {"text": {"sidebar-account": fx["states"]["sync"]["failing_line"], "status-line": fx["states"]["error"]["line"],
                     "settings-accounts": fx["states"]["sync"]["failing_line"]},
            "focused": exp["target"], "lf_account": exp["name"], "lf_reason": exp["phrase"], "row_returns": True, "core_archived": False}


def _m(**kv):
    def f(o):
        for k, v in kv.items():
            if k.startswith("text_"):
                o["text"][k[5:].replace("_", "-")] = v
            else:
                o[k] = v
        return o
    return f


# MUTANTS
# wrong-row        | the click on the target row focuses another row (the archive then hits it)         | target-focused
# raw-server-words | the status line shows the server's raw words, not the account's reason words   | reason-words@status-line
# account-id       | ActionFailed carries the account id, not its name                               | last-failure-account
# act-deleted      | the archive act is removed: no failure, the sync line stays                     | last-failure-account
# optimistic-kept  | the refused archive is not rolled back                                          | row-returns
# settings-other   | Settings: Accounts words the failure differently ("authentication error")      | reason-words@settings-accounts
MUTANTS = {
    "wrong-row": ("click focuses another row", "target-focused", _m(focused="flow-rejected-1")),
    "raw-server-words": ("raw server text", "reason-words@status-line",
                         _m(text_status_line="Provanta RFP: NO [AUTHENTICATIONFAILED] Invalid credentials")),
    "account-id": ("id not name", "last-failure-account", _m(lf_account="acc-18")),
    "act-deleted": ("no refused act", "last-failure-account",
                    _m(lf_account=None, lf_reason=None, text_status_line=fixture()["states"]["sync"]["line"])),
    "optimistic-kept": ("no rollback", "row-returns", _m(row_returns=False, core_archived=True)),
    "settings-other": ("second vocabulary", "reason-words@settings-accounts",
                       _m(text_settings_accounts="Provanta RFP: authentication error")),
}

# ---------------------------------------------------------------- --selftest: observe() driven against a stand-in socket
# The judge's mutants above prove the JUDGE; these prove the DRIVE: observe() runs against a model of the contract (written from the fixture and
# CONTROL-SOCKET.md, never from the app's reply), clean it must be green on every label, and with one defect of the app at a time the label
# that defect breaks must go red. "click-no-focus" is main55's own defect shape: the archive lands on another row.
import json as _json
import seamlib as _seamlib


class ModelCtl(_seamlib.Ctl):
    def __init__(self, defect=None):
        self.defect, self.path, self.s, self.buf, self.log = defect, "model", True, b"", []
        fx = fixture()
        self.fx = fx
        self.acc = fx["states"]["sync"]["failing_account"]
        self.name = next(a["name"] for a in fx["accounts"] if a["id"] == self.acc)
        self.phrase = next(a["status"] for a in fx["accounts"] if a["id"] == self.acc)
        flow = [r["id"] for r in fx.get("flow_example", []) if r["step"] == "rejected"]
        if defect == "file-order":                                          # the old model: the rejected rows in the file's order, after two messages
            ids = [m["id"] for m in fx["messages"]]
            self.ids = ids[:2] + flow + ids[2:]
        else:                                                               # as the app lists them: the one definition, journeys/lib/fixture.py ROWS
            from lib.fixture import ROWS
            self.ids = [r["id"] for r in ROWS]
        self.account_of = {m["id"]: m["account"] for m in fx["messages"]}
        self.account_of.update({r["id"]: r["account"] for r in fx.get("flow_example", [])})
        self.focus, self.failed, self.archived = self.ids[0], False, set()

    @property
    def live(self):
        return True

    def call(self, verb, *args):
        d = self.defect
        if verb == "click" and args[0].startswith("mac-main.list.row."):
            if d != "click-no-focus":
                self.focus = [i for i in self.ids if i not in self.archived][int(args[0].rsplit(".", 1)[1])]
            return {"ok": True}
        if verb == "key" and args[0] == "E":
            if d == "key-dead":
                return {"ok": True}
            if self.account_of.get(self.focus) == self.acc:
                self.failed = True
                return {"ok": False, "error": "the key E does nothing on mac-main"}
            self.archived.add(self.focus)
            return {"ok": True}
        if verb == "click" and args[0] == "mac-main.status.link":
            return {"ok": True}
        if verb == "uistate":
            lf = {"account_name": self.name, "reason_words": self.phrase, "retryable": True} if self.failed else None
            return {"ok": True, "state": {"focus_message": self.focus, "last_failure": lf}}
        if verb == "core" and args[:1] == ("state",):
            return {"ok": True, "value": args[1] in {"message/%s/archived" % i for i in self.archived}}
        if verb == "rows":
            ax, st = args[0], self.fx["states"]
            line = st["error"]["line"] if self.failed else st["sync"]["line"]
            if ax == "mac-main.list":
                return {"ok": True, "rows": [{"ax": "mac-main.list.row.%d" % n, "id": i} for n, i in enumerate(x for x in self.ids if x not in self.archived)]}
            if ax == "mac-main.sidebar.accounts":
                return {"ok": True, "rows": [{"id": self.acc, "name": self.name, "line": st["sync"]["failing_line"]}]}
            if ax == "mac-main.status":
                return {"ok": True, "rows": [{"text": line}]}
            if ax == "mac-settings-accounts.accounts":
                return {"ok": True, "rows": [{"id": self.acc, "name": self.name, "error": self.phrase}]}
        return {"ok": False, "error": "%s %s: not modelled" % (verb, args)}


DEFECTS = {
    "click-no-focus": ("a click on a row does not focus it: the archive lands on the focused first row", "target-focused"),
    "key-dead": ("the E key does nothing: no refusal is ever recorded", "last-failure-account"),
}


def selftest():
    import io, contextlib
    bad = []
    def run_model(defect):
        _seamlib.Ctl = lambda: ModelCtl(defect)
        buf = io.StringIO()
        # the library's own `--selftest` (11 Nov) reads no app at all; this self-test DRIVES the model, so the word is taken out of the arguments for the call
        # (with it, the run printed no label and every defect below read STAYED GREEN)
        argv, sys.argv = sys.argv, [a for a in sys.argv if a != "--selftest"]
        try:
            with contextlib.redirect_stdout(buf):
                rc = _seamlib.run("error-vocabulary", expected, observe, judge, oracle, MUTANTS)
        finally:
            sys.argv = argv
        live = [l for l in buf.getvalue().split("== error-vocabulary  MUTANTS")[0].splitlines() if l.startswith("  ")]
        return rc, live
    rc, live = run_model(None)
    red = [l for l in live if l.strip().startswith(("FAIL", "NOT JUDGED"))]
    print("clean model: exit %d, %d labels, not green: %s" % (rc, len(live), red))
    if rc != 0 or red:
        bad.append("clean")
    print("| defect of the app | label that must go red | seen |")
    print("|---|---|---|")
    for d, (what, must) in DEFECTS.items():
        _, live = run_model(d)
        hit = [l.strip() for l in live if l.strip().startswith("FAIL") and ("%s@" % must) in l]
        print("| %s | %s | %s |" % (what, must, "RED: " + hit[0] if hit else "STAYED GREEN"))
        if not hit:
            bad.append(d)
    # the model's list order is the app's order, read from journeys/lib/fixture.py ROWS (the one place that states it); the file-order mutant must differ
    from lib.fixture import ROWS as _ROWS
    want = [r["id"] for r in _ROWS]
    for d in (None, "file-order"):
        got = ModelCtl(d).ids
        seen = "same as the app's order" if got == want else "DIFFERENT from the app's order"
        print("| model list order, defect %s | the app's order (lib.fixture ROWS) | %s |" % (d, seen))
        if (got == want) != (d is None):
            bad.append("order-" + str(d))
    print("selftest: %d problems" % len(bad))
    return 1 if bad else 0


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        sys.exit(selftest())
    sys.exit(run("error-vocabulary", expected, observe, judge, oracle, MUTANTS))
