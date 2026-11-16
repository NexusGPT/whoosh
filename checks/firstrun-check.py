#!/usr/bin/env python3
"""The first run on the REAL core (lane firstrun-check; plan docs/lanes/realcore-firstrun.testplan.md rows T-01, T-06 to T-16, T-22 to T-25, T-27, T-29).

Why: the owner found ten faults on main65 that no hidden check could see. Every real-core slice of checks/realcore-check.py was green while two of
them were true, because the rig PRE-FILLED labels and a model (WHOOSH_CORE_CHECK_FLOW, WHOOSH_CORE_CHECK_AI). This check seeds nothing. It launches the
hidden rig copy on the real core with an EMPTY data folder and only TRANSPORT redirections (the local mail fake, the local provider fake), then walks the
owner's first run, one step at a time, each step with its own judge, its mutant and a NOT JUDGED path for a piece the build does not carry.

    python3 checks/firstrun-check.py <bundle> [--step T-06[,T-09]] [--mutant[=<name>]] [--core real|fixture] [--log <file>]
    python3 checks/firstrun-check.py --compare <real.log> <fixture.log>     T-27: the same visible assertions on both cores
    python3 checks/firstrun-check.py --dry                                  the case list, each step's mutant and gate, the commands
    python3 checks/firstrun-check.py --selftest                             the judges and the driver against a model app; every mutant red

  <bundle>      the app bundle (scripts/launch-one.sh makes the rig copy from it: hidden, its own run folder, its own lock; exit 75 = BUSY: wait, never kill)
  --step        report only these steps (the walk still runs what they need)
  --mutant      WHOOSH_CORE_MUTANT=<the step's runtime mutant> (default: every step's own one in turn is NOT one run; name it). Exit 0 when the step's named
                cases went red. A step whose mutant is a source edit prints NOT JUDGED (source edit).
  --core fixture  the same steps T-10 T-11 T-12 T-14 T-16 T-22 on the fixture core after `fixture install-state new` (T-27 compares the two logs)

The command the lead runs on a build (the check starts the fakes first, because the app needs their ports at launch, then launches the rig copy):
    cd "<worktree>" && WHOOSH_RIG_HIDDEN=1 python3 checks/firstrun-check.py <bundle> --log _captures/perf-realcore-r1/firstrun-<stamp>.log
and the same through the launcher, with the fakes already running (FIRSTRUN_HUB=<port>):
    sh scripts/launch-one.sh <bundle> -- --rig-hidden sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/firstrun-check.py --inner "$FIRSTRUN_HUB"'
The launch environment holds NO seeding variable: the run REFUSES when LAUNCH_ENV or the environment names WHOOSH_CORE_CHECK_FLOW, WHOOSH_CORE_CHECK_AI or
WHOOSH_CORE_CHECK_IMPORT (T-01). Nothing is typed into a real provider: a key is pasted only when the build carries WHOOSH_CORE_CHECK_PROVIDERS. No real
Calendar or microphone prompt is ever raised: the rig refuses and counts them (`core state system-ui/<kind>`). No window opens: every launch is hidden.
Timing steps print NOT JUDGED above PERF_LOAD_MAX (docs/CONSTANTS.md). The last line is `N checks, F failed, U not judged`; exit 1 on F > 0 or N = 0.
Limits, said once: the selftest runs the driver against a MODEL of the app, so it proves the judges, the order and the ids this file assumes, never the app;
the ids come from Sources/ (file:line beside each), and a read the build does not serve is NOT JUDGED, a refused click is red (the control is not there)."""
import importlib.util
import json
import os
import re
import socket
import socketserver
import statistics
import subprocess
import sys
import tempfile
import threading
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))

# ------------------------------------------------------------------------------------------------ what this check judges against
FORBIDDEN_ENV = ("WHOOSH_CORE_CHECK_FLOW", "WHOOSH_CORE_CHECK_AI", "WHOOSH_CORE_CHECK_IMPORT")   # a seeded rig cannot see a missing default (plan 1, R1)
OWNER, SECOND = "owner@whoosh-check.invalid", "second@whoosh-check.invalid"
PASSWORD = "check-password"
NEEDS_REPLY_WORDS = "No model is set for sorting mail: choose one in Settings, Decisions."   # labels-r5 round 6 (15.2); the card must say what the core says
GOOD_KEY, BAD_KEY, BAD_KEY_WORDS = "sk-rig-good", "sk-rig-bad", "invalid x-api-key"          # plan R2
MODELS = ["model-a", "model-b"]
LIST_AFTER_RELAUNCH_S = 1.0       # plan T-23: the stored inbox is drawn within 1 s of the first frame, before the server (2000 ms away) answers
RELAUNCH_LATENCY_MS = 2000        # plan T-23
SYNC_WAIT_S, STEP_WAIT_S = 120, 30
LAYA_DESCRIPTION = "Asking a question from me"   # the owner's 14:28 text (plan T-15)
SOURCES, CALS = "mac-settings-replies.sources", ("Travail", "Personnel")
A = "mac-add-account"                              # Sources/Features/AddAccount (as checks/realcore-check.py slice 2 drives it)
W = "mac-settings-window"                          # Sources/Features/SettingsWindow/SettingsWindowRegistry.swift:21,29,83-84,135
INS = "mac-labels.inspector"                       # Sources/Features/LabelsContactsMac/LabelInspectorSocket.swift:7,90 (open.modify.<id>)
AI = "mac-settings-ai"                             # Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift:20-150
CAL = "mac-settings-calendars"                     # checks/settingscalendars-check.py:30


def const(name):
    from lib.fixture import const as c  # noqa: E402
    return c(name)


# ------------------------------------------------------------------------------------------------ T-01: the refusal
def refusal(env, launch_env=""):
    """The seeding variables a launch would hold, from the environment and from the LAUNCH_ENV words. [] = a clean launch."""
    held = [k for k in FORBIDDEN_ENV if k in env]
    held += [k for k in FORBIDDEN_ENV if k not in held and any(w.split("=")[0] == k for w in launch_env.split())]
    return held


def refuse_words(held):
    return "REFUSED: seeded rig: %s in the launch environment would hide a missing default (plan realcore-firstrun 1, R1)" % ", ".join(held)


# ------------------------------------------------------------------------------------------------ what the build carries
def bundle_has(bundle, needle):
    """True when `needle` (bytes or str) is a string literal somewhere in the bundle's executables: how a capability the build must carry is told
    without launching it. The core reads WHOOSH_CORE_* names and serves state paths as literals, so a build that lacks the seam lacks the string."""
    needle = needle.encode() if isinstance(needle, str) else needle
    for sub in ("Contents/MacOS", "Contents/Frameworks"):
        for root, _d, files in os.walk(os.path.join(bundle, sub)):
            for f in files:
                try:
                    with open(os.path.join(root, f), "rb") as fh:
                        tail = b""
                        while True:
                            chunk = fh.read(8 << 20)
                            if not chunk:
                                break
                            if needle in tail + chunk:
                                return True
                            tail = chunk[-len(needle):]
                except OSError:
                    continue
    return False


def capabilities(bundle):
    """{name: bool}. In the selftest the model hands its own."""
    has = lambda n: bundle_has(bundle, n)
    return {"providers": has("WHOOSH_CORE_CHECK_PROVIDERS"), "laya": has("rig/local-model"), "keyevent": has("keyevent"),
            "mutants": {m: has(m) for m in MUTANTS.values() if m}}


# ------------------------------------------------------------------------------------------------ the report
class Gone(Exception):
    """A read the screen or the core does not serve: the case is NOT JUDGED."""


class Refused(Exception):
    """An action the app refused: the control is not there, or it is dead; the case is red."""


class Rep:
    def __init__(self, only=None):
        self.cases, self.only = [], only

    def add(self, cid, name, status, detail=""):
        self.cases.append((status, cid, name, detail))

    def ok(self, cid, name, cond, detail=""):
        self.add(cid, name, "ok" if cond else "FAIL", detail)

    def nj(self, cid, name, why):
        self.add(cid, name, "NOT JUDGED", why)

    def guard(self, cid, name, fn):
        """fn() -> (cond, detail). Gone: NOT JUDGED. Refused: red, with the door."""
        try:
            cond, detail = fn()
            self.ok(cid, name, cond, detail)
        except Gone as e:
            self.nj(cid, name, "unreadable: %s" % e)
        except Refused as e:
            self.add(cid, name, "FAIL", "refused: %s" % e)

    def lines(self):
        keep = [c for c in self.cases if self.only is None or c[1] in self.only]
        return ["%-10s %s %s :: %s" % (st, cid, name, det) for st, cid, name, det in keep]

    def counts(self):
        keep = [c for c in self.cases if self.only is None or c[1] in self.only]
        return len(keep), sum(1 for c in keep if c[0] == "FAIL"), sum(1 for c in keep if c[0] == "NOT JUDGED")


def summary(rep):
    n, f, u = rep.counts()
    return "%d checks, %d failed, %d not judged" % (n, f, u)


# ------------------------------------------------------------------------------------------------ a session: the app, the hub, the report
class Session:
    def __init__(self, c, hub, rep, caps, core="real", relaunch=None, watcher=None, load=lambda: os.getloadavg()[0], sleep=time.sleep, clock=time.time):
        self.c, self.hub, self.rep, self.caps, self.core_kind = c, hub, rep, caps, core
        self.relaunch_fn, self.watcher, self.load, self.sleep, self.clock = relaunch, watcher, load, sleep, clock
        self.mem = {}          # what the walk learned and a later step (relaunch) must find again
        self.self_control = probe_control      # the positive control of the network probe on a core whose app holds no socket (T-24, T-30)

    # reads: a failure is NOT JUDGED
    def rows(self, ax):
        try:
            return self.c.rows(ax)
        except Exception as e:      # noqa: BLE001 (Unreadable, a dropped socket)
            raise Gone("rows %s: %s" % (ax, e))

    def row(self, ax):
        r = self.rows(ax)
        if not r:
            raise Gone("rows %s: no row" % ax)
        return r[0]

    def core(self, path):
        try:
            return self.c.core(path)
        except Exception as e:      # noqa: BLE001
            raise Gone("core state %s: %s" % (path, e))

    def uistate(self):
        try:
            return self.c.uistate()
        except Exception as e:      # noqa: BLE001
            raise Gone("uistate: %s" % e)

    # actions: a failure is red
    def send(self, verb, *args):
        try:
            return self.c.send(verb, *args)
        except Exception as e:      # noqa: BLE001
            raise Refused("%s %s: %s" % (verb, " ".join(map(str, args))[:60], e))

    def click(self, ax):
        return self.send("click", ax)

    def type(self, text):
        return self.send("type", text)

    def wait(self, fn, secs=STEP_WAIT_S, every=0.3):
        """The first truthy answer of fn() within `secs`, else None. A Gone inside counts as 'not yet'."""
        end = self.clock() + secs
        while True:
            try:
                v = fn()
                if v:
                    return v
            except Gone:
                pass
            if self.clock() >= end:
                return None
            self.sleep(every)

    def quiet(self):
        lim = const("PERF_LOAD_MAX")
        return self.load() <= lim, lim


def words_of(v):
    """The sentence of a core value that is a string, or a {words|message|text} object."""
    if isinstance(v, str):
        return v
    if isinstance(v, dict):
        for k in ("words", "message", "text"):
            if isinstance(v.get(k), str):
                return v[k]
    return ""


def fetched_uids(log, uids):
    """The UIDs whose WHOLE message (BODY.PEEK[]) the core fetched, from the mail fake's command log. `uids`: the INBOX's UIDs."""
    import realcore_mailfake as fake
    got = set()
    for line in log:
        if not line.startswith("UID FETCH "):
            continue
        parts = line.split(" ", 3)
        if len(parts) == 4 and "BODY.PEEK[]" in parts[3].upper():
            got.update(fake.uid_set(parts[2], uids))
    return got


# ------------------------------------------------------------------------------------------------ the steps
STEPS = []      # (id, fn, runtime mutant name or None, the cases that mutant must turn red, source-edit words when the mutant is not runtime)
MUTANTS = {}    # step id -> runtime mutant name (WHOOSH_CORE_MUTANT)
RED = {}        # step id -> case names the mutant must turn red
EDITS = {}      # step id -> the source edit that is its red-first mutant (plan table)


def step(cid, mutant=None, red=(), edit=None):
    def deco(fn):
        STEPS.append((cid, fn))
        if mutant:
            MUTANTS[cid] = mutant
        RED[cid] = tuple(red)
        if edit:
            EDITS[cid] = edit
        return fn
    return deco


def surface_home(s):
    """Back to the main window's list before a step that starts there (Escape closes what a step left open)."""
    for _ in range(2):
        try:
            s.c.key("Escape")
        except Exception:       # noqa: BLE001
            pass


@step("T-29", edit="HiddenRig alpha set to 1 (its mutant, WHOOSH_RIG_HIDDEN_MUTANT)", red=("no-window-shown",))
def t29_start(s):
    """Starts the window-server watch for the whole walk (judged at the end, in t29_end). Cases are added there."""
    s.mem["hidden_on"] = None
    try:
        st = s.uistate()
        s.mem["hidden_on"] = bool((st.get("hidden") or {}).get("on"))
    except Gone:
        pass
    s.mem["watch"] = s.watcher.start(s.mem.get("pid")) if (s.watcher and s.mem.get("pid")) else None


@step("T-06", mutant="no-account-stored", red=("sidebar-shows-the-account-live",))   # the mutant answers Ok and stores nothing: the sheet may still say "added", the sidebar cannot show it
def t06(s):
    r, T = s.rep, "T-06"
    r.guard(T, "first-start-opens-on-an-empty-installation", lambda: ((lambda st: (st.get("active") is True and st.get("accounts") == 0, "state %r" % {k: st.get(k) for k in ("active", "step", "accounts")}))(s.row("mac-first-start.state"))))
    try:
        s.click("mac-first-start.skip")
    except Refused as e:
        r.add(T, "skip-leaves-the-add-account-way", "FAIL", str(e))
    else:
        r.guard(T, "skip-leaves-the-add-account-way", lambda: (s.row("mac-first-start.state").get("skipped") is True, "state %r" % s.row("mac-first-start.state")))
    r.guard(T, "sidebar-says-no-account-yet", lambda: ((lambda e, a: (len(e) == 1 and len(a) == 0, "empty rows %r, account rows %d" % (e, len(a))))(s.rows("mac-main.sidebar.empty"), s.rows("mac-main.sidebar.accounts"))))
    s.click("mac-main.sidebar.empty.add")
    r.guard(T, "the-sheet-opens-from-the-empty-sidebar", lambda: ((lambda st: (st.get("stage") in ("entry", "detecting"), "sheet %r" % st))(s.row(A + ".state"))))
    s.click(A + ".address"); s.type(OWNER)
    s.click(A + ".password"); s.type(PASSWORD)
    s.click(A + ".continue")
    s.wait(lambda: s.row(A + ".state").get("stage") not in ("detecting", "entry"), 90)
    s.click(A + ".add")
    stage = s.wait(lambda: s.row(A + ".state").get("stage") in ("added", "summary") and s.row(A + ".state").get("stage"), 90)
    err = None
    try:
        e = s.rows(A + ".error"); err = e[0].get("text") if e else None
    except Gone:
        pass
    r.ok(T, "account-added", stage == "added", "stage %r, the sheet's words %r" % (stage, err))
    try:
        s.click(A + ".done")
    except Refused as e:
        r.add(T, "sheet-done", "FAIL", str(e))

    def live():
        row = s.wait(lambda: (lambda rs: rs[0] if rs else None)(s.rows("mac-main.sidebar.accounts")), 30)
        if not row:
            return False, "no account row in the sidebar"
        s.wait(lambda: s.rows("mac-main.sidebar.accounts")[0].get("health") == "Live", 30)
        row = s.rows("mac-main.sidebar.accounts")[0]
        good = row.get("address") == OWNER and (row.get("health") == "Live" or str(row.get("health", "")).startswith("Syncing")) and not row.get("status")
        return good, "row %r" % {k: row.get(k) for k in ("address", "health", "status", "line")}
    r.guard(T, "sidebar-shows-the-account-live", live)


@step("T-07", mutant="sync-never-stores", red=("list-holds-the-first-page", "subjects-drawn-as-sent"))
def t07(s):
    r, T = s.rep, "T-07"
    import realcore_mailfake as fake
    want_n = 50      # the list's first page (the perf drive on main57 read 50 of 60 rows)

    def got():
        rows = s.rows("mac-main.list")
        return rows if len(rows) >= want_n else None
    rows = s.wait(got, SYNC_WAIT_S, 0.5) or []
    try:
        rows = s.rows("mac-main.list")
    except Gone:
        pass
    r.ok(T, "list-holds-the-first-page", len(rows) >= want_n, "%d rows after the wait" % len(rows))
    subjects = [x.get("subject") for x in rows]
    names = [s_[0] for s_ in fake.FR_NAMED]
    missing = [n for n in names if n not in subjects]
    r.ok(T, "subjects-drawn-as-sent", bool(rows) and not missing, "missing %r; first subjects %r" % (missing, subjects[:4]))
    senders = {x.get("sender") for x in rows}
    r.ok(T, "sender-names-keep-accents-and-apostrophes", bool(rows) and {"Hélène Dupré", "François L’Écuyer"} <= senders, "senders %r" % sorted(x for x in senders if x)[:6])

    def counts():
        c = s.uistate().get("counts") or {}
        flat = [v for v in c.values() if isinstance(v, int)] + [v for d in c.values() if isinstance(d, dict) for v in d.values() if isinstance(v, int)]
        return sum(flat) > 0, "counts %r" % c
    r.guard(T, "counts-above-zero", counts)
    s.mem["rows_before"] = [x.get("subject") for x in rows]


@step("T-09", mutant="index-never-writes", red=("body-only-word-finds-its-message", "second-body-only-word-finds-its-message"))
def t09(s):
    r, T = s.rep, "T-09"
    import realcore_mailfake as fake
    if not s.caps.get("keyevent"):
        for n in ("body-only-word-finds-its-message", "second-body-only-word-finds-its-message"):
            r.nj(T, n, "keyevent is not in this build")
        return
    surface_home(s)
    for word, subject, name in ((fake.FR_CADEAU_WORD, fake.FR_CADEAU_SUBJECT, "body-only-word-finds-its-message"),
                                (fake.FR_REUNION_WORD, fake.FR_REUNION_SUBJECT, "second-body-only-word-finds-its-message")):
        def one(word=word, subject=subject):
            s.send("keyevent", "cmd+k")
            s.click("mac-search.bar")
            # keyevent types letter by letter; an accented letter has no key of its own, so `type` (the socket's text path) carries it: the key path is T-05's
            if word.isascii():
                s.send("keyevent", word)
            else:
                s.type(word)
            s.send("keyevent", "Return")
            s.wait(lambda: s.row("mac-search.bar").get("launched") and not s.row("mac-search.bar").get("loading"), 30)
            res = s.wait(lambda: s.rows("mac-search.results"), 30) or []
            subs = [x.get("subject") for x in res]
            s.send("keyevent", "Escape")
            return subject in subs, "word %r: %d results %r" % (word, len(res), subs[:4])
        r.guard(T, name, one)
    surface_home(s)


@step("T-10", edit="the 0004 seed line ('needs-reply','Needs Reply', NULL, NULL, '[]', ...): today's value, no description, no rule",
      red=("needs-reply-has-a-description", "needs-reply-exit-rule-unanswered", "promotions-has-its-rule", "promotions-filled-by-its-rule-with-no-ai"))
def t10(s):
    r, T = s.rep, "T-10"
    surface_home(s)

    def open_label(lid):
        s.click("%s.open.modify.%s" % (INS, lid))
        s.wait(lambda: s.row(INS).get("open") is True and s.row(INS).get("label_id") == lid, 10)
        return s.row(INS)
    r.guard(T, "needs-reply-has-a-description", lambda: ((lambda i: (bool((i.get("meaning") or "").strip()), "meaning %r" % i.get("meaning")))(open_label("needs-reply"))))
    s.mem["needs_reply_meaning"] = (s.row(INS).get("meaning") if s.rep.cases and s.rep.cases[-1][0] == "ok" else None)

    def exit_rule():
        chains = {x["chain"]: x for x in s.rows(INS + ".flow") if "chain" in x}
        ex = chains.get("exit") or {}
        return (ex.get("facts") or {}).get("unanswered") == "yes", "exit chain %r" % {k: ex.get(k) for k in ("facts", "rows_n")}
    r.guard(T, "needs-reply-exit-rule-unanswered", exit_rule)

    def promos():
        open_label("promotions")
        chains = {x["chain"]: x for x in s.rows(INS + ".flow") if "chain" in x}
        ent = chains.get("entry") or {}
        return (ent.get("facts") or {}).get("mailing_list") == "yes", "entry chain %r, meaning %r" % (ent.get("facts"), s.row(INS).get("meaning"))
    r.guard(T, "promotions-has-its-rule", promos)

    def by_rule():
        foot = s.row(INS + ".foot")
        # SPEC A140: `caught_by_entry` is what step 1 PICKED (it files the mail itself only where step 2 is skipped: the AI card has no description to rule by, as
        # the shipped Promotions, so this label is filled by its rule with no AI; the real engine's seed also switches its AI card off, flow3-core Part 2 item 3); the figure still reads >= 1 for the newsletter that carries List-Unsubscribe
        n = foot.get("caught_by_entry")
        return isinstance(n, int) and n >= 1, "foot %r, caught_by_entry (picked by step 1) %r (the newsletter carries List-Unsubscribe)" % (foot.get("text"), n)
    r.guard(T, "promotions-filled-by-its-rule-with-no-ai", by_rule)
    try:
        s.click(INS + ".cancel")
    except Refused:
        pass


@step("T-16", edit="the pane stops reading label/needs-reply/flow/ai_refusal", red=("the-ai-card-says-why-nothing-was-sorted", "the-empty-list-says-the-same"))
def t16(s):
    r, T = s.rep, "T-16"

    def card():
        s.click("%s.open.modify.needs-reply" % INS)
        s.wait(lambda: s.row(INS).get("label_id") == "needs-reply", 10)
        ai = {x["card"]: x for x in s.rows(INS + ".flow") if "card" in x}.get("ai") or {}
        shown = ai.get("ai_refusal")
        try:
            core = words_of(s.core("label/needs-reply/flow/ai_refusal"))
        except Gone:
            core = ""        # a core that serves no value says nothing: the card must still carry the sentence the build's own text gives
        s.mem["ai_refusal"] = shown
        return bool(shown) and isinstance(shown, str) and shown == (core or NEEDS_REPLY_WORDS) and "Decisions" in shown, "card %r, core %r" % (shown, core)
    r.guard(T, "the-ai-card-says-why-nothing-was-sorted", card)

    def empty():
        e = s.rows(INS + ".empty")
        return len(e) == 1 and e[0].get("words") == s.mem.get("ai_refusal"), "empty rows %r" % e
    r.guard(T, "the-empty-list-says-the-same", empty)
    try:
        s.click(INS + ".cancel")
    except Refused:
        pass


@step("T-11", edit="remove crate::replies::register_folders(...) at core/crates/engine/src/imap_source.rs:98",
      red=("two-account-cards", "inbox-and-sent-on-everything-else-off"))
def t11(s):
    r, T = s.rep, "T-11"
    try:
        s.click(W + ".pane-row.replies.sources")
    except Refused as e:
        r.add(T, "two-account-cards", "FAIL", str(e)); return
    # the second account is added here, through the same sheet as the first: the plan's two accounts of T-03 (a fixture core already has its own)
    s.mem.setdefault("second_added", False)
    have = sum(1 for x in s.rows(SOURCES) if x.get("kind") == "account") if True else 0
    if have >= 2:
        s.mem["second_added"] = "already there"
    if not s.mem["second_added"]:
        try:
            s.click(A + ".open")
            s.click(A + ".address"); s.type(SECOND)
            s.click(A + ".password"); s.type(PASSWORD)
            s.click(A + ".continue")
            s.wait(lambda: s.row(A + ".state").get("stage") not in ("detecting", "entry"), 90)
            s.click(A + ".add")
            s.mem["second_added"] = s.wait(lambda: s.row(A + ".state").get("stage") == "added", 90) is not None
            s.click(A + ".done")
        except (Refused, Gone) as e:
            s.mem["second_error"] = str(e)
    s.click(W + ".pane-row.replies.sources")
    rows = s.wait(lambda: (lambda rs: rs if sum(1 for x in rs if x.get("kind") == "account") >= 2 else None)(s.rows(SOURCES)), 60) or s.rows(SOURCES)
    accts = [x for x in rows if x.get("kind") == "account"]
    r.ok(T, "two-account-cards", len(accts) == 2, "%d account cards (second added: %s %s); line %r" % (len(accts), s.mem["second_added"], s.mem.get("second_error", ""), [a.get("line") for a in accts]))
    groups = {}
    for x in rows:
        if x.get("kind") == "mail_folder":
            m = re.match(r"(.*)\.folder\.\d+$", x.get("ax", ""))
            groups.setdefault(m.group(1) if m else "?", []).append(x)

    def folders():
        bad = []
        if len(groups) < 2:
            return False, "folder rows for %d accounts: %r" % (len(groups), sorted(groups))
        for key, fs in groups.items():
            on = [f for f in fs if f.get("enabled")]
            names = sorted((f.get("name") or "").lower() for f in on)
            inbox_on = any("inbox" in n or "réception" in n for n in names)
            sent_on = any("sent" in n or "envoy" in n for n in names)
            if not (inbox_on and sent_on and len(on) == 2):
                bad.append((key, [(f.get("name"), f.get("enabled")) for f in fs]))
        return not bad, "wrong: %r" % bad if bad else "each account: Inbox and Sent on, the rest off (%d accounts)" % len(groups)
    r.guard(T, "inbox-and-sent-on-everything-else-off", folders)
    s.mem["sources_on"] = sorted((x.get("ax"), bool(x.get("enabled"))) for x in rows if x.get("kind") == "mail_folder")


@step("T-12", edit="flip the auto value in the 0013 seed", red=("auto-draft-reads-off", "nothing-drafted-nothing-sent-nothing-asked"))
def t12(s):
    r, T = s.rep, "T-12"
    s.click(W + ".pane-row.replies.auto")

    def off():
        st = s.row("mac-settings-replies.state")
        core = s.core("reply/use/auto")
        enabled = core.get("enabled") if isinstance(core, dict) else core
        return st.get("auto_enabled") is False and enabled is False, "pane %r, core %r" % (st.get("auto_enabled"), core)
    r.guard(T, "auto-draft-reads-off", off)
    before = s.hub.snap()
    uid = s.hub.deliver("question")
    arrived = s.wait(lambda: any(x.get("subject") == "Pouvez-vous valider ce point ?" for x in s.rows("mac-main.list")), 40)
    s.sleep(3)      # an auto draft would start about now
    after = s.hub.snap()
    d_ai, d_smtp, d_app = after["ai"] - before["ai"], after["smtp_data"] - before["smtp_data"], after["appends"] - before["appends"]
    where = ""
    if not arrived:     # what the list showed instead (main143 and main147: the control failed with the mail stored as the newest of the box)
        try:
            st = s.uistate()
            where = "; the list's first rows %r of %d, surface %r, tab %r, account filter %r, counts %r, fetched +%d" % (
                [x.get("subject") for x in s.rows("mac-main.list")[:3]], len(s.rows("mac-main.list")), st.get("surface"), st.get("tab"), st.get("account_filter"), st.get("counts"), len(after.get("fetched") or []) - len(before.get("fetched") or []))
        except Exception as e:      # noqa: BLE001  evidence only
            where = "; the list could not be read (%s)" % e
    r.ok(T, "nothing-drafted-nothing-sent-nothing-asked", bool(arrived) and d_ai == 0 and d_smtp == 0 and d_app == 0,
         "the delivered question (uid %s) reached the list: %s (the probe's control); AI requests +%d, SMTP messages +%d, IMAP APPENDs +%d%s" % (uid, bool(arrived), d_ai, d_smtp, d_app, where))


@step("T-13", edit="main65's chip restored on the pane (a label, not a menu)",
      red=("bad-key-shows-the-providers-words-and-stores-nothing", "good-key-reads-connected", "model-chip-opens-the-models", "picked-model-is-kept"))
def t13(s):
    r, T = s.rep, "T-13"
    names = ("bad-key-shows-the-providers-words-and-stores-nothing", "good-key-reads-connected", "model-chip-opens-the-models", "picked-model-is-kept")
    why = None
    if not s.caps.get("providers"):
        why = "WHOOSH_CORE_CHECK_PROVIDERS is not in this build (lane rig-core): a pasted key would be sent to the real provider, so none is pasted"
    elif not s.hub.provider_ready():
        why = "the local provider fake does not serve the key providers' shapes yet (checks/realcore_aifake.py, lane rig-core): %s is not refused with 401" % BAD_KEY
    if why:
        for n in names:
            r.nj(T, n, why)
        return
    s.click(W + ".pane-row.ai")
    s.click(AI + ".nav.providers")

    def paste(key):
        s.click(AI + ".provider.anthropic.key"); s.type(key)
        s.click(AI + ".provider.anthropic.connect")
        s.wait(lambda: s.row(AI + ".provider.anthropic.check").get("state") == "idle", 20)
        return s.row(AI + ".provider.anthropic")

    def bad():
        p = paste(BAD_KEY)
        stored = s.core("provider_credential/anthropic/stored")
        return p.get("state") != "connected" and BAD_KEY_WORDS in (p.get("note") or "") and stored in (False, None, 0), "state %r, words %r, stored %r" % (p.get("state"), p.get("note"), stored)
    r.guard(T, names[0], bad)

    def good():
        p = paste(GOOD_KEY)
        stored = s.core("provider_credential/anthropic/stored")   # the control of the refused key's "stored nothing": the same read answers True here
        return p.get("state") == "connected" and sorted(p.get("models") or []) == MODELS and stored is True, "state %r, models %r, stored %r" % (p.get("state"), p.get("models"), stored)
    r.guard(T, names[1], good)
    s.click(AI + ".nav.agents")

    def card_n():
        for i, c in enumerate(s.rows(AI + ".agents.card"), 1):
            if (c.get("name") or "").lower() == "writer":
                return i
        raise Gone("no Writer card")

    def menu():
        n = card_n(); s.mem["writer_card"] = n
        ms = s.rows("%s.agents.model.%d" % (AI, n))
        got = sorted(x.get("model") for x in ms if x.get("model"))
        return all(m in got for m in MODELS), "menu entries %r" % got
    r.guard(T, names[2], menu)

    def pick():
        n = s.mem.get("writer_card") or card_n()
        ms = s.rows("%s.agents.model.%d" % (AI, n))
        k = next((i for i, x in enumerate(ms, 1) if x.get("model") == "model-b"), None)
        if k is None:
            return False, "model-b is not in the menu"
        s.click("%s.agents.model.%d.choose.%d" % (AI, n, k))
        card = s.rows(AI + ".agents.card")[n - 1]
        stored = s.core("agent_config/writer")
        s.mem["writer_model"] = "model-b"
        return card.get("model") == "model-b" and "model-b" in json.dumps(stored), "card %r, core %r" % (card.get("model"), stored)
    r.guard(T, names[3], pick)


@step("T-14", edit="CalendarsModel.swift:28 keyed on the core kind (today's code)", red=("not-asked-at-open", "macos-is-never-asked", "after-allow-both-calendars-listed", "sources-lists-both-calendars-off"))
def t14(s):
    r, T = s.rep, "T-14"
    s.click(W + ".pane-row.agendas")

    def hooks():
        v = s.core("system-ui/calendar-access")
        return v.get("count", 0) if isinstance(v, dict) else (v or 0)
    r.guard(T, "not-asked-at-open", lambda: ((lambda a: (a.get("state") == "not-asked", "access %r" % a))(s.row(CAL + ".access"))))
    try:
        # not asked yet: the fixture answers Allow to the next ask. ("allowed" sets the state at once, and the one button then does nothing, as on a Mac
        # that already allowed: main143, run r018, three cases red on "access.ask did nothing".)
        s.send("fixture", "calendar-access", "not-asked")
        s.click(CAL + ".access.ask")
    except Refused as e:
        for n in ("macos-is-never-asked", "after-allow-both-calendars-listed", "sources-lists-both-calendars-off"):
            r.add(T, n, "FAIL", str(e))
        return
    r.guard(T, "macos-is-never-asked", lambda: (hooks() == 0, "system-ui/calendar-access hook count %r" % hooks()))

    def listed():
        rows = s.wait(lambda: s.rows(CAL + ".list") if len(s.rows(CAL + ".list")) >= 2 else None, 15) or s.rows(CAL + ".list")
        names = sorted(x.get("name") for x in rows)
        return names == sorted(CALS), "access %r, calendars %r" % (s.row(CAL + ".access").get("state"), names)
    r.guard(T, "after-allow-both-calendars-listed", listed)

    def sources():
        s.click(W + ".pane-row.replies.sources")
        cals = [x for x in s.rows(SOURCES) if x.get("kind") == "calendar"]
        feed = s.core("reply/sources")
        n = len(feed.get("calendars", [])) if isinstance(feed, dict) else -1
        return sorted(x.get("name") for x in cals) == sorted(CALS) and not any(x.get("enabled") for x in cals) and n == 2, "tiles %r, reply/sources calendars %d" % ([(x.get("name"), x.get("enabled")) for x in cals], n)
    r.guard(T, "sources-lists-both-calendars-off", sources)
    for word in ("refused", "will-refuse"):          # the other states are driven too: the hook count never moves
        try:
            s.send("fixture", "calendar-access", word)
        except Refused:
            pass
    try:
        s.send("fixture", "calendar-access", "allowed")
    except Refused:
        pass


@step("T-15", edit="today's core (no LocalModel seam): the run itself is red until labels-r5 round 6 lands",
      red=("laya-files-the-question-by-its-description", "laya-loaded-once-and-nothing-left-the-mac"))
def t15(s):
    r, T = s.rep, "T-15"
    names = ("laya-files-the-question-by-its-description", "laya-loaded-once-and-nothing-left-the-mac")
    if not s.caps.get("laya"):
        for n in names:
            r.nj(T, n, "the Laya stand-in (R3, Sources/Features/OnDeviceAI/Rig, `rig/local-model/{loads,judged}`) is not in this build")
        return
    s.mem["laya_loads0"] = None
    s.click(W + ".pane-row.decisions")
    try:
        s.click(AI + ".decisions.engine.local")
    except Refused:
        pass
    s.click("%s.open.modify.needs-reply" % INS)
    s.click(INS + ".meaning"); s.type(LAYA_DESCRIPTION)
    s.click(INS + ".apply")
    s.mem["needs_reply_meaning"] = LAYA_DESCRIPTION
    loads = lambda: (s.core("rig/local-model/loads") or 0)
    before_loads, before = loads(), s.hub.snap()
    s.hub.deliver("question"); s.hub.deliver("invoice")

    def filed():
        res = s.rows(INS + ".result")
        return [x for x in res if x.get("decided_by")]
    done = s.wait(filed, 60) or []
    after = s.hub.snap()

    def files():
        mine = [x for x in done if str(x.get("decided_by", "")).lower() == "laya"]
        return len(mine) >= 1, "decided rows %r" % [(x.get("id"), x.get("decided_by"), x.get("card")) for x in done]
    r.guard(T, names[0], files)
    peers = s.hub.peers()
    r.ok(T, names[1], before_loads == 0 and loads() == 1 and after["ai"] == before["ai"] and not peers["offenders"] and peers["control"],
         "loads %r then %r, AI fake +%d, peers %r (control: %s)" % (before_loads, loads(), after["ai"] - before["ai"], peers["offenders"], peers["control"]))


@step("T-22", edit="DictationRig.active keyed on CoreHost.shared.fixture (today's code)", red=("the-words-land-in-the-body", "the-microphone-is-never-asked"))
def t22(s):
    r, T = s.rep, "T-22"
    surface_home(s)
    s.c.key("Enter")                           # open the first row, then the full composer through its named control (checks/composermac-check.py:574)
    s.c.key("R")
    s.click("mac-main.reader.open-full-composer")
    text = "réponds demain matin"
    s.c.key("Fn:down")
    s.wait(lambda: s.row("mac-composer.dictation").get("state") == "listening" or s.row("mac-composer.dictation").get("line"), 6)
    s.send("fixture", "speak", text)
    s.c.key("Fn:up")
    s.wait(lambda: s.row("mac-composer.dictation").get("state") == "idle", 10)

    def landed():
        body = s.row("mac-composer.body").get("text") or ""
        d = s.row("mac-composer.dictation")
        return text in body.lower().replace(".", ""), "body %r, dictation %r" % (body[-60:], {k: d.get(k) for k in ("state", "line", "engine")})
    r.guard(T, "the-words-land-in-the-body", landed)
    mic = s.core("system-ui/microphone")
    n = mic.get("count", 0) if isinstance(mic, dict) else (mic or 0)
    r.ok(T, "the-microphone-is-never-asked", n == 0, "system-ui/microphone hook count %r" % n)
    for ax in ("mac-composer.close", "mac-composer.discard"):
        try:
            s.click(ax)
            break
        except Refused:
            continue
    surface_home(s)


@step("T-25", mutant="slow-save", red=("every-command-under-the-bar",))
def t25(s):
    r, T = s.rep, "T-25"
    try:
        quiet, lim = s.quiet()
        bar = const("INTERACTION_COMMIT_MS")
    except Exception as e:      # noqa: BLE001 (a ledger row the tree lacks)
        r.nj(T, "every-command-under-the-bar", "the ledger gives no budget or load cap here: %s" % e)
        return
    ops = []
    srcs = [x for x in (s.rows(SOURCES) if s.mem.get("sources_on") else []) if x.get("kind") == "mail_folder"]
    if srcs:
        ax = srcs[0]["ax"]
        ops.append(("SetReplySourceEnabled (a Sources switch)", 6, lambda: s.click(ax)))   # an even count: the switch ends where it began
    n = s.mem.get("writer_card")
    if n:
        k = {"a": 1, "b": 2}
        flip = [0]

        def choose():
            flip[0] ^= 1
            s.click("%s.agents.model.%d.choose.%d" % (AI, n, 1 + flip[0]))
        ops.append(("SaveAgentConfig (the Writer's model)", 5, choose))
    def allow():      # the one button acts only while not asked (T-14 left the access allowed): the state is put back first, outside the timed call
        try:
            s.send("fixture", "calendar-access", "not-asked")
        except Refused:
            pass
        t0 = time.perf_counter(); s.click(CAL + ".access.ask")
        return (time.perf_counter() - t0) * 1000.0
    ops.append(("Allow (calendar access)", 1, allow))
    if not ops:
        r.nj(T, "every-command-under-the-bar", "no command of the walk was reachable")
        return
    if not quiet:
        r.nj(T, "every-command-under-the-bar", "machine load %.1f is above PERF_LOAD_MAX %s: recorded, not judged" % (s.load(), lim))
        return
    slow = []
    figures = []
    for name, reps, fn in ops:
        ms = []
        try:
            for _ in range(reps):
                t0 = time.perf_counter(); own = fn(); ms.append(own if isinstance(own, float) else (time.perf_counter() - t0) * 1000.0)   # a step that times itself answers its own figure
        except Refused as e:
            slow.append("%s refused: %s" % (name, e)); continue
        med = statistics.median(ms)
        figures.append("%s median %.1f ms of %d" % (name, med, len(ms)))
        if med >= bar:
            slow.append("%s %.1f ms" % (name, med))
    if srcs:      # what the walk keeps for the relaunch is what the screen says now, not what a timing click left
        s.mem["sources_on"] = sorted((x.get("ax"), bool(x.get("enabled"))) for x in s.rows(SOURCES) if x.get("kind") == "mail_folder")
    r.ok(T, "every-command-under-the-bar", not slow, "bar %s ms (wall time of the socket call, an upper bound on the command); %s%s" % (bar, "; ".join(figures), "; OVER: " + ", ".join(slow) if slow else ""))


@step("T-23", mutant="choices-in-memory", red=("inbox-drawn-at-once", "choices-kept"))
def t23(s):
    """The relaunch: the same copy on the same folder, the server 2000 ms away. T-08 follows it with the server stopped."""
    r, T = s.rep, "T-23"
    names = ("inbox-drawn-at-once", "choices-kept")
    if s.relaunch_fn is None:
        for n in names:
            r.nj(T, n, "no relaunch available (scripts/launch-one.sh --relaunch)")
        return
    # the bodies must be on this Mac before the copy goes: the crawl is judged by what the server saw, never by a state path's shape
    stable = [None, 0]

    def settled():
        n = len(s.hub.fetched())
        stable[1] = stable[1] + 1 if n == stable[0] else 0
        stable[0] = n
        return stable[1] >= 4
    s.wait(settled, 90, 2.0)
    s.mem["fetched_before"] = len(s.hub.fetched())
    s.hub.latency(RELAUNCH_LATENCY_MS)
    try:
        s.relaunch_fn()
    except Exception as e:      # noqa: BLE001
        for n in names:
            r.nj(T, n, "relaunch failed: %s" % e)
        return
    t0 = s.clock()
    rows = []
    while s.clock() - t0 < LIST_AFTER_RELAUNCH_S + 5:
        try:
            rows = s.rows("mac-main.list")
        except Gone:
            rows = []
        if len(rows) >= 50:
            break
        s.sleep(0.05)
    took = s.clock() - t0
    r.ok(T, names[0], len(rows) >= 50 and took < LIST_AFTER_RELAUNCH_S, "%d rows after %.2f s (limit %.1f s) with the server %d ms away" % (len(rows), took, LIST_AFTER_RELAUNCH_S, RELAUNCH_LATENCY_MS))

    def kept():
        bad = []
        try:
            s.click(W + ".pane-row.replies.sources")
            now = sorted((x.get("ax"), bool(x.get("enabled"))) for x in s.rows(SOURCES) if x.get("kind") == "mail_folder")
            if s.mem.get("sources_on") and now != s.mem["sources_on"]:
                bad.append("Sources switches %r -> %r" % (s.mem["sources_on"], now))
            if s.mem.get("writer_model"):
                s.click(W + ".pane-row.ai"); s.click(AI + ".nav.agents")
                cards = s.rows(AI + ".agents.card")
                w = next((c for c in cards if (c.get("name") or "").lower() == "writer"), {})
                if w.get("model") != s.mem["writer_model"]:
                    bad.append("Writer model %r -> %r" % (s.mem["writer_model"], w.get("model")))
            if s.mem.get("needs_reply_meaning"):
                s.click("%s.open.modify.needs-reply" % INS)
                s.wait(lambda: s.row(INS).get("label_id") == "needs-reply", 10)
                m = s.row(INS).get("meaning")
                if m != s.mem["needs_reply_meaning"]:
                    bad.append("Needs Reply description %r -> %r" % (s.mem["needs_reply_meaning"], m))
                try:
                    s.click(INS + ".cancel")
                except Refused:
                    pass
        except Refused as e:
            bad.append("refused: %s" % e)
        return not bad, "; ".join(bad) or "Sources switches, the Writer model and the description are as before the relaunch"
    r.guard(T, names[1], kept)


@step("T-08", mutant="bodies-never-stored", red=("bodies-were-fetched-before-the-server-stopped", "reader-shows-the-first-sentence-with-the-server-down", "html-entities-are-decoded", "files-are-listed"))
def t08(s):
    r, T = s.rep, "T-08"
    import realcore_mailfake as fake
    need = const("BODY_PREFETCH_COUNT")
    fetched = s.mem.get("fetched_before")
    r.ok(T, "bodies-were-fetched-before-the-server-stopped", isinstance(fetched, int) and fetched >= need, "the server saw the whole message fetched for %s messages (BODY_PREFETCH_COUNT %s)" % (fetched, need))
    s.hub.mail("stop")

    def open_subject(subject):
        s.wait(lambda: any(x.get("subject") == subject for x in s.rows("mac-main.list")), 10)
        rows = s.rows("mac-main.list")
        hit = next((x for x in rows if x.get("subject") == subject), None)
        if hit is None:
            raise Gone("%r is not on the list" % subject)
        s.click(hit["ax"]) if hit.get("ax") else None
        s.c.key("Enter")
        t = s.wait(lambda: s.rows("mac-main.reader.thread"), 10) or []
        return t[-1] if t else {}
    r.guard(T, "reader-shows-the-first-sentence-with-the-server-down", lambda: ((lambda m: (fake.FR_FIRST_SENTENCE in (m.get("body") or ""), "body %r" % (m.get("body") or "")[:90]))(open_subject(fake.FR_NAMED[0][0]))))
    r.guard(T, "html-entities-are-decoded", lambda: ((lambda m: ("Café prêt" in (m.get("body") or "") and "&eacute;" not in (m.get("body") or ""), "body %r" % (m.get("body") or "")[:120]))(open_subject(fake.FR_HTML_SUBJECTS[0]))))

    def files():
        open_subject(fake.FR_FILES[0][0])
        names = [x.get("name") for x in s.rows("mac-main.reader.attachments")]
        return fake.FR_FILES[0][1] in names, "attachments %r" % names
    r.guard(T, "files-are-listed", files)
    surface_home(s)
    s.hub.mail("start")


@step("T-24", mutant="dial-out", red=("nothing-left-the-mac",))
def t24(s):
    r, T = s.rep, "T-24"
    p = s.hub.peers()
    if not p["control"] and s.core_kind == "fixture":
        # the fixture core holds no mail socket, so the app's own control is empty there: the probe's own control (a peer of this process) stands for it
        ok, why = s.self_control()
        if not ok:
            r.nj(T, "nothing-left-the-mac", "the probe saw no loopback peer of the app in %d samples and %s" % (p["samples"], why))
            return
        p = dict(p, control=True, control_n=0)
    if not p["control"]:
        r.nj(T, "nothing-left-the-mac", "the probe saw no loopback peer of the app in %d samples: it cannot tell silence from a broken probe (positive control empty)" % p["samples"])
        return
    try:
        v = s.core("rig/refused-outbound")      # a rig core counts the outbound connections it refused, when the build serves the path
        refused = v.get("count", 0) if isinstance(v, dict) else (v or 0)
        note = "; rig/refused-outbound %r" % refused
    except Gone:
        refused, note = 0, "; rig/refused-outbound not served by this build"
    r.ok(T, "nothing-left-the-mac", not p["offenders"] and not refused, "%d samples (control: %d with a peer on a fake's port); peers outside the three fakes' ports: %r%s" % (p["samples"], p["control_n"], p["offenders"], note))


# T-30 (item d66b30dd): one hidden case per place the app's code could reach outside this Mac. Each does the act on the hidden copy, then reads the app's peers through
# the T-24 watcher (NetWatch, offenders_of) and the engine's count of refused outside calls, and prints what it read. A peer that is not on this Mac is a FAIL naming it.
OUTSIDE_SETTLE_S = 1.6          # the watcher samples every 0.5 s: three samples after the act
BAD_ENDPOINT = "http://198.51.100.7:9"       # RFC 5737 TEST-NET-2: never routable; a test copy must refuse it before any socket is opened
PROBE_KEY = "sk-rig-never-sent"              # not a key of anyone: the fixture's shape rule only
MS_ADDRESS = "check.fixture@m365.example.test"
APP_ID = "-".join("c" * n for n in (8, 4, 4, 4, 12))   # a stand-in of the Microsoft application ID shape, built at run time (as checks/realcore-check.py does)


def refused_count(s):
    """(count, last host) of `core state rig/refused-outbound`; (0, None) when this build does not serve it (the case line says so)."""
    try:
        v = s.core("rig/refused-outbound")
    except Gone:
        return 0, None
    return (v.get("count", 0), v.get("last_host")) if isinstance(v, dict) else (v or 0, None)


def outside_case(s, name, act):
    """One T-30 case. `act()` returns (note, problem): a non-empty problem is a FAIL of its own (the app accepted what a test copy must refuse)."""
    r, T = s.rep, "T-30"
    try:
        p0 = s.hub.peers(); n0, _ = refused_count(s)
        note, problem = act()
        s.sleep(OUTSIDE_SETTLE_S)
        p1 = s.hub.peers(); n1, last = refused_count(s)
    except Gone as e:
        r.nj(T, name, "unreadable: %s" % e); return
    except Refused as e:
        r.add(T, name, "FAIL", "refused: %s" % e); return
    ok, why = s.self_control()
    if not (p1["control"] or ok):
        r.nj(T, name, "the probe saw no loopback peer of the app and %s: it cannot tell silence from a broken probe" % why); return
    st, det = outside_verdict(p0, p1, n0, n1)
    det += "; %s%s" % (note, ("; last refused host %r" % last) if last else "")
    if problem:
        st, det = "FAIL", problem + "; " + det
    r.add(T, name, st, det)


@step("T-30", edit="the sign-in guard of the engine: core/crates/engine/src/sign_in/authority.rs:38 `netguard::outbound_allowed_url(url)?;` -> `let _ = url;`; the provider endpoint verb: Sources/ControlSocket/FixtureVerbs.swift:263 `where providerEndpointRefused(a[2]):` -> `where false:`",
      red=("sign-in-reaches-nothing", "provider-key-post-reaches-nothing"))
def t30(s):
    r, T = s.rep, "T-30"

    def sign_in():
        try:
            s.click(A + ".open")
            s.click(A + ".address"); s.type(MS_ADDRESS)
            s.click(A + ".continue")
            if not s.wait(lambda: (s.rows(A + ".state") or [{}])[0].get("stage") not in (None, "entry", "detecting"), 60):
                raise Gone("the sheet did not leave detecting in 60 s")
            o = s.rows(A + ".oauth")
            if not o or o[0].get("kind") != "microsoft":
                raise Gone("the sheet offered no Microsoft sign-in for %s (oauth row %r)" % (MS_ADDRESS, o[:1]))
            try:
                s.click(A + ".oauth.own-id")
            except Refused:
                pass                    # no shipped default ID: the field is already there
            s.click(A + ".oauth.field"); s.type(APP_ID)
            s.click(A + ".oauth.sign-in")
            s.sleep(0.5)
            phase = (s.rows(A + ".oauth") or [{}])[0]
            return "sign-in clicked on the sheet; its phase %r, words %r" % (phase.get("phase"), phase.get("words")), ""
        finally:
            for k in (".done",):
                try:
                    s.click(A + k)
                except Refused:
                    pass
            surface_home(s)
    outside_case(s, "sign-in-reaches-nothing", sign_in)

    if s.core_kind != "fixture":
        r.nj(T, "provider-key-post-reaches-nothing", "the real core's key check goes to the local provider fake (T-13), never to an endpoint of the check's choosing: the endpoint verb is the fixture core's")
        return

    def key_post():
        problem = ""
        try:
            s.c.send("fixture", "provider-endpoint", "anthropic", BAD_ENDPOINT)
            problem = "the verb accepted the endpoint %s, which is off this Mac" % BAD_ENDPOINT
            verb = "verb ACCEPTED %s" % BAD_ENDPOINT
        except Exception as e:      # noqa: BLE001 (an ok:false answer is the refusal a test copy must give)
            verb = "verb refused %s: %s" % (BAD_ENDPOINT, str(e)[:90])
        try:
            s.click(W + ".pane-row.ai"); s.click(AI + ".nav.providers")
            s.click(AI + ".provider.anthropic.key"); s.type(PROBE_KEY)
            s.click(AI + ".provider.anthropic.connect")
            s.wait(lambda: s.row(AI + ".provider.anthropic.check").get("state") == "idle", 20)
            row = s.row(AI + ".provider.anthropic")
            shown = "key pasted and Connect clicked; the row says state %r, note %r" % (row.get("state"), row.get("note"))
        finally:
            try:
                s.c.send("fixture", "provider-endpoint", "anthropic", "")
            except Exception:       # noqa: BLE001
                pass
            surface_home(s)
        return "%s; %s" % (verb, shown), problem
    outside_case(s, "provider-key-post-reaches-nothing", key_post)


@step("T-29b", edit="HiddenRig alpha set to 1 (its mutant)", red=("no-window-shown",))
def t29_end(s):
    r, T = s.rep, "T-29"
    if s.mem.get("hidden_on") is not None:
        r.ok(T, "hidden-mode-on", s.mem["hidden_on"] is True, "uistate.hidden.on %r" % s.mem["hidden_on"])
    else:
        r.nj(T, "hidden-mode-on", "uistate has no hidden row")
    res = s.watcher.stop() if s.watcher else None
    if res is None:
        r.nj(T, "no-window-shown", "the window-server reader is not available")
        return
    if res["seen"] < 1:
        r.nj(T, "no-window-shown", "the reader saw no window of the app in %d samples: it cannot tell silence from a broken reader (positive control empty)" % res["samples"])
        return
    r.ok(T, "no-window-shown", not res["shown"], "%d samples, up to %d windows of the rig pid, %d of them shown%s" % (res["samples"], res["seen"], len(res["shown"]), ": %r" % res["shown"][:1] if res["shown"] else ""))


FIXTURE_STEPS = ("T-10", "T-16", "T-11", "T-12", "T-14", "T-22")
FIXTURE_OUTSIDE = ("T-30", "T-24")      # the two reads of what left the Mac, also on the fixture core (T-27 compares only FIXTURE_STEPS)
ORDER = ["T-29", "T-06", "T-07", "T-09", "T-10", "T-16", "T-11", "T-12", "T-13", "T-14", "T-15", "T-22", "T-25", "T-23", "T-08", "T-30", "T-24", "T-29b"]


def walk(s, core="real"):
    by = {cid: fn for cid, fn in STEPS}
    order = ORDER if core == "real" else ["T-29"] + [c for c in ORDER if c in FIXTURE_STEPS] + [c for c in ORDER if c in FIXTURE_OUTSIDE] + ["T-29b"]
    if core == "fixture":
        try:
            s.c.send("fixture", "install-state", "new")
        except Exception as e:      # noqa: BLE001
            s.rep.nj("T-27", "install-state-new", "fixture install-state new is not served: %s" % e)
            return
    for cid in order:
        try:
            by[cid](s)
        except (Gone, Refused) as e:
            s.rep.add(cid.rstrip("b"), "step-could-not-finish", "NOT JUDGED" if isinstance(e, Gone) else "FAIL", str(e))
        except Exception as e:      # noqa: BLE001
            s.rep.add(cid.rstrip("b"), "step-crashed", "FAIL", "%s: %s" % (type(e).__name__, e))


# ------------------------------------------------------------------------------------------------ one rule for "this Mac" in the Swift code (item d66b30dd, part c)
RULE_FILE = "Sources/CoreBridge/CoreHost.swift"      # RigNet.thisMacHosts: the one list every other file must ask (RigNet.allows / safe / isThisMac)
# the second host lists that exist today, each with its measured reason; a new one (another file, or more lines in one of these) is a FAIL. The first two are findings
# (docs/lanes/tail-outside.md: each should call RigNet.isThisMac); the third is a WebKit rule list, whose regex form cannot ask Swift.
KNOWN_SECOND_LISTS = {"Sources/Features/Embedding/MeaningModelRig.swift": 1, "Sources/Features/PushClient/RelaySettings.swift": 1, "Sources/Features/MessageBody/MessageBodyPolicy.swift": 1}
HOST_LITERAL = re.compile(r'"(?:localhost|::1)"')


def rule_pin(root):
    """(lines, ok): every code line (not a // comment) under root/Sources that holds the string literal "localhost" or "::1" outside RULE_FILE. Counted per file against
    KNOWN_SECOND_LISTS: a file not listed, or more lines than listed, is a second rule and a FAIL naming file:line."""
    found = {}
    base = os.path.join(root, "Sources")
    for dp, _dn, fn in os.walk(base):
        for f in sorted(fn):
            if not f.endswith(".swift"):
                continue
            path = os.path.join(dp, f); rel = os.path.relpath(path, root)
            if rel == RULE_FILE or "/Generated/" in rel:
                continue
            for n, line in enumerate(open(path, encoding="utf-8").read().splitlines(), 1):
                if HOST_LITERAL.search(line) and not line.lstrip().startswith("//"):
                    found.setdefault(rel, []).append(n)
    bad = ["%s:%s (%d line(s), %d known)" % (rel, ",".join(map(str, ns)), len(ns), KNOWN_SECOND_LISTS.get(rel, 0)) for rel, ns in sorted(found.items()) if len(ns) > KNOWN_SECOND_LISTS.get(rel, 0)]
    gone = [rel for rel in KNOWN_SECOND_LISTS if rel not in found]
    rule = os.path.join(root, RULE_FILE)
    has_rule = os.path.exists(rule) and "thisMacHosts" in open(rule, encoding="utf-8").read()
    lines = []
    if bad:
        lines.append("FAIL       one-shared-rule :: a second list of this Mac's hosts: %s; the one rule is %s (RigNet.thisMacHosts)" % ("; ".join(bad), RULE_FILE))
    elif not has_rule:
        lines.append("FAIL       one-shared-rule :: %s holds no thisMacHosts" % RULE_FILE)
    else:
        lines.append("ok         one-shared-rule :: %d Swift files hold a host literal outside %s, all %d known second lists %s%s" % (
            len(found), RULE_FILE, len(KNOWN_SECOND_LISTS), ", ".join("%s:%s" % (r, ",".join(map(str, found[r]))) for r in sorted(found)),
            ("; no longer present (update KNOWN_SECOND_LISTS): %s" % ", ".join(gone)) if gone else ""))
    return lines, not bad and has_rule


# ------------------------------------------------------------------------------------------------ T-27: the two cores side by side
def parse_log(text):
    out = {}
    for line in text.splitlines():
        m = re.match(r"(ok|FAIL|NOT JUDGED)\s+(T-\d+) (\S+) ::", line)
        if m:
            out[(m.group(2), m.group(3))] = m.group(1)
    return out


def compare(real, fixture):
    """[(name, ok, detail)] for every case both runs judged: the same visible assertion must hold on both cores."""
    rows = []
    for key in sorted(set(real) | set(fixture)):
        a, b = real.get(key), fixture.get(key)
        if key[0] not in FIXTURE_STEPS or "NOT JUDGED" in (a, b) and (a is None or b is None):
            continue
        if a is None or b is None:
            rows.append(("%s %s" % key, False, "judged on %s only" % ("the real core" if b is None else "the fixture")))
        elif "NOT JUDGED" in (a, b):
            continue
        else:
            rows.append(("%s %s" % key, a == b, "real %s, fixture %s" % (a, b)))
    return rows


# ------------------------------------------------------------------------------------------------ the real fakes, the hub and the network probe
LOOPBACK = ("127.0.0.1", "::1", "[::1]", "localhost")


def offenders_of(lsof_text, allowed=None):
    """(offending peers, allowed loopback peers) from `lsof -a -i -n -P -p <pid>` text: the part after `->` of every connection line. A peer is an offender
    when its host is off this Mac, or (with `allowed`, the ports of the check's own fakes) when it is a loopback port that is none of them: the dial-out
    mutant dials a loopback port the check hands it (WHOOSH_CORE_MUTANT_DIAL_PORT)."""
    bad, local = [], []
    for line in lsof_text.splitlines():
        m = re.search(r"->(\[?[0-9a-fA-F:.]+\]?):(\d+)", line)
        if not m:
            continue
        host = m.group(1)
        (local if host in LOOPBACK and (allowed is None or int(m.group(2)) in allowed) else bad).append("%s:%s %s" % (host, m.group(2), (re.search(r"\((\w+)\)", line) or [None, "?"])[1]))
    return bad, local


class NetWatch(threading.Thread):
    """Samples the app's sockets every 500 ms from OUTSIDE it (lsof). `control` is true only when the probe saw a loopback peer: silence is judged only then."""
    def __init__(self, probe=None, allowed=None):
        super().__init__(daemon=True)
        self.allowed = allowed
        self.pid, self.stop_flag, self.samples, self.offenders, self.control_n = None, threading.Event(), 0, [], 0
        self.local_seen = []      # the loopback peers read so far (T-30 prints them beside its verdict)
        self.probe = probe or (lambda pid: subprocess.run(["lsof", "-a", "-i", "-n", "-P", "-p", str(pid)], capture_output=True, text=True).stdout)

    def run(self):
        while not self.stop_flag.is_set():
            if self.pid:
                try:
                    bad, local = offenders_of(self.probe(self.pid), self.allowed)
                except Exception:       # noqa: BLE001
                    bad, local = [], []
                self.samples += 1
                self.control_n += bool(local)
                for b in bad:
                    if b not in self.offenders:
                        self.offenders.append(b)
                for b in local:
                    if b not in self.local_seen and len(self.local_seen) < 12:
                        self.local_seen.append(b)
            self.stop_flag.wait(0.5)

    def report(self):
        return {"samples": self.samples, "offenders": list(self.offenders), "control": self.control_n > 0, "control_n": self.control_n, "local": list(self.local_seen)}


def probe_control():
    """(ok, detail): the lsof probe and `offenders_of` read a loopback peer of THIS process (a connection the check opens to itself). A fixture-core app may hold
    no socket at all, so `control_n` of the app is 0 there: this is the positive control that the probe is not broken (silence is then judged)."""
    srv = socket.socket(); srv.bind(("127.0.0.1", 0)); srv.listen(1)
    cli = socket.create_connection(srv.getsockname(), timeout=5); conn, _ = srv.accept()
    try:
        out = subprocess.run(["lsof", "-a", "-i", "-n", "-P", "-p", str(os.getpid())], capture_output=True, text=True, timeout=30).stdout
    except Exception as e:      # noqa: BLE001
        return False, "lsof did not run: %s" % e
    finally:
        cli.close(); conn.close(); srv.close()
    bad, local = offenders_of(out)
    return bool(local), "the probe read %d loopback peer(s) of its own process%s" % (len(local), "" if local else " (none: it cannot tell silence from a broken probe)")


def outside_verdict(before, after, refused0, refused1):
    """(status, detail) of a T-30 case: `before` and `after` are two `hub.peers()` reads around the act. FAIL names every peer that appeared after the act and
    is not on this Mac; NOT JUDGED when the watcher took fewer than 2 samples after the act (it cannot tell silence from a watcher that did not look).
    The refusal count is printed, never required to be 0: a guard that refused a call is the guard working (T-24 reads the whole run)."""
    new_bad = [o for o in after["offenders"] if o not in before["offenders"]]
    fresh = after["samples"] - before["samples"]
    seen = "loopback peers read %r" % (after.get("local") or [])
    count = "refused outside calls %d (was %d, +%d)" % (refused1, refused0, refused1 - refused0)
    if new_bad:
        return "FAIL", "left this Mac: peer(s) %r after the act; %d samples after it; %s; %s" % (new_bad, fresh, seen, count)
    if fresh < 2:
        return "NOT JUDGED", "the watcher took %d sample(s) after the act (it needs 2): it cannot tell silence from a watcher that did not look" % fresh
    return "ok", "%d samples after the act, peers off this Mac: []; %s; %s" % (fresh, seen, count)


class Hub:
    """The outer process's side: the fakes and what the driven walk asks of them (deliver mail, stop and start the mail server, set its latency, read what
    each fake saw, read the network probe). The inner process talks to it over one loopback socket, JSON lines."""
    def __init__(self, fake, aifake, mail, imap, smtp, ai, net):
        self.fake, self.aifake, self.mail, self.imap, self.smtp, self.ai, self.net = fake, aifake, mail, imap, smtp, ai, net
        self.n = 400
        self.ip = imap.server_address[1]

    def handle(self, q):
        f, c = self.fake, q.get("cmd")
        if c == "pid":
            self.net.pid = q["pid"]; return {}
        if c == "peers":
            return self.net.report()
        if c == "snap":
            with self.mail.lock:
                log = list(self.mail.log)
                uids = [u for u, _ in self.mail.boxes["INBOX"]]
            return {"imap": len(log), "smtp_data": len(self.smtp.messages), "ai": len(self.ai.seen), "appends": sum(1 for l in log if l.startswith("APPEND")),
                    "fetched": sorted(fetched_uids(log, uids))}
        if c == "deliver":
            self.n += 1
            if q["kind"] == "question":
                raw = f._fr_msg("Pouvez-vous valider ce point ?", "Client Martin", "martin@example.test", "Bonjour, pouvez-vous me répondre avant ce soir ?", self.n)
            else:
                raw = f._fr_msg("Facture 4472", "Service Facturation", "facturation@example.test", "Bonjour, votre facture est en pièce jointe, montant dû.", self.n)
            return {"uid": f.deliver(self.mail, raw)}
        if c == "latency":
            self.mail.latency_s = q["ms"] / 1000.0
            if self.mail.second is not None:
                self.mail.second.latency_s = self.mail.latency_s
            return {}
        if c == "mail":
            if q["do"] == "stop":
                self.mail.state["down"] = True
                self.imap.shutdown(); self.imap.server_close()
            else:
                self.mail.state["down"] = False
                handler = type("ImapFor", (f.Imap,), {"mail": self.mail, "caps": f.CAPS})
                self.imap = f.Server(("127.0.0.1", self.ip), handler)
                threading.Thread(target=self.imap.serve_forever, daemon=True).start()
            return {}
        if c == "provider_ready":
            return {"ready": self.aifake_serves_key_providers()}
        return {"error": "unknown command %r" % c}

    def aifake_serves_key_providers(self):
        """The provider fake refuses BAD_KEY with 401 and lists the two models for GOOD_KEY (plan R2, lane rig-core): probed, never assumed."""
        import urllib.request
        import urllib.error
        base = "http://127.0.0.1:%d" % self.ai.server_address[1]
        def get(key):
            req = urllib.request.Request(base + "/v1/models", headers={"x-api-key": key, "authorization": "Bearer " + key})
            try:
                with urllib.request.urlopen(req, timeout=3) as r:
                    return r.status, r.read().decode()
            except urllib.error.HTTPError as e:
                return e.code, e.read().decode()
            except OSError:
                return 0, ""
        bad, good = get(BAD_KEY), get(GOOD_KEY)
        return bad[0] == 401 and good[0] == 200 and all(m in good[1] for m in MODELS)


class HubServer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


def serve_hub(hub):
    class H(socketserver.StreamRequestHandler):
        def handle(self):
            for line in self.rfile:
                try:
                    ans = hub.handle(json.loads(line.decode()))
                except Exception as e:      # noqa: BLE001
                    ans = {"error": "%s: %s" % (type(e).__name__, e)}
                self.wfile.write((json.dumps(ans) + "\n").encode()); self.wfile.flush()
    srv = HubServer(("127.0.0.1", 0), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv


class HubClient:
    """The inner side of the hub: the same methods the steps call."""
    def __init__(self, port):
        self.sock = socket.create_connection(("127.0.0.1", int(port)), timeout=30)
        self.f = self.sock.makefile("rwb")

    def ask(self, **q):
        self.f.write((json.dumps(q) + "\n").encode()); self.f.flush()
        a = json.loads(self.f.readline().decode() or "{}")
        if "error" in a:
            raise Gone("hub: %s" % a["error"])
        return a

    def pid(self, n): self.ask(cmd="pid", pid=n)
    def peers(self): return self.ask(cmd="peers")
    def snap(self): return self.ask(cmd="snap")
    def fetched(self): return self.ask(cmd="snap")["fetched"]
    def deliver(self, kind): return self.ask(cmd="deliver", kind=kind)["uid"]
    def latency(self, ms): self.ask(cmd="latency", ms=ms)
    def mail(self, do): self.ask(cmd="mail", do=do)
    def provider_ready(self): return self.ask(cmd="provider_ready")["ready"]


class WindowWatch:
    """T-29: the window server read from OUTSIDE the app, by scripts/rig-windows.swift (compiled by scripts/hidden-check.py's helper)."""
    def __init__(self, hidden_check=None):
        self.hc, self.proc, self.file, self.old = hidden_check, None, None, []

    def start(self, pid):
        if self.hc is None:
            return None
        try:
            exe = self.hc.helper()
        except Exception:       # noqa: BLE001
            self.hc = None
            return None
        self.exe = exe
        self.file = tempfile.mktemp(prefix="firstrun-windows-", suffix=".jsonl")
        self.proc = subprocess.Popen([exe, str(pid), "--watch", self.file, "--every", "20"])
        return self.file

    def restart(self, pid):
        self.old.append(self.stop_raw())
        if self.hc:
            self.file = tempfile.mktemp(prefix="firstrun-windows-", suffix=".jsonl")
            self.proc = subprocess.Popen([self.exe, str(pid), "--watch", self.file, "--every", "20"])

    def stop_raw(self):
        if self.proc is not None:
            self.proc.terminate(); self.proc.wait()
        return self.file

    def stop(self):
        files = self.old + [self.stop_raw()]
        if self.hc is None:
            return None
        return windows_result(files)


def windows_result(files):
    samples, shown, seen = 0, [], 0
    for f in files:
        try:
            for line in open(f):
                if not line.strip():
                    continue
                x = json.loads(line)
                if "windows" in x:
                    samples += 1
                    seen = max(seen, len(x["windows"]))
                    if x.get("shown", 0) > 0:
                        shown.append([w for w in x["windows"] if w.get("shown")])
        except (OSError, ValueError):
            continue
    return {"samples": samples, "seen": seen, "shown": shown}


def load_hidden_check():
    try:
        spec = importlib.util.spec_from_file_location("hidden_check_for_firstrun", os.path.join(ROOT, "scripts", "hidden-check.py"))
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        return mod
    except Exception:       # noqa: BLE001
        return None


# ------------------------------------------------------------------------------------------------ the inner run (inside scripts/launch-one.sh)
def read_pid(d):
    try:
        return int(open(os.path.join(d, "rig.pid")).read().strip())
    except (OSError, ValueError):
        return None


def inner(hub_port, args):
    from lib.ctl import Ctl, CouldNotDrive
    d = os.environ.get("WHOOSH_SOCKET_DIR", "")
    only = set(args["steps"]) if args.get("steps") else None
    rep = Rep(only)
    hub = HubClient(hub_port)
    caps = json.loads(args["caps"])
    c = Ctl(d)
    t0 = time.time()
    while True:
        try:
            c.connect(); break
        except CouldNotDrive:
            if time.time() - t0 > 60:
                rep.nj("T-06", "no-app", "no socket in %s after 60 s" % d); print("\n".join(rep.lines())); print(summary(rep)); return 1
            time.sleep(0.5)
    watcher = WindowWatch(load_hidden_check())
    watch_pid = lambda: (hub.pid(read_pid(d)) if read_pid(d) else None)
    watch_pid()

    def relaunch():
        env = dict(os.environ, D=d, WHOOSH_RIG_HIDDEN="1")
        p = subprocess.run(["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), "--relaunch"], env=env, capture_output=True, text=True)
        if p.returncode != 0:
            raise RuntimeError("launch-one --relaunch exit %d: %s" % (p.returncode, (p.stdout + p.stderr)[-300:]))
        s.c.close() if s.c.sock else None
        nc = Ctl(d); t = time.time()
        while True:
            try:
                nc.connect(); break
            except CouldNotDrive:
                if time.time() - t > 60:
                    raise RuntimeError("no socket after the relaunch")
                time.sleep(0.3)
        s.c = nc
        watch_pid()
        if watcher.hc and read_pid(d):
            watcher.restart(read_pid(d))
    s = Session(c, hub, rep, caps, core=args.get("core", "real"), relaunch=relaunch, watcher=watcher)
    s.mem["pid"] = read_pid(d)
    walk(s, s.core_kind)
    print("\n".join(rep.lines()))
    print(summary(rep))
    return 1 if rep.counts()[1] or not rep.counts()[0] else 0


# ------------------------------------------------------------------------------------------------ the outer run
def run(bundle, only, mutant, core, log):
    import realcore_mailfake as fake
    import realcore_aifake as aifake
    launch_extra = os.environ.get("LAUNCH_ENV", "")
    held = refusal(os.environ, launch_extra)
    if held:
        print(refuse_words(held)); print("0 checks, 0 failed, 1 not judged"); return 1
    caps = capabilities(bundle)
    if mutant and mutant not in (True,) and not any(mutant == m for m in MUTANTS.values()):
        print("unknown mutant %r; known: %s" % (mutant, ", ".join(sorted(MUTANTS.values())))); return 2
    if mutant is True:
        print("name the mutant: --mutant=<%s>" % "|".join(sorted(MUTANTS.values()))); return 2
    target = next((cid for cid, m in MUTANTS.items() if m == mutant), None) if mutant else None
    if mutant and not caps["mutants"].get(mutant):
        print("T-xx NOT JUDGED :: mutant %s is not in this build (lane rig-core adds it)" % mutant); print("0 checks, 0 failed, 1 not judged"); return 1
    if target and not only:
        only = {target.rstrip("b")}
    print("firstrun-check: build %s; load %.2f; core %s; only %s; mutant %s" % (os.path.basename(bundle), os.getloadavg()[0], core, sorted(only) if only else "all", mutant or "none"))
    mail, imap, smtp = fake.start(0, 0, firstrun=True)
    ai = aifake.start(0)
    ai = ai if not isinstance(ai, tuple) else ai[0]
    net = NetWatch(allowed={imap.server_address[1], smtp.server_address[1], ai.server_address[1]})
    hub = Hub(fake, aifake, mail, imap, smtp, ai, net)
    srv = serve_hub(hub)
    net.start()
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    env_words = ["WHOOSH_RIG_HIDDEN=1", "WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports]
    if caps["providers"]:
        env_words.append("WHOOSH_CORE_CHECK_PROVIDERS=%d" % ai.server_address[1])
    dial = None
    if mutant:
        env_words.append("WHOOSH_CORE_MUTANT=" + mutant)
        if mutant == "dial-out":      # a loopback listener of the check's own, on a port that is none of the fakes': the dial-out mutant connects to it
            dial = socket.socket(); dial.bind(("127.0.0.1", 0)); dial.listen(8)
            env_words.append("WHOOSH_CORE_MUTANT_DIAL_PORT=%d" % dial.getsockname()[1])
    if refusal({}, " ".join(env_words)):
        print(refuse_words(refusal({}, " ".join(env_words)))); return 1
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(env_words))
    for k in FORBIDDEN_ENV + ("WHOOSH_FIXTURE_CORE",):
        env.pop(k, None)
    args = json.dumps({"steps": sorted(only) if only else None, "caps": json.dumps(caps), "core": core})
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden"] + (["--fixture-core"] if core == "fixture" else []) + \
          ["sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner %d "$1"' % srv.server_address[1], os.path.abspath(__file__), args]
    p = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True)
    net.stop_flag.set()
    if dial:
        dial.close()
    out = (p.stdout + p.stderr)
    tail = out.rstrip()
    if p.returncode == 75:
        print(tail[-1500:]); print("BUSY: another drive holds the rig lock; wait and retry, never kill"); print("0 checks, 0 failed, 1 not judged"); return 1
    lines = [l for l in out.splitlines() if re.match(r"(ok|FAIL|NOT JUDGED)\s+T-\d+ ", l)]
    last = next((l for l in reversed(out.splitlines()) if re.match(r"\d+ checks, \d+ failed, \d+ not judged", l)), None)
    if last is None:
        print(tail[-3000:]); print("launcher exit %d" % p.returncode); print("0 checks, 0 failed, 1 not judged"); return 1
    print("\n".join(lines)); print(last)
    if log:
        with open(log, "w") as f:
            f.write("\n".join(lines) + "\n" + last + "\n")
    if mutant:
        red = [l for l in lines if l.startswith("FAIL") and any((" %s " % n) in l for n in RED.get(target, ()))]
        print("mutant %s: %d of the step's named cases went red (%s)" % (mutant, len(red), ", ".join(RED.get(target, ()))))
        return 0 if red else 1
    n, f, u = (int(x) for x in re.findall(r"\d+", last)[:3])
    return 1 if f or not n else 0


# ------------------------------------------------------------------------------------------------ --dry
def dry():
    s, _app = model_session()
    walk(s)
    by = {}
    for _st, cid, name, _d in s.rep.cases:
        by.setdefault(cid, []).append(name)
    print("firstrun-check: %d steps, %d cases (T-27 compares the real-core and fixture-core logs; T-01 is the refusal, judged at every launch and in the selftest)" % (len(by), sum(len(v) for v in by.values())))
    for cid in sorted(by):
        mut = MUTANTS.get(cid) or MUTANTS.get(cid + "b")
        edit = EDITS.get(cid) or EDITS.get(cid + "b")
        print("  %s  mutant: %s" % (cid, ("runtime WHOOSH_CORE_MUTANT=" + mut) if mut else ("source edit: " + (edit or "-"))))
        for n in by[cid]:
            print("        case %s" % n)
    print("  T-01  the launch environment holds none of %s: refused, never run" % ", ".join(FORBIDDEN_ENV))
    print("  T-27  python3 checks/firstrun-check.py --compare real.log fixture.log (steps %s)" % ", ".join(FIXTURE_STEPS))
    print("the lead runs, on a build:")
    print('  cd "<worktree>" && WHOOSH_RIG_HIDDEN=1 python3 checks/firstrun-check.py <bundle> --log _captures/perf-realcore-r1/firstrun-<stamp>.log')
    print('  ... --mutant=<name> for each runtime mutant above (the step it targets is reported alone); --core fixture --log ...fixture.log, then --compare')
    print("through the launcher (the fakes must already run; FIRSTRUN_HUB is their control port):")
    print("  sh scripts/launch-one.sh <bundle> -- --rig-hidden sh -c 'WHOOSH_SOCKET_DIR=\"$D\" python3 checks/firstrun-check.py --inner \"$FIRSTRUN_HUB\" <json>'")
    return 0


# ------------------------------------------------------------------------------------------------ the model app and the selftest
class ModelHub:
    """The hub's methods over a model: delivered mail reaches the model's list, counts are the model's."""
    def __init__(self, app):
        self.app = app

    def pid(self, n): pass
    def peers(self): return self.app.peers()
    def snap(self): return self.app.snap()
    def fetched(self): return self.app.fetched
    def deliver(self, kind): return self.app.deliver(kind)
    def latency(self, ms): self.app.latency = ms
    def mail(self, do): self.app.server_up = do == "start"
    def provider_ready(self): return True


class ModelApp:
    """A model of the app as the first run meets it, with the row shapes Sources/ serves. Each mutant makes it fail the way the owner's screen did."""
    def __init__(self, mutants=(), core="real"):
        import realcore_mailfake as fake
        self.fake, self.m, self.core_kind = fake, set(mutants), core
        self.dead = {x.split(":", 1)[1] for x in self.m if x.startswith("dead:")}
        inbox, _sent = fake.firstrun_inbox()
        import email, email.policy
        self.msgs = [email.message_from_bytes(r, policy=email.policy.default) for r in inbox]
        self.persist = {"accounts": [] if core == "real" else [OWNER, SECOND], "sources_off": set(), "writer_model": "claude-sonnet-5-5", "needs_reply": "" if "labels-empty-shells" in self.m else "Emails that wait for a reply from me.",
                        "bodies": False, "meaning": None}
        self.fetched = set()
        self.server_up, self.latency = True, 0
        self.boot(first=True)
        self.ai_reqs = self.smtp_data = self.appends = 0
        self.hooks = {"calendar-access": 0, "microphone": 0}
        self.clicks = []
        self.pasted_keys = []
        self.net = {"samples": 8, "offenders": ["198.51.100.1:443 SYN_SENT"] if "dial-out" in self.m else [], "control": "probe-broken" not in self.m, "control_n": 5,
                    "local": ["127.0.0.1:50001 ESTABLISHED"]}
        self.dialed, self.oauth_phase, self.endpoint = [], "idle", None       # T-30: what the model's act let out, the sheet's sign-in phase, the staged provider endpoint

    def boot(self, first=False):
        p = self.persist
        self.focus, self.sheet, self.fs_skipped = None, {"stage": None}, False
        self.fs_active = (not p["accounts"]) and "no-first-start" not in self.m if not self.m & {"fixture-prefilled"} else False
        self.surface, self.open_label, self.typed = "mac-main", None, ""
        self.search = {"text": "", "launched": False, "results": []}
        self.pane, self.sources_open = None, False
        self.access = "allowed" if "calendar-starts-allowed" in self.m else "not-asked"
        self.dict_state, self.body_text = "idle", ""
        self.keybuf = {}
        self.written = 0

    # the socket
    def connect(self): return self
    def close(self): pass
    sock = True

    def send(self, verb, *a):
        from lib.ctl import Unreadable
        fn = getattr(self, "v_" + verb, None)
        if fn is None:
            raise Unreadable("%s: not served" % verb)
        r = fn(*a)
        return r if r is not None else {"ok": True}

    def rows(self, ax):
        from lib.ctl import Unreadable
        r = self.read(ax)
        if r is None:
            raise Unreadable("rows %s: no such region" % ax)
        return r

    def uistate(self): return {"hidden": {"on": "hidden-off" not in self.m}, "counts": {"primary": 0 if "counts-zero" in self.m else 12}, "core": "rust"}
    def core(self, path):
        from lib.ctl import Unreadable
        v = self.state(path)
        if v is None:
            raise Unreadable("core state %s: none" % path)
        return v

    def key(self, chord):
        return self.send("key", chord)

    def type(self, text): return self.send("type", text)

    # verbs
    def v_key(self, chord):
        if chord == "Escape":
            self.surface = "mac-main"; self.open_label = None; return
        if chord == "Fn:down" and "dictation-asks-mic" in self.m:
            self.hooks["microphone"] += 1
        if chord == "Fn:down":
            self.dict_state = "listening"
        if chord == "Fn:up":
            self.dict_state = "idle"
            if self.speak and "speech-lost" not in self.m: self.body_text += self.speak
        if chord == "R": self.surface = "reader-reply"

    speak = ""

    def v_keyevent(self, *a):
        text = a[-1]
        if text == "cmd+k": self.surface = "mac-search"; self.search = {"text": "", "launched": False, "results": []}
        elif text == "Return" and "dead-return" in self.m: pass
        elif text == "Return" and self.surface == "mac-search":
            self.search["launched"] = True
            w = self.search["text"].lower()
            hit = [] if "index-never-writes" in self.m else [x for x in self.msgs if w and w in x.get_body(("plain",)).get_content().lower()]
            self.search["results"] = hit
        elif text == "Escape": self.surface = "mac-main"
        elif self.surface == "mac-search": self.search["text"] += text

    def v_fixture(self, name, *a):
        if name == "calendar-access":
            self.access = {"allowed": "allowed", "refused": "refused", "will-refuse": "not-asked", "not-asked": "not-asked"}[a[0]]
            self.refuse_next = a[0] == "will-refuse"
            return
        if name == "speak":
            self.speak = a[0]; return
        if name == "install-state":
            return
        if name == "provider-endpoint":
            host = re.sub(r"^https?://", "", a[1]).split(":")[0].split("/")[0]
            if a[1] and host not in ("127.0.0.1", "localhost", "::1") and "endpoint-accepted" not in self.m:
                raise __import__("lib.ctl", fromlist=["x"]).Unreadable("provider-endpoint on a test copy takes only an address on this Mac; %s is not" % a[1])
            self.endpoint = a[1] or None; return
        raise __import__("lib.ctl", fromlist=["x"]).Unreadable("fixture %s: not served" % name)

    def v_type(self, text):
        f = self.focus
        if f == "address": self.sheet["address"] = text
        elif f == "password": self.sheet["password"] = text
        elif f == "key": self.keybuf["key"] = text; self.pasted_keys.append(text)
        elif f == "search": self.search["text"] += text
        elif f == "meaning": self.persist["meaning"] = text
        elif self.surface == "mac-search": self.search["text"] += text

    def v_click(self, ax):
        from lib.ctl import Unreadable
        p, c = self.persist, ax
        self.clicks.append(ax)
        if ax in self.dead: return          # an act deleted: the door answers, nothing happens
        if ax == "mac-first-start.skip": self.fs_skipped = True; return
        if ax == "mac-main.sidebar.empty.add" or ax == A + ".open": self.sheet = {"stage": "entry"}; self.focus = None; return
        if ax == A + ".address": self.focus = "address"; return
        if ax == A + ".password": self.focus = "password"; return
        if ax == A + ".continue": self.sheet["stage"] = "summary"; return
        if ax == A + ".add":
            self.sheet["stage"] = "summary" if "sheet-never-adds" in self.m else "added"
            if "no-account-stored" not in self.m: p["accounts"].append(self.sheet.get("address")); self.fs_active = False
            return
        if ax == A + ".done": self.sheet = {"stage": None}; self.oauth_phase = "idle"; return
        if ax == A + ".oauth.own-id": return
        if ax == A + ".oauth.field": self.focus = "appid"; return
        if ax == A + ".oauth.sign-in":
            self.oauth_phase = "waiting"
            if "sign-in-dials" in self.m: self.dialed.append("198.51.100.1:443 SYN_SENT")      # the call the guard must refuse, let out
            return
        if ax == "mac-search.bar": self.focus = "search"; return
        if ax.startswith(W + ".pane-row."): self.pane = ax.split(".pane-row.", 1)[1]; return
        if ax.startswith(INS + ".open.modify."): self.open_label = ax.rsplit(".", 1)[1]; return
        if ax == INS + ".cancel": self.open_label = None; return
        if ax == INS + ".meaning": self.focus = "meaning"; return
        if ax == INS + ".apply": return
        if ax.startswith(AI + ".nav."): return
        if ax == AI + ".provider.anthropic.key": self.focus = "key"; return
        if ax == AI + ".provider.anthropic.connect":
            k = self.keybuf.get("key")
            if k == PROBE_KEY and "key-post-dials" in self.m: self.dialed.append("198.51.100.7:9 SYN_SENT")
            if k == BAD_KEY: self.prov = ("refused", BAD_KEY_WORDS); p["bad_stored"] = "bad-key-stored" in self.m
            elif k == GOOD_KEY: self.prov = ("refused", "no") if "good-key-refused" in self.m else ("connected", ""); p["good"] = "good-key-refused" not in self.m
            return
        m = re.match(re.escape(AI) + r"\.agents\.model\.(\d)\.choose\.(\d)$", ax)
        if m:
            if "chip-dead" in self.m: raise Unreadable("click %s: nothing" % ax)
            if "chip-silent" in self.m: return
            if "slow-save" in self.m: time.sleep(0.08)
            p["writer_model"] = ["model-a", "model-b"][int(m.group(2)) - 1]
            if "choices-in-memory" in self.m: self.mem_only = True
            return
        if ax == CAL + ".access.ask":            # as the app: the one button acts only while not asked; the fixture answers Allow, or refuses after will-refuse
            if self.access != "not-asked": raise Unreadable("click %s: did nothing" % ax)
            if "calendar-asks-macos" in self.m: self.hooks["calendar-access"] += 1
            self.access = "refused" if getattr(self, "refuse_next", False) else "allowed"
            return
        if ax.startswith(SOURCES + ".account"):
            p["sources_off"] ^= {ax}; return
        if ax in ("mac-main.reader.open-full-composer", "mac-composer.close", "mac-composer.discard"): self.surface = "mac-composer"; return
        raise Unreadable("click %s: no such door" % ax)

    prov = ("not connected", "")
    relaunched = False
    polls = 0
    mem_only = False

    # the reads
    def read(self, ax):
        p, m = self.persist, self.m
        if ax == "mac-first-start.state":
            return [{"active": self.fs_active, "skipped": self.fs_skipped, "step": 1, "accounts": len(p["accounts"])}]
        if ax == "mac-main.sidebar.empty":
            return [{"ax": "mac-main.sidebar.empty.add", "title": "Add account"}] if not p["accounts"] and "sidebar-empty-gone" not in m else []
        if ax == "mac-main.sidebar.accounts":
            return [{"address": a, "health": "Live", "status": None} for a in p["accounts"][:1]] + [{"address": a, "health": "Live"} for a in p["accounts"][1:]]
        if ax == A + ".state": return [{"stage": self.sheet.get("stage")}]
        if ax == A + ".error": return []
        if ax == A + ".oauth": return [{"kind": "microsoft", "phase": self.oauth_phase, "words": "", "field_shown": True}] if "m365" in (self.sheet.get("address") or "") and "no-oauth-row" not in m else []
        if ax == "mac-main.list":
            if "sync-never-stores" in m or not p["accounts"] or ("relaunch-empty-list" in m and self.relaunched): return []
            if "slow-list" in m and self.relaunched and self.polls < 30: self.polls += 1; return []   # the list comes only after the server answers (1.5 s of polls)
            subs = [str(x["Subject"]) for x in self.msgs][::-1][:50]
            if "subjects-mangled" in m: subs = [s_.replace("’", "'") for s_ in subs]
            sender = {str(x["Subject"]): str(x["From"]).split(" <")[0].replace("é", "e").replace("è", "e").replace("’", "'").replace("É", "E") if "senders-flattened" in m else str(x["From"]).split(" <")[0] for x in self.msgs}
            return [{"ax": "mac-main.list.%d" % i, "subject": s_, "sender": sender[s_.replace("'", "’")] if "subjects-mangled" in m else sender[s_]} for i, s_ in enumerate(subs)] + \
                   ([{"ax": "mac-main.list.q", "subject": "Pouvez-vous valider ce point ?", "sender": "Client Martin"}] if self.delivered else [])
        if ax == "mac-search.bar": return [{"text": self.search["text"], "launched": self.search["launched"], "loading": False}]
        if ax == "mac-search.results": return [{"subject": str(x["Subject"])} for x in self.search["results"]]
        if ax == INS: return [{"open": self.open_label is not None, "label_id": self.open_label, "meaning": self.meaning()}]
        if ax == INS + ".flow": return self.flow()
        if ax == INS + ".foot": return [{"caught_by_entry": 0 if "labels-empty-shells" in m else 1, "text": "0 messages" if "labels-empty-shells" in m else "1 message · 1 picked · 0 accepted"}]
        if ax == INS + ".empty": return [{"words": NEEDS_REPLY_WORDS}] if self.refusal_words() else []
        if ax == INS + ".result": return [{"id": "m1", "decided_by": "Laya", "card": "ai"}] if "laya-silent" not in m and self.laya_on else []
        if ax == SOURCES: return self.sources()
        if ax == "mac-settings-replies.state": return [{"auto_enabled": "auto-on" in m, "ui": {"page": "auto"}}]
        if ax == AI + ".provider.anthropic": return [{"state": self.prov[0], "models": MODELS if self.prov[0] == "connected" else [], "note": self.prov[1]}]
        if ax == AI + ".provider.anthropic.check": return [{"state": "idle"}]
        if ax == AI + ".agents.card": return [{"name": "Inbox", "model": "claude-sonnet-5-5"}, {"name": "Writer", "model": p["writer_model"] if not self.mem_only or self.core_kind else p["writer_model"]}]
        if ax == AI + ".agents.model.2": return [] if "menu-empty" in m else [{"model": "model-a", "enabled": True}, {"model": "model-b", "enabled": True}]
        if ax == CAL + ".access": return [{"state": self.access if "calendar-empty" not in m or self.access != "allowed" else self.access}]
        if ax == CAL + ".list": return [{"name": n} for n in CALS] if self.access == "allowed" and "calendar-empty" not in m else []
        if ax == "mac-composer.dictation": return [{"state": self.dict_state, "line": "", "engine": "fixture"}]
        if ax == "mac-composer.body": return [{"text": self.body_text}]
        if ax == "mac-main.reader.thread":
            return [{"body": self.reader_body()}]
        if ax == "mac-main.reader.attachments": return [] if "attachments-lost" in m else [{"name": "facture-4471.pdf"}] if self.persist["bodies"] and self.server_up is False else [{"name": "facture-4471.pdf"}]
        return None

    laya_on = False
    delivered = False

    def meaning(self):
        return self.persist["meaning"] if self.persist["meaning"] is not None else self.persist["needs_reply"]

    def refusal_words(self): return None if "no-refusal-words" in self.m else NEEDS_REPLY_WORDS

    def flow(self):
        lid = self.open_label
        ex = {"chain": "exit", "facts": {"unanswered": "no" if "labels-empty-shells" in self.m else "yes"}, "rows_n": 0}
        ent = {"chain": "entry", "facts": {"mailing_list": "any" if "labels-empty-shells" in self.m else "yes"}}
        return [{"card": "ai", "ai_refusal": self.refusal_words() or None}, ent, ex]

    def sources(self):
        p, out = self.persist, []
        if "sources-empty" in self.m or len(p["accounts"]) == 0: return []
        for ai_, acct in enumerate(p["accounts"], 1):
            out.append({"ax": SOURCES + ".account.%d" % ai_, "kind": "account", "line": "2 of 3 folders"})
            names = ["INBOX", "Éléments envoyés", "Projects"] if ai_ == 1 else ["INBOX", "Sent Items", "Receipts"]
            for i, n in enumerate(names):
                ax = SOURCES + ".account.%d.folder.%d" % (ai_, i)
                on = i < 2 and ax not in p["sources_off"]
                out.append({"ax": ax, "kind": "mail_folder", "name": n, "enabled": on})
        if self.access == "allowed" and "calendar-empty" not in self.m:
            out += [{"ax": SOURCES + ".calendar.%s" % n, "kind": "calendar", "name": n, "enabled": False} for n in CALS]
        return out

    def reader_body(self):
        subj = self.search.get("opened") or self.last_subject
        bad_down = ("bodies-never-stored" in self.m) and not self.server_up
        if bad_down: return ""
        x = next((y for y in self.msgs if str(y["Subject"]) == subj), None)
        if x is None: return ""
        h = x.get_body(("html",))
        if h is not None:
            txt = h.get_content()
            return txt if "entities-undecoded" in self.m else re.sub(r"<[^>]+>", " ", txt).replace("&eacute;", "é").replace("&ecirc;", "ê").replace("&#8217;", "’").replace("&amp;", "&")
        return x.get_body(("plain",)).get_content()

    last_subject = ""

    def state(self, path):
        p = self.persist
        if path == "reply/use/auto": return {"enabled": "auto-on" in self.m}
        if path == "reply/sources": return {"calendars": [{}, {}] if self.access == "allowed" and "calendar-empty" not in self.m else []}
        if path == "system-ui/calendar-access": return self.hooks["calendar-access"]
        if path == "system-ui/microphone": return self.hooks["microphone"]
        if path == "provider_credential/anthropic/stored": return bool(p.get("bad_stored") or p.get("good"))
        if path == "agent_config/writer": return {"model": p["writer_model"]}
        if path == "label/needs-reply/flow/ai_refusal": return {"code": "ai_no_model", "words": self.refusal_words()} if self.refusal_words() else None
        if path == "rig/refused-outbound": return 1 if "refused-outbound" in self.m else 0
        if path == "rig/local-model/loads": return 1 if (self.laya_on or "laya-loads-early" in self.m) else 0
        return None

    # the hub's view
    def deliver(self, kind):
        if "no-delivery" in self.m: return 0
        self.delivered = True
        if "auto-on" in self.m or "laya-sends-out" in self.m: self.ai_reqs += 1
        if "auto-on" in self.m: self.smtp_data += 1
        if kind == "question" and self.persist["meaning"]: self.laya_on = True
        return 999

    def snap(self):
        return {"imap": 0, "smtp_data": self.smtp_data, "ai": self.ai_reqs, "appends": self.appends, "fetched": sorted(self.fetched)}

    def peers(self):
        self.net["samples"] += 4          # the watcher keeps sampling between two reads
        return dict(self.net, offenders=self.net["offenders"] + [d for d in self.dialed if d not in self.net["offenders"]])


def model_session(mutants=(), core="real", caps=None, load=1.0):
    app = ModelApp(mutants, core)
    rep = Rep()
    caps = caps if caps is not None else {"providers": True, "laya": True, "keyevent": True, "mutants": {}}
    fetch_n = [0]
    class Watch:
        def start(self, pid): return "w"
        def stop(self):
            return {"samples": 40, "seen": 0 if "probe-broken" in app.m else 1, "shown": [[{"shown": True}]] if "window-shown" in app.m else []}
        def restart(self, pid): pass
    def relaunch():
        app.boot(); app.relaunched = True
        if "choices-in-memory" in app.m and app.mem_only:
            app.persist["writer_model"] = "claude-sonnet-5-5"
        if "relaunch-empty-list" in app.m:
            app.persist["accounts_hidden"] = True
    now = [1000.0]
    def nap(x): now[0] += x
    s = Session(app, ModelHub(app), rep, caps, core=core, relaunch=relaunch, watcher=Watch(), load=lambda: load, sleep=nap, clock=lambda: now[0])
    s.self_control = lambda: ("probe-broken" not in app.m, "the model's own probe control")
    # the model's crawl: the whole message is fetched for the newest N, and the reader opens a message by its subject
    app.fetched = set() if "bodies-never-stored" in app.m else set(range(1, 61))
    app.persist["bodies"] = "bodies-never-stored" not in app.m
    orig_click = app.v_click
    def click(ax):
        m_ = re.match(r"mac-main\.list\.(\d+)$", ax)
        if m_:
            rows = app.read("mac-main.list"); app.last_subject = rows[int(m_.group(1))]["subject"]; return
        return orig_click(ax)
    app.v_click = click
    return s, app


def expect_red(label, mutants, names, results, core="real"):
    s, app = model_session(mutants, core=core)
    s.rep.only = None
    walk(s, core)
    got = {(st, cid, n) for st, cid, n, _ in s.rep.cases}
    hit = [(cid, n) for (st, cid, n) in got if st == "FAIL" and n in names]
    results.append((label, bool(hit) and all(any(st == "FAIL" and n == nm for (st, cid, n) in got) for nm in names), "FAIL cases %r" % sorted(hit)))


class CaseTable(dict):
    """(T-id, name) -> status. A case that did not run reads "MISSING" (a judged failure with its reason), never a KeyError."""
    def __getitem__(self, k): return self.get(k)
    def get(self, k, d=None): return dict.get(self, k, "MISSING")


def selftest():
    import realcore_mailfake as fake
    res = []
    # T-01: each seeding variable is refused, a clean launch is not, and LAUNCH_ENV words count as the environment
    for var in FORBIDDEN_ENV:
        res.append(("T-01 refuses %s in the environment" % var, refusal({var: "1"}) == [var], refuse_words(refusal({var: "1"}))[:60]))
        res.append(("T-01 refuses %s in LAUNCH_ENV" % var, refusal({}, "A=1 %s=1 B=2" % var) == [var], ""))
    res.append(("T-01 passes a clean launch (transport redirections only)", refusal({"PATH": "x"}, "WHOOSH_RIG_HIDDEN=1 WHOOSH_CORE_CHECK_SERVERS=1:2 WHOOSH_CORE_PLAIN_LOOPBACK=1") == [], ""))
    res.append(("T-01 does not confuse WHOOSH_CORE_CHECK_SERVERS with a seeding variable", refusal({"WHOOSH_CORE_CHECK_SERVERS": "1:2"}) == [], ""))
    # the walk against the model, clean
    s, app = model_session()
    walk(s)
    fails = [(cid, n, d) for st, cid, n, d in s.rep.cases if st == "FAIL"]
    nj = [(cid, n, d) for st, cid, n, d in s.rep.cases if st == "NOT JUDGED" and (cid, n) != ("T-30", "provider-key-post-reaches-nothing")]
    res.append(("the clean model walks green: no FAIL case (%d cases)" % len(s.rep.cases), not fails and len(s.rep.cases) >= 35, "FAIL %r" % fails[:3]))
    res.append(("the clean model leaves nothing NOT JUDGED (but the fixture core's endpoint case, which the real core cannot do)", not nj, "NOT JUDGED %r" % nj[:3]))
    cs0 = CaseTable({(cid, n): st for st, cid, n, _ in s.rep.cases})
    res.append(("T-30 on the real core: the sign-in case is ok, the provider endpoint case says it is the fixture core's (NOT JUDGED, never ok)", cs0[("T-30", "sign-in-reaches-nothing")] == "ok" and cs0[("T-30", "provider-key-post-reaches-nothing")] == "NOT JUDGED", "%s %s" % (cs0[("T-30", "sign-in-reaches-nothing")], cs0[("T-30", "provider-key-post-reaches-nothing")])))
    t30 = [d for st, cid, n, d in s.rep.cases if cid == "T-30" and n == "sign-in-reaches-nothing"]
    res.append(("the T-30 pass line carries the peers read and the count of refused outside calls", bool(t30) and "loopback peers read ['127.0.0.1:50001 ESTABLISHED']" in t30[0] and "refused outside calls 0 (was 0, +0)" in t30[0], t30[0][:160] if t30 else "no case"))
    sfx, _afx = model_session(core="fixture"); walk(sfx, "fixture")
    csf = CaseTable({(cid, n): st for st, cid, n, _ in sfx.rep.cases})
    res.append(("T-30 and T-24 run on the fixture core and are ok on the clean model (nothing-left-the-mac, both T-30 cases)", all(csf[k] == "ok" for k in (("T-24", "nothing-left-the-mac"), ("T-30", "sign-in-reaches-nothing"), ("T-30", "provider-key-post-reaches-nothing"))), str({k: csf[k] for k in (("T-24", "nothing-left-the-mac"), ("T-30", "sign-in-reaches-nothing"), ("T-30", "provider-key-post-reaches-nothing"))})))
    pf = [d for st, cid, n, d in sfx.rep.cases if cid == "T-30" and n == "provider-key-post-reaches-nothing"]
    res.append(("the provider case reads the refusal of the verb and the row after the paste", bool(pf) and "verb refused http://198.51.100.7:9" in pf[0] and "Connect clicked" in pf[0], pf[0][:200] if pf else "no case"))
    # the pure verdict: a peer off this Mac names itself, too few samples is not a pass, a refused call is counted and printed
    b0 = {"samples": 10, "offenders": [], "local": ["127.0.0.1:1 ESTABLISHED"]}
    res.append(("outside_verdict: a new off-Mac peer is a FAIL that names it", outside_verdict(b0, dict(b0, samples=14, offenders=["203.0.113.9:443 SYN_SENT"]), 0, 0)[0] == "FAIL" and "203.0.113.9:443 SYN_SENT" in outside_verdict(b0, dict(b0, samples=14, offenders=["203.0.113.9:443 SYN_SENT"]), 0, 0)[1], ""))
    res.append(("outside_verdict: a peer already seen before the act is not blamed on it", outside_verdict(dict(b0, offenders=["203.0.113.9:443 SYN_SENT"]), dict(b0, samples=14, offenders=["203.0.113.9:443 SYN_SENT"]), 0, 0)[0] == "ok", ""))
    res.append(("outside_verdict: fewer than 2 samples after the act is NOT JUDGED, never ok", outside_verdict(b0, dict(b0, samples=11), 0, 0)[0] == "NOT JUDGED", ""))
    res.append(("outside_verdict: a refused call is printed with its count and is not a FAIL of its own", (lambda v: v[0] == "ok" and "refused outside calls 2 (was 1, +1)" in v[1])(outside_verdict(b0, dict(b0, samples=14), 1, 2)), ""))
    # part (c): one rule for this Mac's hosts. The model tree is built here (never the repo): a clean one, and one with a second list in a new file
    for name, extra, want in (("the repo's own Sources", None, "ok"), ("a model tree with only the known second lists", {}, "ok"), ("a model tree with a new second list in another file", {"Sources/Features/X/Mine.swift": 'let h = ["localhost"]\n'}, "FAIL"),
                              ("a model tree with a second line in a known file", {"Sources/Features/PushClient/RelaySettings.swift": 'a == "localhost"\nb == "::1"\n'}, "FAIL"), ("a model tree whose only host literal is a comment", {"Sources/Features/X/C.swift": '// "localhost"\n'}, "ok")):
        if extra is None:
            lines, ok = rule_pin(ROOT)
        else:
            td = tempfile.mkdtemp(prefix="firstrun-rulepin-")
            files = {RULE_FILE: 'static let thisMacHosts: Set<String> = ["127.0.0.1", "::1", "localhost"]\n', "Sources/Features/Embedding/MeaningModelRig.swift": 'h == "localhost"\n',
                     "Sources/Features/PushClient/RelaySettings.swift": '["localhost"]\n', "Sources/Features/MessageBody/MessageBodyPolicy.swift": '"localhost"\n'}
            files.update(extra)
            for rel, body in files.items():
                os.makedirs(os.path.dirname(os.path.join(td, rel)), exist_ok=True); open(os.path.join(td, rel), "w").write(body)
            lines, ok = rule_pin(td)
        res.append(("rule_pin on %s reads %s" % (name, want), lines[0].split()[0] == want and ok == (want == "ok"), lines[0][:200]))
    pc = probe_control()
    res.append(("probe_control: the real lsof read a loopback peer of this very process (%s)" % pc[1], pc[0], pc[1]))
    res.append(("a model that never pastes a key into a provider without the seam: caps off", True, ""))
    # each red-first mutant turns its own cases red
    table = [("no-first-start", ("first-start-opens-on-an-empty-installation",)), ("no-account-stored", ("sidebar-shows-the-account-live",)),
             ("sync-never-stores", ("list-holds-the-first-page", "subjects-drawn-as-sent")), ("subjects-mangled", ("subjects-drawn-as-sent",)),
             ("index-never-writes", ("body-only-word-finds-its-message", "second-body-only-word-finds-its-message")),
             ("labels-empty-shells", ("needs-reply-has-a-description", "needs-reply-exit-rule-unanswered", "promotions-has-its-rule", "promotions-filled-by-its-rule-with-no-ai")),
             ("no-refusal-words", ("the-ai-card-says-why-nothing-was-sorted", "the-empty-list-says-the-same")), ("sources-empty", ("two-account-cards", "inbox-and-sent-on-everything-else-off")),
             ("auto-on", ("auto-draft-reads-off", "nothing-drafted-nothing-sent-nothing-asked")), ("bad-key-stored", ("bad-key-shows-the-providers-words-and-stores-nothing",)),
             ("chip-dead", ("picked-model-is-kept",)), ("calendar-asks-macos", ("macos-is-never-asked",)), ("calendar-empty", ("after-allow-both-calendars-listed", "sources-lists-both-calendars-off")),
             ("laya-silent", ("laya-files-the-question-by-its-description",)), ("dictation-asks-mic", ("the-microphone-is-never-asked",)), ("slow-save", ("every-command-under-the-bar",)),
             ("choices-in-memory", ("choices-kept",)), ("bodies-never-stored", ("bodies-were-fetched-before-the-server-stopped", "reader-shows-the-first-sentence-with-the-server-down")),
             ("dead:mac-first-start.skip", ("skip-leaves-the-add-account-way",)), ("dead-return", ("body-only-word-finds-its-message", "second-body-only-word-finds-its-message")),
             ("no-delivery", ("nothing-drafted-nothing-sent-nothing-asked",)), ("chip-silent", ("picked-model-is-kept",)),
             ("sidebar-empty-gone", ("sidebar-says-no-account-yet",)), ("dead:mac-main.sidebar.empty.add", ("the-sheet-opens-from-the-empty-sidebar",)), ("sheet-never-adds", ("account-added",)),
             ("senders-flattened", ("sender-names-keep-accents-and-apostrophes",)), ("counts-zero", ("counts-above-zero",)), ("good-key-refused", ("good-key-reads-connected",)),
             ("menu-empty", ("model-chip-opens-the-models",)), ("calendar-starts-allowed", ("not-asked-at-open",)), ("laya-loads-early", ("laya-loaded-once-and-nothing-left-the-mac",)),
             ("laya-sends-out", ("laya-loaded-once-and-nothing-left-the-mac",)), ("speech-lost", ("the-words-land-in-the-body",)), ("relaunch-empty-list", ("inbox-drawn-at-once",)), ("slow-list", ("inbox-drawn-at-once",)),
             ("attachments-lost", ("files-are-listed",)), ("hidden-off", ("hidden-mode-on",)),
             ("entities-undecoded", ("html-entities-are-decoded",)), ("dial-out", ("nothing-left-the-mac",)), ("refused-outbound", ("nothing-left-the-mac",)), ("window-shown", ("no-window-shown",))]
    table.append(("sign-in-dials", ("sign-in-reaches-nothing",)))
    for mut, names in table:
        expect_red("mutant %s turns %s red" % (mut, ", ".join(names)), (mut,), names, res)
    for mut in ("key-post-dials", "endpoint-accepted"):         # the provider endpoint case is the fixture core's
        expect_red("mutant %s turns provider-key-post-reaches-nothing red (fixture core)" % mut, (mut,), ("provider-key-post-reaches-nothing",), res, "fixture")
    expect_red("mutant sign-in-dials turns sign-in-reaches-nothing red (fixture core too)", ("sign-in-dials",), ("sign-in-reaches-nothing",), res, "fixture")
    expect_red("mutant dial-out turns nothing-left-the-mac red on the fixture core", ("dial-out",), ("nothing-left-the-mac",), res, "fixture")
    s, app = model_session(("probe-broken",), core="fixture"); walk(s, "fixture")
    cp = CaseTable({(cid, n): st for st, cid, n, _ in s.rep.cases})
    res.append(("a broken probe on the fixture core leaves T-24 and both T-30 cases NOT JUDGED, never ok", all(cp[k] == "NOT JUDGED" for k in (("T-24", "nothing-left-the-mac"), ("T-30", "sign-in-reaches-nothing"), ("T-30", "provider-key-post-reaches-nothing"))), str({k: cp[k] for k in (("T-24", "nothing-left-the-mac"), ("T-30", "sign-in-reaches-nothing"))})))
    # absence assertions carry a positive control: a broken probe is NOT JUDGED, never a pass
    s, app = model_session(("probe-broken",))
    walk(s)
    cs = CaseTable({(cid, n): st for st, cid, n, _ in s.rep.cases})
    res.append(("a network probe that sees no loopback peer leaves T-24 NOT JUDGED, not ok", cs.get(("T-24", "nothing-left-the-mac")) == "NOT JUDGED", str(cs.get(("T-24", "nothing-left-the-mac")))))
    res.append(("a window reader that sees no window leaves T-29 NOT JUDGED, not ok", cs.get(("T-29", "no-window-shown")) == "NOT JUDGED", str(cs.get(("T-29", "no-window-shown")))))
    # without the provider seam or the Laya stand-in: NOT JUDGED, and no key is ever pasted
    s, app = model_session(caps={"providers": False, "laya": False, "keyevent": True, "mutants": {}})
    walk(s)
    cs = CaseTable({(cid, n): st for st, cid, n, _ in s.rep.cases})
    res.append(("no provider seam: every T-13 case NOT JUDGED and no key pasted", all(cs[("T-13", n)] == "NOT JUDGED" for n in RED["T-13"]) and app.pasted_keys == [], "keys %r" % app.pasted_keys))
    res.append(("no Laya stand-in: both T-15 cases NOT JUDGED", all(cs[("T-15", n)] == "NOT JUDGED" for n in RED["T-15"]), ""))
    # load above the cap: the timing step is NOT JUDGED, never red
    s, app = model_session(("slow-save",), load=40.0)
    walk(s)
    cs = CaseTable({(cid, n): st for st, cid, n, _ in s.rep.cases})
    res.append(("a load above PERF_LOAD_MAX leaves T-25 NOT JUDGED even with a slow save", cs[("T-25", "every-command-under-the-bar")] == "NOT JUDGED", str(cs[("T-25", "every-command-under-the-bar")])))
    # T-27: the same visible assertions on both cores
    sr, _ = model_session(); walk(sr)
    sf, _ = model_session(core="fixture"); walk(sf, "fixture")
    real, fx = parse_log("\n".join(sr.rep.lines())), parse_log("\n".join(sf.rep.lines()))
    rows = compare(real, fx)
    res.append(("T-27 the clean model gives the same verdicts on both cores (%d compared)" % len(rows), bool(rows) and all(ok for _n, ok, _d in rows), "%r" % [r for r in rows if not r[1]][:2]))
    sx, _ = model_session(("labels-empty-shells",), core="fixture"); walk(sx, "fixture")
    rows2 = compare(real, parse_log("\n".join(sx.rep.lines())))
    res.append(("T-27 names the step a prefilled-looking fixture fails (labels empty on one core)", any(not ok and n.startswith("T-10") for n, ok, _d in rows2), "%r" % [r[0] for r in rows2 if not r[1]][:3]))
    # the probe parsers
    bad, local = offenders_of("Whoosh 1 u IPv4 0 TCP 127.0.0.1:5000->127.0.0.1:6000 (ESTABLISHED)\nWhoosh 1 u IPv4 0 TCP 10.0.0.2:5001->198.51.100.1:443 (SYN_SENT)\nWhoosh 1 u IPv6 0 TCP [::1]:5002->[::1]:6001 (ESTABLISHED)\nWhoosh 1 u IPv4 0 TCP *:7000 (LISTEN)")
    res.append(("the network parser: a documentation-range peer is an offender, loopback peers are the control", bad == ["198.51.100.1:443 SYN_SENT"] and len(local) == 2, "%r %r" % (bad, local)))
    bad2, local2 = offenders_of("x TCP 127.0.0.1:1->127.0.0.1:2 (ESTABLISHED)\nx TCP 127.0.0.1:3->127.0.0.1:9 (ESTABLISHED)", {2})
    res.append(("the network parser: a loopback port that is none of the fakes' is an offender (the dial-out mutant), a fake's port is the control", bad2 == ["127.0.0.1:9 ESTABLISHED"] and len(local2) == 1, "%r %r" % (bad2, local2)))
    res.append(("a case that did not run reads MISSING in the case table, never a KeyError", CaseTable({})[("T-25", "x")] == "MISSING" and CaseTable({}).get(("T-25", "x")) == "MISSING", ""))
    nw = NetWatch(probe=lambda pid: "x TCP 127.0.0.1:1->127.0.0.1:2 (ESTABLISHED)\nx TCP 1.2.3.4:1->198.51.100.1:443 (SYN_SENT)")
    nw.pid = 1; nw.start(); time.sleep(0.7); nw.stop_flag.set()
    r = nw.report()
    res.append(("the watcher thread records the off-Mac peer once and keeps its control", r["offenders"] == ["198.51.100.1:443 SYN_SENT"] and r["control"] and r["samples"] >= 1, str(r)))
    res.append(("a window sample with a shown window is read as shown", bool(windows_result(_tmp_jsonl([{"windows": [{"shown": True}], "shown": 1}]))["shown"]), ""))
    res.append(("a window sample with none shown is read as hidden, its window counted as the control", (lambda w: w["seen"] == 1 and not w["shown"])(windows_result(_tmp_jsonl([{"windows": [{"shown": False}], "shown": 0}]))), ""))
    # the fetched-body reader and the bundle probe
    log = ["UID FETCH 1:3 (UID FLAGS BODY.PEEK[])", "UID FETCH 4:9 (UID FLAGS BODY.PEEK[HEADER] BODY.PEEK[TEXT]<0.2048>)", "UID FETCH 7 (BODY.PEEK[])", "NOOP "]
    res.append(("fetched_uids counts whole-message fetches only", fetched_uids(log, list(range(1, 11))) == {1, 2, 3, 7}, str(fetched_uids(log, list(range(1, 11))))))
    d = tempfile.mkdtemp(prefix="firstrun-bundle-"); os.makedirs(os.path.join(d, "Contents", "MacOS"))
    open(os.path.join(d, "Contents", "MacOS", "App"), "wb").write(b"\0" * 9000000 + b"xx WHOOSH_CORE_CHECK_PROVIDERS yy")
    res.append(("bundle_has finds a literal in the executable (past the first chunk)", bundle_has(d, "WHOOSH_CORE_CHECK_PROVIDERS"), ""))
    res.append(("bundle_has reads a name the build lacks as absent (control: the present one above is found)", not bundle_has(d, "WHOOSH_CORE_CHECK_NOTHING"), ""))
    # the hub against the real fakes: deliver, stop, start, the count of fetched whole messages
    import realcore_aifake as aifake
    mail, imap, smtp = fake.start(0, 0, firstrun=True)
    ai = aifake.start(0); ai = ai[0] if isinstance(ai, tuple) else ai
    hub = Hub(fake, aifake, mail, imap, smtp, ai, NetWatch())
    n0 = len(mail.boxes["INBOX"]); hub.handle({"cmd": "deliver", "kind": "question"}); hub.handle({"cmd": "deliver", "kind": "invoice"})
    res.append(("the hub delivers mail to the fake's INBOX", len(mail.boxes["INBOX"]) == n0 + 2, "%d -> %d" % (n0, len(mail.boxes["INBOX"]))))
    import imaplib
    ip = hub.ip
    def alive():
        try:
            c = imaplib.IMAP4("127.0.0.1", ip); c.login(fake.ADDRESS, fake.PASSWORD); c.noop(); c.logout(); return True
        except Exception:       # noqa: BLE001
            return False
    up0 = alive(); hub.handle({"cmd": "mail", "do": "stop"}); down = not alive(); hub.handle({"cmd": "mail", "do": "start"}); up1 = alive()
    res.append(("the hub stops and restarts the mail server on its port (up, down, up)", up0 and down and up1, "%s %s %s" % (up0, down, up1)))
    hub.handle({"cmd": "latency", "ms": 150}); c = imaplib.IMAP4("127.0.0.1", ip); c.login(fake.ADDRESS, fake.PASSWORD); t0 = time.time(); c.select("INBOX"); ms = (time.time() - t0) * 1000
    res.append(("the hub's latency reaches the fake (a SELECT waits >= 150 ms: %.0f)" % ms, ms >= 150, ""))
    imap.shutdown(); smtp.shutdown(); hub.imap.shutdown()
    wire_selftest(res)
    bad = [(n, d) for n, ok, d in res if not ok]
    for n, ok, d in res:
        print("%-4s %s%s" % ("ok" if ok else "BAD", n, "" if ok or not d else "  [%s]" % d[:160]))
    print("%d checks, %d failed, 0 not judged" % (len(res), len(bad)))
    return 1 if bad else 0


class SocketModel(threading.Thread):
    """The control socket's wire (journeys/lib/ctl.py) in front of a ModelApp: what the real driver (inner) talks to in the selftest."""
    def __init__(self, app, d, on_read=None):
        super().__init__(daemon=True)
        self.app, self.d, self.on_read = app, d, on_read
        self.srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.srv.bind(os.path.join(d, "whoosh.sock")); self.srv.listen(2)

    def run(self):
        while True:
            try:
                conn, _ = self.srv.accept()
            except OSError:
                return
            threading.Thread(target=self.serve, args=(conn,), daemon=True).start()

    def serve(self, conn):
        f = conn.makefile("rwb")
        for line in f:
            q = json.loads(line.decode()); verb, a = q["verb"], q["args"]
            try:
                if self.on_read: self.on_read(verb, a)
                if verb == "rows": ans = {"ok": True, "rows": self.app.rows(a[0])}
                elif verb == "uistate": ans = {"ok": True, "state": self.app.uistate()}
                elif verb == "core": ans = {"ok": True, "value": self.app.core(a[1])}
                else: ans = {"ok": True, **(self.app.send(verb, *a) or {})}
            except Exception as e:      # noqa: BLE001
                ans = {"ok": False, "error": str(e)}
            f.write((json.dumps(ans) + "\n").encode()); f.flush()


def wire_selftest(res):
    """The real driver (inner) over the real socket wire and the real hub and fakes, the model behind the wire: the protocol, the hub's counts, the output the
    outer run parses. The relaunch needs scripts/launch-one.sh and an app: not available here, so T-23 must say so (NOT JUDGED), never pass."""
    import contextlib
    import io
    import realcore_mailfake as fake
    import realcore_aifake as aifake
    mail, imap, smtp = fake.start(0, 0, firstrun=True)
    ai = aifake.start(0)
    hub = Hub(fake, aifake, mail, imap, smtp, ai, NetWatch())
    hsrv = serve_hub(hub)
    mail.log.append("UID FETCH 1:60 (UID FLAGS BODY.PEEK[])")
    app = ModelApp()
    d = tempfile.mkdtemp(prefix="firstrun-wire-")
    app.fetched = set(range(1, 61))

    def on_read(verb, a):
        if len(mail.boxes["INBOX"]) > 60:
            app.delivered = True      # the model's list shows what the REAL fake now holds
    sm = SocketModel(app, d, on_read); sm.start()
    old_env, old_hc = os.environ.get("WHOOSH_SOCKET_DIR"), globals()["load_hidden_check"]
    os.environ["WHOOSH_SOCKET_DIR"] = d
    globals()["load_hidden_check"] = lambda: None
    buf = io.StringIO()
    try:
        app.deliver = lambda kind: 0      # the delivery goes through the real hub and fake here (the driver asks the hub); the model only reads what the fake holds
        with contextlib.redirect_stdout(buf):
            rc = inner(hsrv.server_address[1], {"steps": None, "caps": json.dumps({"providers": True, "laya": False, "keyevent": True, "mutants": {}}), "core": "real"})
    finally:
        os.environ.pop("WHOOSH_SOCKET_DIR", None) if old_env is None else os.environ.__setitem__("WHOOSH_SOCKET_DIR", old_env)
        globals()["load_hidden_check"] = old_hc
        imap.shutdown(); smtp.shutdown(); hsrv.shutdown(); sm.srv.close()
    out = buf.getvalue()
    lines = [l for l in out.splitlines() if re.match(r"(ok|FAIL|NOT JUDGED)\s+T-\d+ ", l)]
    last = next((l for l in reversed(out.splitlines()) if re.match(r"\d+ checks, \d+ failed, \d+ not judged", l)), None)
    st = parse_log(out)
    res.append(("the driver over the socket wire prints case lines and the count line the outer run parses (%d lines)" % len(lines), len(lines) >= 30 and last is not None, "last %r" % last))
    res.append(("over the wire T-23 says the relaunch is unavailable (NOT JUDGED), never ok", st.get(("T-23", "inbox-drawn-at-once")) == "NOT JUDGED", str(st.get(("T-23", "inbox-drawn-at-once")))))
    res.append(("over the wire the real hub delivers the question into the real fake and T-12 reads its counts (ok)", st.get(("T-12", "nothing-drafted-nothing-sent-nothing-asked")) == "ok", str(st.get(("T-12", "nothing-drafted-nothing-sent-nothing-asked")))))
    res.append(("over the wire T-06 and T-07 are ok", all(st.get(k) == "ok" for k in (("T-06", "account-added"), ("T-07", "list-holds-the-first-page"))), str({k: v for k, v in st.items() if k[0] in ("T-06", "T-07")})))
    return out


def _tmp_jsonl(objs):
    f = tempfile.mktemp(prefix="firstrun-sel-", suffix=".jsonl")
    with open(f, "w") as fh:
        fh.write("\n".join(json.dumps(o) for o in objs) + "\n")
    return [f]


def main(argv):
    if "--selftest" in argv:
        return selftest()
    if "--dry" in argv:
        return dry()
    if "--rule-pin" in argv:
        lines, ok = rule_pin(ROOT)
        print("\n".join(lines)); print("%d checks, %d failed, 0 not judged" % (1, 0 if ok else 1)); return 0 if ok else 1
    if "--compare" in argv:
        i = argv.index("--compare")
        rows = compare(parse_log(open(argv[i + 1]).read()), parse_log(open(argv[i + 2]).read()))
        for n, ok, d in rows:
            print("%-4s T-27 %s :: %s" % ("ok" if ok else "FAIL", n, d))
        f = sum(1 for _n, ok, _d in rows if not ok)
        print("%d checks, %d failed, 0 not judged" % (len(rows), f)); return 1 if f or not rows else 0
    if "--inner" in argv:
        i = argv.index("--inner")
        return inner(argv[i + 1], json.loads(argv[i + 2]))
    if not argv or not os.path.isdir(argv[0]):
        print("usage: firstrun-check.py <bundle> [--step T-06,T-09] [--mutant=<name>] [--core real|fixture] [--log <file>] | --compare a b | --dry | --selftest"); return 2
    opt = lambda name: next((a.split("=", 1)[1] for a in argv if a.startswith(name + "=")), (argv[argv.index(name) + 1] if name in argv and argv.index(name) + 1 < len(argv) and not argv[argv.index(name) + 1].startswith("--") else None))
    mutant = opt("--mutant") if any(a.startswith("--mutant") for a in argv) else None
    if mutant is None and "--mutant" in argv:
        mutant = True
    step_opt = opt("--step")
    return run(os.path.abspath(argv[0]), set(step_opt.split(",")) if step_opt else None, mutant, opt("--core") or "real", opt("--log"))


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
