#!/usr/bin/env python3
"""sweep-settingsai: the control sweep of Settings > AI and agents and Settings > Decisions (`mac-settings-ai`).

Owner, 2026-11-11: "the email sender account selector button isn't working btw, you haven't properly tested every button/feature".
Every control of the screen, clicked ONCE through the door the app registers, its visible outcome read through the control socket.
The table is docs/COVERAGE.md "Control sweep: Settings AI"; the notes are docs/lanes/gap-sweep-a.md.

    sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/sweep-settingsai.py'
    python3 checks/parity/sweep-settingsai.py --selftest     a model of the screen: every case green, then one broken copy per case (no app, no socket)

Prints one line per case `name  pass|fail|unjudged  detail` (print_cases of journeys/lib/harness.py), then 'N checks, F failed, U not judged'.
Provider keys are typed into the fixture core, which checks only the length rule (docs/CONTROL-SOCKET.md delta 60): no provider, no network.
The decision model's download runs only against the loopback stub of checks/settingsaimac-check.py (`fixture provider-endpoint laya <127.0.0.1 url>`, the
stub serves five small files); the search model's download is clicked as it is: on a copy launched without WHOOSH_RIG_EMBED_BASE it answers
'failed' in words and asks nothing (the rig refuses a host that is not this Mac), which is still the click's visible outcome. Nothing here
asks macOS for the microphone: the dictation engine is chosen, never used.
"""
import os
import re
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable      # noqa: E402
from lib.harness import print_cases, declared_mutants   # noqa: E402

SCREEN = "sweep-settingsai"
CASES = []                                                # (name, door, words, fn)
DEPENDENT = {}                                            # case -> cases whose set-up clicks that case's control (red on its broken copy too, named here)


def case(name, door, words):
    def deco(fn):
        CASES.append((name, door, words, fn))
        return fn
    return deco


class BaseModel:
    """The self-test's stand-in for the app. A broken copy `sw-<case>` makes the control of that case do nothing: `dead(door)` is true."""
    def __init__(self, m=()):
        self.m = set(m)
        self.door_case = {}
        for name, door, _w, _f in CASES:
            self.door_case.setdefault(door, []).append(name)
        self.reset_state()

    def dead(self, door):
        return any("sw-" + n in self.m for n in self.door_case.get(door, ()))

    def reset_state(self):
        pass

    def send(self, verb, *a):
        if verb == "click":
            return self.click(a[0])
        if verb == "key":
            return self.key(a[0])
        if verb == "type":
            return self.type(a[0])
        if verb == "fixture":
            return self.fixture(*a)
        raise Unreadable("model: verb %s not served" % verb)

    def ok(self, **kw):
        return dict({"ok": True}, **kw)

    def fixture(self, *a):
        raise Unreadable("model: fixture %s not served" % (a[0],))

    def type(self, text):
        raise Unreadable("model: type not served")

    def key(self, chord):
        raise Unreadable("model: key %s: no binding" % chord)

    def uistate(self):
        raise Unreadable("model: uistate not served")

    def core(self, path):
        raise Unreadable("model: core %s not served" % path)


class Shim:
    """The Ctl calls the cases use, over a model (the real Ctl is used on the app)."""
    def __init__(self, model):
        self.m = model

    def send(self, verb, *a):
        return self.m.send(verb, *a)

    def click(self, ax):
        return self.m.click(ax)

    def key(self, chord):
        return self.m.key(chord)

    def type(self, text):
        return self.m.type(text)

    def uistate(self):
        return self.m.uistate()

    def rows(self, ax):
        return self.m.rows(ax)

    def core(self, path):
        return self.m.core(path)


D = "mac-settings-ai"
PROVIDERS = ("anthropic", "openai", "gemini", "openrouter")
KEY = "sk-sweep-0123456789"                              # 19 characters, no space: the fixture core's length rule (8 or more) accepts it
PAGES = ("which-tab", "needs-reply", "sign-in-codes", "vips", "notify-me", "labels")
PERMS = ("read", "search", "draft", "label")
LOOP = {}                                                # the loopback stub of the decision model (set by main on a real run)


def rows(c, ax):
    return c.rows(ax)


def wait_preview(c, page="which-tab", tries=300, every=0.02):
    """Waits on the served `...page.<id>.preview.state` (idle, running, done, refused), never on time alone: reads until it says done or refused, at most `tries` reads
    (a bound; the same loop as checks/decision-check.py `wait_preview`). Returns the state it ended on."""
    state = None
    for _ in range(tries):
        got = rows(c, "%s.decisions.page.%s.preview.state" % (D, page))
        state = got[0]["state"] if got else None
        if state in ("done", "refused"):
            return state
        time.sleep(every)
    return state


def one(c, ax):
    return c.rows(ax)[0]


def section(c):
    return one(c, D + ".section")["section"]


def prov(c, p):
    return one(c, "%s.provider.%s" % (D, p))["state"]


def cards(c):
    return rows(c, D + ".agents.card")


def reset(c):
    """The screen as a person finds it: the window open on the decisions page, no key, no agent made, no download."""
    c.send("fixture", "reset")
    c.click(D + ".open")
    c.click(D + ".nav.decisions")


def with_anthropic(c):
    """Anthropic connected without clicking any provider control (the fixture's own paste door), so a case on another control does not lean on Connect."""
    reset(c)
    c.send("fixture", "paste", "anthropic-key")
    c.send("clock", "advance", "1")
    c.click(D + ".nav.agents")


def typed(c, p):
    c.click("%s.provider.%s.key" % (D, p))
    c.type(KEY)


def PRE_RUN(c):
    """A real run only: the loopback stub of the decision model (five small files on 127.0.0.1) and the helpers of settingsaimac-check.py that empty the rig's models folder."""
    import importlib.util
    spec = importlib.util.spec_from_file_location("j5harness", os.path.join(ROOT, "checks", "settingsaimac-check.py"))
    H = importlib.util.module_from_spec(spec)
    sys.modules["j5harness"] = H
    spec.loader.exec_module(H)
    stub = H.Stub("laya")
    LOOP.update(stub=stub, fresh=H._laya_fresh, slow=lambda on: setattr(stub, "piece_delay", H.LAYA_PIECE_S if on else 0), done=lambda c: H._laya_wait(c, 15))
    c.send("fixture", "provider-endpoint", "laya", stub.url)           # loopback only: the rig refuses any other host


def POST_RUN():
    if LOOP.get("stub"):
        LOOP["stub"].close()


# ---- providers: the key field and Connect of each of the four rows
def key_field_case(p):
    @case("provider-%s-key-field-takes-the-typed-key" % p, "%s.provider.%s.key" % (D, p), "a click on the %s key field leaves it unfocused: the typed key goes nowhere" % p)
    def _(c):
        reset(c)
        c.click(D + ".nav.providers")
        before = one(c, "%s.provider.%s.key" % (D, p))
        c.click("%s.provider.%s.key" % (D, p))
        c.type(KEY)
        after = one(c, "%s.provider.%s.key" % (D, p))
        reset(c)
        return (before["length"], after["length"], after["focused"]), (0, len(KEY), True)


def connect_case(p):
    @case("provider-%s-connect-connects" % p, "%s.provider.%s.connect" % (D, p), "the Connect button of %s does nothing: the row stays 'not connected'" % p)
    def _(c):
        reset(c)
        c.click(D + ".nav.providers")
        before = prov(c, p)
        typed(c, p)
        c.click("%s.provider.%s.connect" % (D, p))
        c.send("clock", "advance", "1")
        after = prov(c, p)
        reset(c)
        return (before, after), ("not connected", "connected")


for _p in PROVIDERS:
    key_field_case(_p)
for _p in PROVIDERS:
    connect_case(_p)


# ---- dictation engine: this Mac, and each connected provider
@case("speech-engine-local-chooses-this-mac", D + ".speech.engine.local", "the 'on this Mac' choice of the dictation engine does nothing: the cloud engine stays chosen")
def _(c):
    with_anthropic(c)
    c.click(D + ".speech.engine.anthropic")
    before = [r["engine"] for r in rows(c, D + ".speech.engines") if r["chosen"]]
    c.click(D + ".speech.engine.local")
    after = [r["engine"] for r in rows(c, D + ".speech.engines") if r["chosen"]]
    reset(c)
    return (before, after), (["anthropic"], ["local"])


def speech_case(p):
    @case("speech-engine-%s-chooses-the-provider" % p, "%s.speech.engine.%s" % (D, p), "the %s choice of the dictation engine does nothing: 'on this Mac' stays chosen" % p)
    def _(c):
        if p == "anthropic":
            with_anthropic(c)
        else:
            reset(c)
            c.click(D + ".nav.providers")
            typed(c, p)
            c.click("%s.provider.%s.connect" % (D, p))
            c.send("clock", "advance", "1")
        before = [r["engine"] for r in rows(c, D + ".speech.engines") if r["chosen"]]
        c.click("%s.speech.engine.%s" % (D, p))
        after = [r["engine"] for r in rows(c, D + ".speech.engines") if r["chosen"]]
        reset(c)
        return (before, after), (["local"], [p])


for _p in PROVIDERS:
    speech_case(_p)


@case("speech-open-shortcuts-opens-the-shortcuts-page", D + ".speech.shortcuts", "the Open shortcuts door does nothing: the Settings window stays on its page")
def _(c):
    reset(c)
    before = one(c, D + ".speech.link")["pane"]
    c.click(D + ".speech.shortcuts")
    after = one(c, D + ".speech.link")["pane"]
    reset(c)
    return (before != "shortcuts", after), (True, "shortcuts")


# ---- the two model downloads
@case("meaning-download-leaves-not-downloaded", D + ".meaning.download", "the search model's Download button does nothing: the row stays 'notDownloaded'")
def _(c):
    reset(c)
    c.click(D + ".nav.providers")
    before = one(c, D + ".meaning")["state"]
    c.click(D + ".meaning.download")
    after = one(c, D + ".meaning")["state"]
    return (before, after != "notDownloaded"), ("notDownloaded", True), (after, one(c, D + ".meaning").get("words"))


def laya_fresh(c):
    reset(c)
    if LOOP.get("fresh"):
        try:
            LOOP["fresh"](c)
        except Unreadable as e:
            # round 12 (main103: the row read 'ready' after its folder was emptied): the line reads the disk when the page opens or on a reset
            # (AISettingsStore.resetFromFixture -> layaStateChanged), and here the page was already open when the previous case's download ended.
            # A reset after the folder is emptied makes the line read the disk again; only a line still not absent then stops the case.
            reset(c)
            if one(c, D + ".decisions.local")["state"] != "absent":
                raise e
    elif hasattr(c, "m"):
        c.m.laya = "absent"


@case("laya-download-starts-the-download", D + ".decisions.local.download", "the decision model's Download button does nothing: the row stays 'absent'")
def _(c):
    laya_fresh(c)
    before = one(c, D + ".decisions.local")["state"]
    c.click(D + ".decisions.local.download")
    after = one(c, D + ".decisions.local")["state"]
    if LOOP.get("done"):
        LOOP["done"](c)
    return (before, after in ("downloading", "ready")), ("absent", True), (before, after)


@case("laya-cancel-stops-the-download", D + ".decisions.local.cancel", "the Cancel button does nothing: the download keeps running")
def _(c):
    laya_fresh(c)
    if LOOP.get("slow"):
        LOOP["slow"](True)
    c.click(D + ".decisions.local.download")
    mid = one(c, D + ".decisions.local")["state"]
    c.click(D + ".decisions.local.cancel")
    row = one(c, D + ".decisions.local")
    after = row["state"]
    if LOOP.get("slow"):
        LOOP["slow"](False)
    # round 7: the row's own words say why when the download ended on its own (main93 read 'failed' right after Cancel)
    return (mid, after), ("downloading", "absent"), {"words_after": row.get("words"), "stub_gets": LOOP["stub"].gets[-6:] if LOOP.get("stub") else None}


# ---- Jev: key field, Connect, Allow, Withdraw
@case("jev-key-field-takes-the-typed-key", D + ".decisions.jev.key.field", "a click on the Jev key field leaves it unfocused: the typed key goes nowhere")
def _(c):
    reset(c)
    c.click(D + ".decisions.engine.jev")
    before = one(c, D + ".provider.jev.key") if False else None
    c.click(D + ".decisions.jev.key.field")
    c.type(KEY)
    c.click(D + ".decisions.jev.connect")
    got = one(c, D + ".decisions.jev.key")["connected"]
    reset(c)
    return (before, got), (None, True)


@case("jev-connect-saves-the-key", D + ".decisions.jev.connect", "the Jev Connect button does nothing: the key is never stored")
def _(c):
    reset(c)
    c.click(D + ".decisions.engine.jev")
    before = one(c, D + ".decisions.jev.key")["connected"]
    c.send("fixture", "paste", KEY) if False else None
    c.click(D + ".decisions.jev.key.field")
    c.type(KEY)
    c.click(D + ".decisions.jev.connect")
    after = one(c, D + ".decisions.jev.key")["connected"]
    reset(c)
    return (before, after), (False, True)


@case("jev-allow-gives-the-consent", D + ".decisions.jev.consent.allow", "the Allow button does nothing: the consent stays 'asked'")
def _(c):
    reset(c)
    before = one(c, D + ".decisions.jev.consent")["given"]
    c.click(D + ".decisions.jev.consent.allow")
    after = one(c, D + ".decisions.jev.consent")["given"]
    reset(c)
    return (before, after), (False, True)


@case("jev-withdraw-takes-the-consent-back", D + ".decisions.jev.consent.withdraw", "the Withdraw button does nothing: the consent stays given")
def _(c):
    reset(c)
    c.click(D + ".decisions.jev.consent.allow")
    before = one(c, D + ".decisions.jev.consent")["given"]
    c.click(D + ".decisions.jev.consent.withdraw")
    after = one(c, D + ".decisions.jev.consent")["given"]
    reset(c)
    return (before, after), (True, False)


# ---- the engine segmented control
def chosen_engine(c):
    return [r["id"] for r in rows(c, D + ".decisions.engine") if r["chosen"]]


@case("engine-off-chooses-no-model", D + ".decisions.engine.off", "the Off segment does nothing: Laya stays chosen")
def _(c):
    reset(c)
    before = chosen_engine(c)
    c.click(D + ".decisions.engine.off")
    after = chosen_engine(c)
    reset(c)
    return (before, after), (["local"], ["off"])


@case("engine-local-chooses-laya", D + ".decisions.engine.local", "the Laya segment does nothing: Off stays chosen")
def _(c):
    reset(c)
    c.click(D + ".decisions.engine.off")
    before = chosen_engine(c)
    c.click(D + ".decisions.engine.local")
    after = chosen_engine(c)
    reset(c)
    return (before, after), (["off"], ["local"])


@case("engine-jev-asks-for-its-key", D + ".decisions.engine.jev", "the Jev segment does nothing: Laya stays chosen and no word says what Jev needs")
def _(c):
    reset(c)
    before = chosen_engine(c)
    c.click(D + ".decisions.engine.jev")
    after = chosen_engine(c)
    refusal = [r["refusal"] for r in rows(c, D + ".decisions.engine") if r["id"] == "jev"][0]
    reset(c)
    return (before, after, bool(refusal)), (["local"], ["jev"], True)


# ---- the list of pages and one page
def page_open_case(pid):
    @case("page-%s-row-opens-the-page" % pid, "%s.decisions.open.%s" % (D, pid), "the %s row does nothing: the list stays on the decisions page" % pid)
    def _(c):
        reset(c)
        before = section(c)
        c.click("%s.decisions.open.%s" % (D, pid))
        after = section(c)
        reset(c)
        return (before, after), ("decisions", "page." + pid)


for _pg in PAGES:
    page_open_case(_pg)


@case("page-back-returns-to-the-list", D + ".decisions.back", "the back button does nothing: the page stays open")
def _(c):
    reset(c)
    c.click(D + ".decisions.open.vips")
    before = section(c)
    c.click(D + ".decisions.back")
    after = section(c)
    reset(c)
    return (before, after), ("page.vips", "decisions")


def entries(c, pid):
    return rows(c, "%s.decisions.page.%s.entries" % (D, pid))


@case("page-add-rule-inserts-a-rule-first", D + ".decisions.page.which-tab.add-rule", "Add a rule does nothing: the list keeps its four entries")
def _(c):
    reset(c)
    before = len(entries(c, "which-tab"))
    c.click(D + ".decisions.page.which-tab.add-rule")
    after = entries(c, "which-tab")
    reset(c)
    return (before, len(after), after[0]["kind"]), (4, 5, "rule")


@case("page-add-model-inserts-the-model-entry", D + ".decisions.page.vips.add-model", "Add the model does nothing: the VIPs page keeps no model entry")
def _(c):
    reset(c)
    before = [e["kind"] for e in entries(c, "vips")]
    c.click(D + ".decisions.page.vips.add-model")
    after = [e["kind"] for e in entries(c, "vips")]
    reset(c)
    return (before, after), (["rule", "default"], ["rule", "model", "default"])


@case("page-move-up-reorders-the-entries", D + ".decisions.page.which-tab.entry.2.move-up", "the move door does nothing: entry 2 stays second")
def _(c):
    reset(c)
    before = [e["text"] for e in entries(c, "which-tab")][:2]
    c.click(D + ".decisions.page.which-tab.entry.2.move-up")
    after = [e["text"] for e in entries(c, "which-tab")][:2]
    reset(c)
    return (after, before[::-1]), (before[::-1], after[::-1][::-1])


@case("page-preview-lists-the-last-messages", D + ".decisions.page.which-tab.preview", "Preview does nothing: no message is listed")
def _(c):
    reset(c)
    before = len(rows(c, D + ".decisions.page.which-tab.preview"))
    c.click(D + ".decisions.page.which-tab.preview")
    wait_preview(c)
    after = len(rows(c, D + ".decisions.page.which-tab.preview"))
    reset(c)
    return (before, after > 0), (0, True)


@case("page-apply-writes-the-preview", D + ".decisions.page.which-tab.apply", "Apply does nothing: the core never records the applied preview")
def _(c):
    reset(c)
    c.click(D + ".decisions.page.which-tab.preview")
    wait_preview(c)
    before = c.core("decision_preview/applied")
    c.click(D + ".decisions.page.which-tab.apply")
    after = c.core("decision_preview/applied")
    reset(c)
    return (before, after), (False, True)


@case("page-proposal-accept-adds-the-rule", D + ".decisions.page.labels.proposal.accept", "Accept does nothing: the proposed label stays a proposal")
def _(c):
    reset(c)
    c.click(D + ".decisions.open.labels")
    before = (len(rows(c, D + ".decisions.page.labels.proposal")), len(entries(c, "labels")))
    c.click(D + ".decisions.page.labels.proposal.accept")
    after = (len(rows(c, D + ".decisions.page.labels.proposal")), len(entries(c, "labels")))
    reset(c)
    # the Labels list ships with its default alone (docs/DECISION-MODEL.md v2 §1, the lead's ruling 2026-11-12 on hid-ai Q1): 1 entry, 2 once
    # the proposal is a rule. Before: ((1, 2), (0, 3)), counted on the sample rule the app shipped then.
    return (before, after), ((1, 1), (0, 2))


# ---- one entry of a page: the switch, the question field, the trust menu, the label menu, and the proposal's Dismiss (doors added by gap-decisions: docs/lanes/gap-decisions.md "Settings AI sweep findings", F4 to F8)
@case("page-entry-switch-turns-the-entry-off", D + ".decisions.page.which-tab.entry.1.enabled", "the entry switch does nothing: entry 1 stays on")
def _(c):
    reset(c)
    before = entries(c, "which-tab")[0]["enabled"]
    c.click(D + ".decisions.page.which-tab.entry.1.enabled")
    after = entries(c, "which-tab")[0]["enabled"]
    reset(c)
    return (before, after), (True, False)


@case("page-question-field-takes-the-typed-words", D + ".decisions.page.which-tab.entry.3.question", "a click on the model entry's question does not give it the typed words")
def _(c):
    reset(c)
    before = entries(c, "which-tab")[2]["text"]
    c.click(D + ".decisions.page.which-tab.entry.3.question")
    c.type(" today")
    after = entries(c, "which-tab")[2]["text"]
    reset(c)
    return after, before + " today"


@case("page-trust-strict-sets-the-level", D + ".decisions.page.which-tab.entry.3.trust.1", "the Strict item of the trust menu does nothing: the trust stays at the shipped value")
def _(c):
    reset(c)
    before = entries(c, "which-tab")[2]["trust"]
    c.click(D + ".decisions.page.which-tab.entry.3.trust.1")
    after = entries(c, "which-tab")[2]["trust"]
    reset(c)
    return (before, after), (80, 90)


@case("page-trust-eager-sets-the-level", D + ".decisions.page.which-tab.entry.3.trust.3", "the Eager item of the trust menu does nothing: the trust stays at the shipped value")
def _(c):
    reset(c)
    before = entries(c, "which-tab")[2]["trust"]
    c.click(D + ".decisions.page.which-tab.entry.3.trust.3")
    after = entries(c, "which-tab")[2]["trust"]
    reset(c)
    return (before, after), (80, 70)


@case("page-trust-balanced-sets-the-level", D + ".decisions.page.which-tab.entry.3.trust.2", "the Balanced item of the trust menu does nothing: the trust stays where the Eager item put it")
def _(c):
    reset(c)
    c.click(D + ".decisions.page.which-tab.entry.3.trust.3")
    before = entries(c, "which-tab")[2]["trust"]
    c.click(D + ".decisions.page.which-tab.entry.3.trust.2")
    after = entries(c, "which-tab")[2]["trust"]
    reset(c)
    return (before, after), (70, 80)


@case("page-label-menu-item-files-the-rule-under-the-label", D + ".decisions.page.labels.entry.1.node.2", "an item of the label menu does nothing: the rule keeps its label")
def _(c):
    reset(c)
    c.click(D + ".decisions.open.labels")
    c.click(D + ".decisions.page.labels.add-rule")      # the list ships with its default alone (DECISION-MODEL.md v2 §1): the rule to file is the owner's own
    menu = [r["path"] for r in rows(c, D + ".label-menu")]
    want = menu[1] if len(menu) > 1 else None
    before = entries(c, "labels")[0]["outcome"]
    c.click(D + ".decisions.page.labels.entry.1.node.2")
    after = entries(c, "labels")[0]["outcome"]
    reset(c)
    return (len(menu) > 1, before != want, after), (True, True, want)


@case("page-proposal-dismiss-clears-the-proposal", D + ".decisions.page.labels.proposal.dismiss", "Dismiss does nothing: the proposed label stays a proposal")
def _(c):
    reset(c)
    c.click(D + ".decisions.open.labels")
    before = (len(rows(c, D + ".decisions.page.labels.proposal")), len(entries(c, "labels")))
    c.click(D + ".decisions.page.labels.proposal.dismiss")
    after = (len(rows(c, D + ".decisions.page.labels.proposal")), len(entries(c, "labels")))
    reset(c)
    # the Labels list ships with its default alone (DECISION-MODEL.md v2 §1; hid-ai Q1). Before: ((1, 2), (0, 2)).
    return (before, after), ((1, 1), (0, 1))


# ---- agents
@case("agents-new-opens-a-draft-card", D + ".agents.new", "New agent does nothing: no draft card opens")
def _(c):
    with_anthropic(c)
    before = len(cards(c))
    c.click(D + ".agents.new")
    after = cards(c)
    got = (before, len(after), after[0]["editing"])
    reset(c)
    return got, (before, before + 1, True)


@case("agents-form-link-goes-to-the-providers", D + ".agents.form.link", "the 'Go to providers' button does nothing: the page stays on agents")
def _(c):
    reset(c)
    c.click(D + ".agents.new")                         # no provider connected: the form line says which is missing
    before = (len(rows(c, D + ".agents.form")), section(c))
    c.click(D + ".agents.form.link")
    after = section(c)
    reset(c)
    return (before[0], after), (1, "providers")


def draft(c):
    with_anthropic(c)
    c.click(D + ".agents.new")


@case("agents-name-field-takes-the-typed-name", D + ".agents.name", "a click on the name field does not give it the typed text")
def _(c):
    draft(c)
    c.click(D + ".agents.name")
    c.type("Scout")
    got = cards(c)[0]["name"]
    reset(c)
    return got, "Scout"


@case("agents-save-keeps-the-agent", D + ".agents.save", "Save does nothing: the draft stays open")
def _(c):
    draft(c)
    c.click(D + ".agents.name")
    c.type("Scout")
    before = [(x["name"], x["editing"]) for x in cards(c)][:1]
    c.click(D + ".agents.save")
    after = [(x["name"], x["editing"]) for x in cards(c) if x["name"] == "Scout"]
    reset(c)
    return (before, after), ([("Scout", True)], [("Scout", False)])


@case("agents-duplicate-copies-the-last-agent", D + ".agents.duplicate", "Duplicate does nothing: no copy appears")
def _(c):
    draft(c)
    c.click(D + ".agents.name")
    c.type("Scout")
    c.click(D + ".agents.save")
    before = len(cards(c))
    c.click(D + ".agents.duplicate")
    after = cards(c)
    reset(c)
    return (before + 1, len(after), any("Scout" in x["name"] and x["name"] != "Scout" for x in after)), (before + 1, before + 1, True)


def allow_case(w):
    @case("agents-allow-%s-toggles-the-permission" % w, "%s.agents.allow.%s" % (D, w), "the %s chip does nothing: the agent's permissions stay as they were" % w)
    def _(c):
        draft(c)
        c.click(D + ".agents.name")
        c.type("Scout")
        before = list(cards(c)[0]["allowed"])
        c.click("%s.agents.allow.%s" % (D, w))
        after = list(cards(c)[0]["allowed"])
        reset(c)
        return (len(before), len(after)), (0, 1)


for _w in PERMS:
    allow_case(_w)


@case("agents-model-choice-sets-the-model", D + ".agents.model.1.choose.2", "a choice in the model menu does nothing: the card keeps its first model")
def _(c):
    draft(c)
    first = [r for r in rows(c, D + ".agents.model.1") if r["enabled"]]
    c.click(D + ".agents.model.1.choose.2")
    after = [r for r in rows(c, D + ".agents.model.1") if r["current"]]
    reset(c)
    return (len(first) >= 2, [r["model"] for r in after]), (True, [first[1]["model"]] if len(first) >= 2 else None)


# ---- the palette lines that open the pages
@case("palette-ai-settings-line-opens-the-window", "mac-palette.result.ai-settings", "the palette line 'AI settings' opens nothing")
def _(c):
    reset(c)
    c.click("mac-settings-window.close")
    before = c.uistate().get("surface")
    c.click("mac-palette.result.ai-settings")
    after = c.uistate().get("surface")
    reset(c)
    return (before, after), ("mac-main", D)


@case("palette-decisions-line-opens-the-decisions-page", "mac-palette.result.decisions", "the palette line 'Decisions' opens nothing")
def _(c):
    reset(c)
    c.click(D + ".nav.agents")
    c.click("mac-palette.result.decisions")
    after = section(c)
    reset(c)
    return after, "decisions"


DEPENDENT.update({
    "provider-%s-connect-connects" % p: ["speech-engine-%s-chooses-the-provider" % p] for p in PROVIDERS if p != "anthropic"})
DEPENDENT["speech-engine-anthropic-chooses-the-provider"] = ["speech-engine-local-chooses-this-mac"]
DEPENDENT["agents-name-field-takes-the-typed-name"] = ["agents-save-keeps-the-agent", "agents-duplicate-copies-the-last-agent"] + ["agents-allow-%s-toggles-the-permission" % w for w in PERMS]
DEPENDENT["agents-save-keeps-the-agent"] = ["agents-duplicate-copies-the-last-agent"]
DEPENDENT["agents-new-opens-a-draft-card"] = ["agents-name-field-takes-the-typed-name", "agents-save-keeps-the-agent", "agents-duplicate-copies-the-last-agent", "agents-model-choice-sets-the-model"] + ["agents-allow-%s-toggles-the-permission" % w for w in PERMS]
DEPENDENT["jev-key-field-takes-the-typed-key"] = ["jev-connect-saves-the-key"]
DEPENDENT["jev-connect-saves-the-key"] = ["jev-key-field-takes-the-typed-key"]
DEPENDENT["engine-off-chooses-no-model"] = ["engine-local-chooses-laya"]
DEPENDENT["page-vips-row-opens-the-page"] = ["page-back-returns-to-the-list"]
DEPENDENT["jev-allow-gives-the-consent"] = ["jev-withdraw-takes-the-consent-back"]
DEPENDENT["page-preview-lists-the-last-messages"] = ["page-apply-writes-the-preview"]
for _p in PROVIDERS:        # Connect and the speech choice type the key through the key field's door first
    DEPENDENT["provider-%s-key-field-takes-the-typed-key" % _p] = ["provider-%s-connect-connects" % _p] + (["speech-engine-%s-chooses-the-provider" % _p] if _p != "anthropic" else [])
DEPENDENT["laya-download-starts-the-download"] = ["laya-cancel-stops-the-download"]
DEPENDENT["page-trust-eager-sets-the-level"] = ["page-trust-balanced-sets-the-level"]      # Balanced starts from the level Eager sets

# Controls with no door, no drawn control or no readable outcome: findings (title, file, line, a snippet that must be on that line, why).
FINDINGS = [
    ("F1 the dictation menu pill", "Sources/Features/SettingsAIMac/SpeechGroup.swift", 28, "Menu {",
     "the pill is a SwiftUI Menu with an id (`mac-settings-ai.speech.engine`) but no click door opens it; each ITEM has a door (`.speech.engine.local|<provider>`, covered), the menu opening is not driven. gap-decisions: a SwiftUI Menu pill cannot be opened from a door; its items have doors and the cases click those"),
    ("F2 Open shortcuts is never drawn", "Sources/Features/SettingsAIMac/SpeechGroup.swift", 64, "SettingsButton(title: AIWords.openShortcuts",
     "the button sits in the `else` branch of `if let key = store.dictationKey`, and `dictationKey` is never nil (AISettingsStore.swift:272, `?? Self.holdToDictateKey`), so the control is dead code on screen; the door `mac-settings-ai.speech.shortcuts` is clicked, the drawn button is not reachable. gap-decisions: showing it is a look change, so it needs the lead's or the owner's word"),
    ("F3 the agent's model pill", "Sources/Features/SettingsAIMac/AgentsView.swift", 94, "Menu {",
     "the pill itself has an id (`.agents.model.<n>`) and no door to open it; its entries have doors `.agents.model.<n>.choose.<k>` (one is clicked, k = 2); the greyed 'add a key above' entry has no action. gap-decisions: a SwiftUI Menu pill cannot be opened from a door, the case keeps clicking the items"),
    ("F9 drag to reorder an entry", "Sources/Features/SettingsAIMac/DecisionPageView.swift", 28, ".onDrag { dragging = e.id; return NSItemProvider(object: e.id.uuidString as NSString) }",
     "the real gesture is a drag-and-drop (the `drag` verb does not deliver events to the window, docs/CONTROL-SOCKET.md delta 35); the door `.entry.<n>.move-up` runs the same `move`, and is what the case clicks"),
    ("F10 the search model's Cancel", "Sources/Features/SettingsAIMac/MeaningSetupRow.swift", 49, "download: { line.download() }, cancel: nil, chip: AIWords.speechStateReady)",
     "`cancel: nil`: the search model row draws no Cancel button, so its id `mac-settings-ai.meaning.cancel` names nothing on screen. gap-decisions: adding a Cancel button is a look change, so it needs the lead's or the owner's word"),
    ("F11 the ChatGPT row", "Sources/Features/SettingsAIMac/ProvidersView.swift", 44, "SettingsRow(ax:",
     "a state chip only (`ChatGPTState.words`, the sign-in lives elsewhere): no control to click; its state row `mac-settings-ai.provider.chatgpt` is read by settingsaimac-check.py. gap-decisions: a state chip, no control"),
]


class Model(BaseModel):
    def reset_state(self):
        self.prov = {p: "not connected" for p in PROVIDERS}
        self.drafts, self.focus, self.section = {}, None, "decisions"
        self.speech = "local"
        self.laya = getattr(self, "laya", "absent") if False else "absent"
        self.meaning = "notDownloaded"
        self.engine, self.jev_key, self.consent, self.jev_wanted = "local", False, False, False
        self.pages = {"which-tab": ["from a VIP", "has a list header", "Which tab?", "Primary"], "needs-reply": ["a", "b", "c", "d"],
                      "sign-in-codes": ["a", "b", "c"], "vips": ["on your VIP list", "no"], "notify-me": ["a", "b", "c", "d"], "labels": ["no label"]}
        self.kinds = {"which-tab": ["rule", "rule", "model", "default"], "vips": ["rule", "default"], "labels": ["default"]}
        self.preview, self.applied, self.proposal = [], False, True
        self.outs = {"which-tab": ["Primary", "Updates", "the tab it picks", "Primary"], "labels": ["none"]}
        self.meta = {pid: [{"enabled": True, "trust": None, "outcome": self.outs.get(pid, ["x"] * 9)[i]} for i in range(len(texts))] for pid, texts in self.pages.items()}
        self.meta["which-tab"][2]["trust"] = 80
        self.tree = ["Work", "Work \u203a Clients", "Work \u203a Clients \u203a Provanta", "Work \u203a Clients \u203a New"]
        self.agents, self.draft_agent, self.form = [], None, False
        self.surface = "mac-main"

    def fixture(self, *a):
        if a[0] == "reset":
            self.reset_state()
            return self.ok()
        if a[0] == "paste" and a[1] == "anthropic-key":
            self.prov["anthropic"] = "connected"
            return self.ok()
        return super().fixture(*a)

    def send(self, verb, *a):
        if verb == "clock":
            return self.ok()
        return super().send(verb, *a)

    def type(self, text):
        if self.focus == "jev":
            self.drafts["jev"] = self.drafts.get("jev", "") + text
        elif self.focus in PROVIDERS:
            self.drafts[self.focus] = self.drafts.get(self.focus, "") + text
        elif isinstance(self.focus, tuple):
            pid, i = self.focus
            self.pages[pid][i] += text
        elif self.focus == "name" and self.draft_agent is not None:
            self.draft_agent["name"] += text
        return self.ok()            # no field holds the keys: the text goes to the shared buffer, nothing on this screen changes

    def click(self, ax):
        d = self.dead(ax)
        s = ax[len(D) + 1:] if ax.startswith(D + ".") else ax
        if s in ("open", "nav.decisions", "nav.agents", "nav.providers"):
            self.surface = D
            if s != "open":
                self.section = s.split(".")[1]
        elif ax == "mac-settings-window.close":
            self.surface = "mac-main"
        elif ax == "mac-palette.result.ai-settings":
            if not d:
                self.surface = D
        elif ax == "mac-palette.result.decisions":
            if not d:
                self.section, self.surface = "decisions", D
        elif s.startswith("provider.") and s.endswith(".key"):
            if not d:
                self.focus = s.split(".")[1]
        elif s.startswith("provider.") and s.endswith(".connect"):
            p = s.split(".")[1]
            if not d and len(self.drafts.get(p, "")) >= 8:
                self.prov[p] = "connected"
            self.drafts[p] = ""
            self.focus = None
        elif s.startswith("speech.engine."):
            e = s.rsplit(".", 1)[1]
            if e != "local" and self.prov.get(e) != "connected":
                return {"ok": False, "error": "did nothing"}
            if not d:
                self.speech = e
        elif s == "speech.shortcuts":
            if not d:
                self.sw = "shortcuts"
        elif s == "meaning.download":
            if not d:
                self.meaning = "failed"
        elif s == "decisions.local.download":
            if self.laya == "ready":
                return {"ok": False, "error": "did nothing"}
            if not d:
                self.laya = "downloading"
        elif s == "decisions.local.cancel":
            if self.laya != "downloading":
                return {"ok": False, "error": "did nothing"}
            if not d:
                self.laya = "absent"
        elif s == "decisions.jev.key.field":
            if not d:
                self.focus = "jev"
        elif s == "decisions.jev.connect":
            k = self.drafts.pop("jev", "")
            self.focus = None
            if not d and len(k) >= 8:
                self.jev_key = True
        elif s == "decisions.jev.consent.allow":
            if not d:
                self.consent = True
        elif s == "decisions.jev.consent.withdraw":
            if not d:
                self.consent = False
        elif s.startswith("decisions.engine."):
            e = s.rsplit(".", 1)[1]
            if not d:
                if e == "jev" and not (self.jev_key and self.consent):
                    self.jev_wanted = True
                else:
                    self.engine, self.jev_wanted = e, False
        elif s.startswith("decisions.open."):
            if not d:
                self.section = "page." + s.rsplit(".", 1)[1]
        elif s == "decisions.back":
            if not d:
                self.section = "decisions"
        elif s == "decisions.page.which-tab.add-rule":
            if not d:
                self.pages["which-tab"].insert(0, "subject contains …")
                self.kinds["which-tab"].insert(0, "rule")
                self.meta["which-tab"].insert(0, {"enabled": True, "trust": None, "outcome": "x"})
        elif s == "decisions.page.labels.add-rule":      # the Labels list ships with its default alone: its first rule is added (filed under the tree's first node)
            if not d:
                self.pages["labels"].insert(0, "subject contains …")
                self.kinds["labels"].insert(0, "rule")
                self.meta["labels"].insert(0, {"enabled": True, "trust": None, "outcome": self.tree[0]})
        elif s == "decisions.page.vips.add-model":
            if not d:
                self.pages["vips"].insert(1, "Ask the model")
                self.kinds["vips"].insert(1, "model")
                self.meta["vips"].insert(1, {"enabled": True, "trust": 80, "outcome": "x"})
        elif s == "decisions.page.which-tab.entry.2.move-up":
            if not d:
                p = self.pages["which-tab"]
                p[0], p[1] = p[1], p[0]
                m = self.meta["which-tab"]
                m[0], m[1] = m[1], m[0]
        elif s == "decisions.page.which-tab.preview":
            if not d:
                self.preview = [{"message": "msg-%02d" % i} for i in range(20)]
        elif s == "decisions.page.which-tab.apply":
            if not d:
                self.applied = True
        elif s == "decisions.page.labels.proposal.accept":
            if not d and self.proposal:
                self.proposal = False
                self.pages["labels"].insert(0, "the model proposed it")
                self.kinds["labels"].insert(0, "rule")
                self.meta["labels"].insert(0, {"enabled": True, "trust": None, "outcome": "x"})
        elif s.startswith("decisions.page.") and ".entry." in s and s.rsplit(".", 1)[1] == "enabled":
            pid, n = s.split(".")[2], int(s.split(".")[4])
            if n >= len(self.pages[pid]):
                return {"ok": False, "error": "did nothing"}
            if not d:
                self.meta[pid][n - 1]["enabled"] = not self.meta[pid][n - 1]["enabled"]
        elif s.startswith("decisions.page.") and ".entry." in s and s.endswith(".question"):
            pid, n = s.split(".")[2], int(s.split(".")[4])
            if n > len(self.pages[pid]) or self.kind_of(pid, n - 1) != "model":
                return {"ok": False, "error": "did nothing"}
            if not d:
                self.focus = (pid, n - 1)
        elif s.startswith("decisions.page.") and ".entry." in s and ".trust." in s:
            pid, n, k = s.split(".")[2], int(s.split(".")[4]), int(s.split(".")[6])
            if n > len(self.pages[pid]) or self.meta[pid][n - 1]["trust"] is None or k > 3:
                return {"ok": False, "error": "did nothing"}
            if not d:
                self.meta[pid][n - 1]["trust"] = (90, 80, 70)[k - 1]
        elif s.startswith("decisions.page.labels.entry.") and ".node." in s:
            n, k = int(s.split(".")[4]), int(s.split(".")[6])
            if n > len(self.pages["labels"]) or self.kind_of("labels", n - 1) != "rule" or k > len(self.tree):
                return {"ok": False, "error": "did nothing"}
            if not d:
                self.meta["labels"][n - 1]["outcome"] = self.tree[k - 1]
        elif s == "decisions.page.labels.proposal.dismiss":
            if not self.proposal:
                return {"ok": False, "error": "did nothing"}
            if not d:
                self.proposal = False
        elif s == "agents.new":
            if self.prov["anthropic"] == "connected":
                if not d:
                    self.draft_agent = {"name": "", "allowed": [], "editing": True, "model": "claude-sonnet-5-5"}
                self.form = False
            else:
                self.form = True
        elif s == "agents.form.link":
            if not self.form:
                return {"ok": False, "error": "did nothing"}
            if not d:
                self.section = "providers"
        elif s == "agents.name":
            if self.draft_agent is None:
                return {"ok": False, "error": "did nothing"}
            if not d:
                self.focus = "name"
        elif s == "agents.save":
            if self.draft_agent is not None and not d:
                a = dict(self.draft_agent, editing=False)
                a["name"] = a["name"] or "Untitled agent"
                self.agents.append(a)
                self.draft_agent = None
        elif s == "agents.duplicate":
            if self.agents and not d:
                self.agents.append(dict(self.agents[-1], name=self.agents[-1]["name"] + " copy"))
        elif s.startswith("agents.allow."):
            w = s.rsplit(".", 1)[1]
            tgt = self.draft_agent or (self.agents[-1] if self.agents else None)
            if tgt is None:
                return {"ok": False, "error": "did nothing"}
            if not d:
                (tgt["allowed"].remove(w) if w in tgt["allowed"] else tgt["allowed"].append(w))
        elif s.startswith("agents.model.1.choose."):
            k = int(s.rsplit(".", 1)[1])
            if self.draft_agent is None or k > 6:
                return {"ok": False, "error": "did nothing"}
            if not d:
                self.draft_agent["model"] = ["claude-sonnet-5-5", "claude-haiku-5-5", "claude-opus-5-5"][k - 1] if k <= 3 else self.draft_agent["model"]
        else:
            raise Unreadable("click %s: nothing registered" % ax)
        return {"ok": True}

    sw = "general"

    def kind_of(self, pid, i):
        ks = self.kinds.get(pid, ["rule"] * 9)
        return ks[i] if i < len(ks) else "rule"

    def uistate(self):
        return {"surface": self.surface}

    def core(self, path):
        if path == "decision_preview/applied":
            return self.applied
        raise Unreadable("core %s not served" % path)

    def rows(self, ax):
        if ax == D + ".section":
            return [{"section": self.section if self.section in ("providers", "agents", "decisions") or self.section.startswith("page.") else "decisions"}]
        if ax.startswith(D + ".provider."):
            rest = ax[len(D) + 10:]
            if rest.endswith(".key"):
                p = rest[:-4]
                return [{"length": len(self.drafts.get(p, "")), "focused": self.focus == p}]
            return [{"state": self.prov[rest]}]
        if ax == D + ".speech.engines":
            return [{"engine": "local", "chosen": self.speech == "local"}] + [{"engine": p, "chosen": self.speech == p} for p in PROVIDERS if self.prov[p] == "connected"]
        if ax == D + ".speech.link":
            return [{"pane": self.sw}]
        if ax == D + ".meaning":
            return [{"state": self.meaning, "words": "failed in words" if self.meaning == "failed" else ""}]
        if ax == D + ".decisions.local":
            return [{"state": self.laya}]
        if ax == D + ".decisions.jev.key":
            return [{"connected": self.jev_key}]
        if ax == D + ".decisions.jev.consent":
            return [{"given": self.consent}]
        if ax == D + ".decisions.engine":
            return [{"id": e, "chosen": (e == "jev" and self.jev_wanted) or (e == self.engine and not self.jev_wanted),
                     "refusal": "Add your Jev key first." if e == "jev" and self.jev_wanted else ""} for e in ("local", "jev", "off")]
        if ax.startswith(D + ".decisions.page.") and ax.endswith(".entries"):
            pid = ax[len(D) + 16:-8]
            return [{"n": i + 1, "kind": self.kind_of(pid, i), "text": t, "enabled": self.meta[pid][i]["enabled"], "trust": self.meta[pid][i]["trust"], "outcome": self.meta[pid][i]["outcome"]} for i, t in enumerate(self.pages[pid])]
        if ax == D + ".label-menu":
            return [{"path": p} for p in self.tree]
        if ax == D + ".decisions.page.which-tab.preview":
            return self.preview
        if ax == D + ".decisions.page.which-tab.preview.state":
            return [{"state": "done" if self.preview else "idle"}]      # the served state of the page's preview (SettingsAIMacFeature.swift)
        if ax == D + ".decisions.page.labels.proposal":
            return [{"path": "Work › Clients › New"}] if self.proposal else []
        if ax == D + ".agents.card":
            return ([dict(self.draft_agent)] if self.draft_agent else []) + [dict(a) for a in reversed(self.agents)] if False else ([dict(self.draft_agent)] if self.draft_agent else []) + [dict(a) for a in self.agents]
        if ax == D + ".agents.form":
            return [{"missing": "Connect a provider first."}] if self.form else []
        if ax == D + ".agents.model.1":
            if self.draft_agent is None:
                return []
            ms = ["claude-sonnet-5-5", "claude-haiku-5-5", "claude-opus-5-5"]
            return [{"model": m, "enabled": True, "current": m == self.draft_agent["model"]} for m in ms]
        raise Unreadable("no rows at %s" % ax)


# MUTANTS: per case, the one-line change in the app that turns it red: `label | file | find (occurs once in the file) | replace`.
# sweep-settingsai/provider-anthropic-key-field-takes-the-typed-key | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.provider.\(p).key"] = { _ in s.section = .providers; s.keyFocus = p; return true } | out["mac-settings-ai.provider.\(p).key"] = { _ in s.section = .providers; s.keyFocus = p == "anthropic" ? nil : p; return true }
# sweep-settingsai/provider-openai-key-field-takes-the-typed-key | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.provider.\(p).key"] = { _ in s.section = .providers; s.keyFocus = p; return true } | out["mac-settings-ai.provider.\(p).key"] = { _ in s.section = .providers; s.keyFocus = p == "openai" ? nil : p; return true }
# sweep-settingsai/provider-gemini-key-field-takes-the-typed-key | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.provider.\(p).key"] = { _ in s.section = .providers; s.keyFocus = p; return true } | out["mac-settings-ai.provider.\(p).key"] = { _ in s.section = .providers; s.keyFocus = p == "gemini" ? nil : p; return true }
# sweep-settingsai/provider-openrouter-key-field-takes-the-typed-key | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.provider.\(p).key"] = { _ in s.section = .providers; s.keyFocus = p; return true } | out["mac-settings-ai.provider.\(p).key"] = { _ in s.section = .providers; s.keyFocus = p == "openrouter" ? nil : p; return true }
# sweep-settingsai/provider-anthropic-connect-connects | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.provider.\(p).connect"] = { _ in s.connect(p); return true } | out["mac-settings-ai.provider.\(p).connect"] = { _ in p == "anthropic" ? true : { s.connect(p); return true }() }
# sweep-settingsai/provider-openai-connect-connects | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.provider.\(p).connect"] = { _ in s.connect(p); return true } | out["mac-settings-ai.provider.\(p).connect"] = { _ in p == "openai" ? true : { s.connect(p); return true }() }
# sweep-settingsai/provider-gemini-connect-connects | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.provider.\(p).connect"] = { _ in s.connect(p); return true } | out["mac-settings-ai.provider.\(p).connect"] = { _ in p == "gemini" ? true : { s.connect(p); return true }() }
# sweep-settingsai/provider-openrouter-connect-connects | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.provider.\(p).connect"] = { _ in s.connect(p); return true } | out["mac-settings-ai.provider.\(p).connect"] = { _ in p == "openrouter" ? true : { s.connect(p); return true }() }
# sweep-settingsai/speech-engine-local-chooses-this-mac | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | "mac-settings-ai.speech.engine.local": { _ in s.chooseSpeech(provider: nil); return true }, | "mac-settings-ai.speech.engine.local": { _ in true },
# sweep-settingsai/speech-engine-anthropic-chooses-the-provider | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | s.chooseSpeech(provider: p); return true | s.chooseSpeech(provider: p == "anthropic" ? nil : p); return true
# sweep-settingsai/speech-engine-openai-chooses-the-provider | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | s.chooseSpeech(provider: p); return true | s.chooseSpeech(provider: p == "openai" ? nil : p); return true
# sweep-settingsai/speech-engine-gemini-chooses-the-provider | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | s.chooseSpeech(provider: p); return true | s.chooseSpeech(provider: p == "gemini" ? nil : p); return true
# sweep-settingsai/speech-engine-openrouter-chooses-the-provider | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | s.chooseSpeech(provider: p); return true | s.chooseSpeech(provider: p == "openrouter" ? nil : p); return true
# sweep-settingsai/speech-open-shortcuts-opens-the-shortcuts-page | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | "mac-settings-ai.speech.shortcuts": { _ in s.openShortcuts(); return true }, | "mac-settings-ai.speech.shortcuts": { _ in true },
# sweep-settingsai/meaning-download-leaves-not-downloaded | Sources/Features/BackgroundWork/BackgroundWorkFeature.swift | [MeaningModelDoor.downloadId: { _ in MeaningModelStore.shared.startInBackground() }] | [MeaningModelDoor.downloadId: { _ in false }]
# sweep-settingsai/laya-download-starts-the-download | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out[ax + ".download"] = { _ in guard case .ready = s.modelState(entry) else { s.downloadModel(entry.id); return true }; return false } | out[ax + ".download"] = { _ in guard case .ready = s.modelState(entry) else { return true }; return false }
# sweep-settingsai/laya-cancel-stops-the-download | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | guard case .downloading = s.modelState(entry) else { return false } | if true { return true }
# sweep-settingsai/jev-key-field-takes-the-typed-key | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.decisions.jev.key.field"] = { _ in s.keyFocus = "jev"; return true } | out["mac-settings-ai.decisions.jev.key.field"] = { _ in true }
# sweep-settingsai/jev-connect-saves-the-key | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | s.connectJev(k); return true | return true
# sweep-settingsai/jev-allow-gives-the-consent | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.decisions.jev.consent.allow"] = { _ in s.setConsent(true); return true } | out["mac-settings-ai.decisions.jev.consent.allow"] = { _ in true }
# sweep-settingsai/jev-withdraw-takes-the-consent-back | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.decisions.jev.consent.withdraw"] = { _ in s.setConsent(false); return true } | out["mac-settings-ai.decisions.jev.consent.withdraw"] = { _ in true }
# sweep-settingsai/engine-off-chooses-no-model | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.decisions.engine.\(e.rawValue)"] = { _ in s.chooseEngine(e); return true } | out["mac-settings-ai.decisions.engine.\(e.rawValue)"] = { _ in e == .off ? true : { s.chooseEngine(e); return true }() }
# sweep-settingsai/engine-local-chooses-laya | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.decisions.engine.\(e.rawValue)"] = { _ in s.chooseEngine(e); return true } | out["mac-settings-ai.decisions.engine.\(e.rawValue)"] = { _ in e == .local ? true : { s.chooseEngine(e); return true }() }
# sweep-settingsai/engine-jev-asks-for-its-key | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.decisions.engine.\(e.rawValue)"] = { _ in s.chooseEngine(e); return true } | out["mac-settings-ai.decisions.engine.\(e.rawValue)"] = { _ in e == .jev ? true : { s.chooseEngine(e); return true }() }
# sweep-settingsai/page-which-tab-row-opens-the-page | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.decisions.open.\(id)"] = { _ in s.section = .page(id); return true } | out["mac-settings-ai.decisions.open.\(id)"] = { _ in id == "which-tab" ? true : { s.section = .page(id); return true }() }
# sweep-settingsai/page-needs-reply-row-opens-the-page | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.decisions.open.\(id)"] = { _ in s.section = .page(id); return true } | out["mac-settings-ai.decisions.open.\(id)"] = { _ in id == "needs-reply" ? true : { s.section = .page(id); return true }() }
# sweep-settingsai/page-sign-in-codes-row-opens-the-page | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.decisions.open.\(id)"] = { _ in s.section = .page(id); return true } | out["mac-settings-ai.decisions.open.\(id)"] = { _ in id == "sign-in-codes" ? true : { s.section = .page(id); return true }() }
# sweep-settingsai/page-vips-row-opens-the-page | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.decisions.open.\(id)"] = { _ in s.section = .page(id); return true } | out["mac-settings-ai.decisions.open.\(id)"] = { _ in id == "vips" ? true : { s.section = .page(id); return true }() }
# sweep-settingsai/page-notify-me-row-opens-the-page | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.decisions.open.\(id)"] = { _ in s.section = .page(id); return true } | out["mac-settings-ai.decisions.open.\(id)"] = { _ in id == "notify-me" ? true : { s.section = .page(id); return true }() }
# sweep-settingsai/page-labels-row-opens-the-page | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.decisions.open.\(id)"] = { _ in s.section = .page(id); return true } | out["mac-settings-ai.decisions.open.\(id)"] = { _ in id == "labels" ? true : { s.section = .page(id); return true }() }
# sweep-settingsai/page-back-returns-to-the-list | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | "mac-settings-ai.decisions.back": { _ in s.section = .decisions; return true }, | "mac-settings-ai.decisions.back": { _ in true },
# sweep-settingsai/page-add-rule-inserts-a-rule-first | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.decisions.page.\(id).add-rule"] = { _ in s.addRule(page: id); return true } | out["mac-settings-ai.decisions.page.\(id).add-rule"] = { _ in id == "which-tab" ? true : { s.addRule(page: id); return true }() }
# sweep-settingsai/page-add-model-inserts-the-model-entry | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.decisions.page.\(id).add-model"] = { _ in s.addModel(page: id); return true } | out["mac-settings-ai.decisions.page.\(id).add-model"] = { _ in id == "vips" ? true : { s.addModel(page: id); return true }() }
# sweep-settingsai/page-move-up-reorders-the-entries | Sources/Features/SettingsAIMac/AISettingsStore.swift | guard from != to, from < last, (0..<es.count).contains(to) else { return } | guard from != to, from < last, (0..<es.count).contains(to), id != "which-tab" else { return }
# sweep-settingsai/page-preview-lists-the-last-messages | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.decisions.page.\(id).preview"] = { _ in s.section = .page(id); s.runPreview(page: id); return true } | out["mac-settings-ai.decisions.page.\(id).preview"] = { _ in id == "which-tab" ? true : { s.section = .page(id); s.runPreview(page: id); return true }() }
# sweep-settingsai/page-apply-writes-the-preview | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.decisions.page.\(id).apply"] = { _ in s.apply(page: id); return true } | out["mac-settings-ai.decisions.page.\(id).apply"] = { _ in id == "which-tab" ? true : { s.apply(page: id); return true }() }
# sweep-settingsai/page-proposal-accept-adds-the-rule | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-settings-ai.decisions.page.\(id).proposal.accept"] = { _ in s.acceptProposal(page: id); return true } | out["mac-settings-ai.decisions.page.\(id).proposal.accept"] = { _ in id == "labels" ? true : { s.acceptProposal(page: id); return true }() }
# sweep-settingsai/page-entry-switch-turns-the-entry-off | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | { $0.enabled.toggle() }; return true | { _ in }; return true
# sweep-settingsai/page-question-field-takes-the-typed-words | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | s.questionFocus = (id, s.pages[i].entries[n - 1].id); return true | return true
# sweep-settingsai/page-trust-strict-sets-the-level | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | { $0.trust = levels[k - 1].1 }; return true | { if k != 1 { $0.trust = levels[k - 1].1 } }; return true
# sweep-settingsai/page-trust-eager-sets-the-level | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | { $0.trust = levels[k - 1].1 }; return true | { if k != 3 { $0.trust = levels[k - 1].1 } }; return true
# sweep-settingsai/page-trust-balanced-sets-the-level | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | { $0.trust = levels[k - 1].1 }; return true | { if k != 2 { $0.trust = levels[k - 1].1 } }; return true
# sweep-settingsai/page-label-menu-item-files-the-rule-under-the-label | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | { $0.outcome = tree[k - 1] }; return true | { if k != 2 { $0.outcome = tree[k - 1] } }; return true
# sweep-settingsai/page-proposal-dismiss-clears-the-proposal | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | guard s.proposals[id] != nil else { return false }; s.proposals[id] = nil; return true | guard s.proposals[id] != nil else { return false }; return true
# sweep-settingsai/agents-new-opens-a-draft-card | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | "mac-settings-ai.agents.new": { _ in s.section = .agents; s.questionFocus = nil; s.newAgent(); return true }, | "mac-settings-ai.agents.new": { _ in s.section = .agents; return true },
# sweep-settingsai/agents-form-link-goes-to-the-providers | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | "mac-settings-ai.agents.form.link": { _ in guard s.formMissing != nil else { return false }; s.section = .providers; return true }, | "mac-settings-ai.agents.form.link": { _ in guard s.formMissing != nil else { return false }; return true },
# sweep-settingsai/agents-name-field-takes-the-typed-name | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | "mac-settings-ai.agents.name": { _ in s.focusDraftName() }, | "mac-settings-ai.agents.name": { _ in s.keyFocus = "jev"; return s.draftAgent != nil },
# sweep-settingsai/agents-save-keeps-the-agent | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | "mac-settings-ai.agents.save": { _ in s.saveDraft(); return true }, | "mac-settings-ai.agents.save": { _ in true },
# sweep-settingsai/agents-duplicate-copies-the-last-agent | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | s.duplicate(a); return true | return true
# sweep-settingsai/agents-allow-read-toggles-the-permission | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | s.toggle(p, on: id); return true | w == "read" ? true : { s.toggle(p, on: id); return true }()
# sweep-settingsai/agents-allow-search-toggles-the-permission | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | s.toggle(p, on: id); return true | w == "search" ? true : { s.toggle(p, on: id); return true }()
# sweep-settingsai/agents-allow-draft-toggles-the-permission | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | s.toggle(p, on: id); return true | w == "draft" ? true : { s.toggle(p, on: id); return true }()
# sweep-settingsai/agents-allow-label-toggles-the-permission | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | s.toggle(p, on: id); return true | w == "label" ? true : { s.toggle(p, on: id); return true }()
# sweep-settingsai/agents-model-choice-sets-the-model | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | s.setModel(s.cards[n - 1].id, to: choices[k - 1]); return true | return true
# sweep-settingsai/palette-ai-settings-line-opens-the-window | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-palette.result.ai-settings"] = open   // the palette lane wins by registry order once it lands | out["mac-palette.result.ai-settings"] = { _ in false }
# sweep-settingsai/palette-decisions-line-opens-the-decisions-page | Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift | out["mac-palette.result.decisions"] = { m in s.section = .decisions; return WindowBridge.open(.macSettingsAI, in: m) } | out["mac-palette.result.decisions"] = { m in WindowBridge.open(.macSettingsAI, in: m) }


def run(c):
    out = []
    for name, _door, _w, fn in CASES:
        label = "%s/%s" % (SCREEN, name)
        try:
            r = fn(c)
            got, want = r[:2]
            more = " | evidence: %r" % (r[2],) if len(r) > 2 and got != want else ""   # round 8: a case's third value is printed on a red (it was dropped)
            out.append((label, "pass" if got == want else "fail", "expected %r, got %r%s" % (want, got, more)))
        except Unreadable as e:
            out.append((label, "unjudged", str(e)))
        except (KeyError, IndexError, TypeError) as e:
            out.append((label, "fail", "reply shape: %s %s" % (type(e).__name__, e)))
    return out


def mutant_lines():
    rows = []
    for line in open(os.path.abspath(__file__), encoding="utf-8"):
        m = re.match(r"# (" + SCREEN + r"/[\w-]+) \| (\S+) \| (.+?) \| (.+)$", line.rstrip("\n"))
        if m:
            rows.append(m.groups())
    return rows


def selftest():
    bad = []
    names = [n for n, *_ in CASES]
    clean = run(Shim(Model()))
    for label, st, d in clean:
        print("model %-44s %-8s %s" % (label.split("/", 1)[1][:44], st, d[:100]))
        bad += [] if st == "pass" else ["clean model: %s is %s: %s" % (label, st, d)]
    seen = 0
    for n in names:
        res = run(Shim(Model({"sw-" + n})))
        red = sorted([l.split("/", 1)[1] for l, st, _ in res if st == "fail"], key=lambda x: x != n)
        ok = red[:1] == [n] and set(red) <= {n} | set(DEPENDENT.get(n, ()))
        seen += ok
        print("broken %-46s red: %s" % (n, ", ".join(red) or "NONE"))
        bad += [] if ok else ["broken copy of %s: red cases %s" % (n, red)]
    declared = set(declared_mutants(os.path.abspath(__file__)))
    want = {"%s/%s" % (SCREEN, n) for n in names}
    lines = mutant_lines()
    if declared != want or len(lines) != len(names):
        bad.append("MUTANTS block: declared %d, cases %d, missing %s, stray %s" % (len(declared), len(want), sorted(want - declared), sorted(declared - want)))
    for label, path, find, repl in lines:
        text = open(os.path.join(ROOT, path), encoding="utf-8").read()
        if text.count(find) != 1:
            bad.append("%s: the find text occurs %d times in %s (must be once)" % (label, text.count(find), path))
        if find == repl:
            bad.append("%s: replace equals find" % label)
    for title, path, line, snippet, _why in FINDINGS:
        got = open(os.path.join(ROOT, path), encoding="utf-8").read().split("\n")[line - 1]
        if snippet.strip() not in got:
            bad.append("finding %s: line %d of %s reads %r, not the snippet" % (title, line, path, got.strip()[:80]))
    print("%d cases, %d seen red on their broken model, %d MUTANTS rows checked against the files (find occurs once), %d findings checked at their line" % (len(names), seen, len(lines), len(FINDINGS)))
    for b in bad:
        print("PROBLEM " + b)
    print("selftest: every check seen red on its broken model" if not bad else "selftest: %d problem(s)" % len(bad))
    return 1 if bad else 0


def main():
    if "--selftest" in sys.argv:
        return selftest()
    try:
        c = Ctl().connect()
    except CouldNotDrive as e:
        res = [("%s/%s" % (SCREEN, n), "unjudged", str(e)) for n, *_ in CASES]
    else:
        try:
            pre = globals().get("PRE_RUN")
            if pre:
                pre(c)
            res = run(c)
        finally:
            post = globals().get("POST_RUN")
            if post:
                post()
            c.close()
    print_cases({"cases": res})
    f, u = sum(r[1] == "fail" for r in res), sum(r[1] == "unjudged" for r in res)
    print("%d checks, %d failed, %d not judged" % (len(res), f, u))
    return 1 if f or u else 0


if __name__ == "__main__":
    sys.exit(main())
