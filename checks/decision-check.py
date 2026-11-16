#!/usr/bin/env python3
"""decision-check: J5.8 and J5.9 as docs/DECISION-MODEL.md VERSION 2 (signed 2026-11-07 19:16) defines them.

    python3 checks/decision-check.py              drive the running rig copy; exit 1 when F>0 or U>0
    python3 checks/decision-check.py --selftest   each check against a stand-in app, clean and with its mutant

What v2 says, and what each check holds it to: Settings > Decisions has one page per thing decided (not per method);
each page is ONE ordered list of rule entries and "ask the model" entries with a trust level, a default last; the first
entry that answers wins; below the trust level the list continues; the Engine block is Local / Jev / Off and Off skips
every model entry; each message carries a "why" naming the page and the entry. Not v1: there is no Rules page apart from
a Decision model page, and no layer order of rules, then model, then defaults.
Cases and the shipped lists come from fixtures/tests-j5.json (a literal copy of the doc's table). The Jev engine is a
stub on 127.0.0.1: its request log proves what leaves the Mac. Numbers (20 messages, 2000 body characters) come from
docs/CONSTANTS.md. The trust levels 80, 75 and 90 and the refit-at-35 floor are in DECISION-MODEL.md but NOT yet in
docs/CONSTANTS.md: a hand-back finding, logged in docs/lanes/tests-j5.md.
"""
import importlib.util
import json
import os
import sys
import time

_p = os.path.join(os.path.dirname(os.path.abspath(__file__)), "settingsaimac-check.py")
_s = importlib.util.spec_from_file_location("j5harness", _p)
H = importlib.util.module_from_spec(_s)
sys.modules["j5harness"] = H
_s.loader.exec_module(H)
A, FX, D, MSG, const, eq, act, soft, rows, core = H.A, H.FX, H.D, H.MSG, H.const, H.eq, H.act, H.soft, H.rows, H.core

S = H.Suite("decision")
CASES = {c["id"]: c for c in D["cases"]}
ENG = "mac-settings-ai.decisions.engine."
PAGE = "mac-settings-ai.decisions.page."
JEVPATH = "/v1/systemone"


# ---- the preview runs off the main actor (gap-decisions-real): its state is a served row of the page (`page.<id>.preview.state`: idle, running, done, refused), and a case
# that reads the rows right after the click waits on that state, never on time. The stand-in app serves the same row (the stand-in of settingsaimac-check.py has none, and
# that file is not edited here): done once the preview ran, `running` for ever under its mutant `preview-state-never-done`.
_stand_in_rows = H.FakeApp.rows_


def _rows_with_preview_state(self, ax):
    if ax.startswith(PAGE) and ax.endswith(".preview.state"):
        return [{"state": "running" if "preview-state-never-done" in self.m else ("done" if self.preview else "idle")}]
    if ax.startswith(PAGE) and ax.endswith(".teaching"):      # round 4: the engine's answer to Refit, as the banner says it
        return [dict(r, note=getattr(self, "refit_note", "")) for r in _stand_in_rows(self, ax)]
    return _stand_in_rows(self, ax)


H.FakeApp.rows_ = _rows_with_preview_state

# ---- the teaching block's "Make a rule from these" (DECISION-MODEL.md v2 §3, hid-ai round 3): the stand-in app's click, as the app's door
# `page.<id>.teaching.make-rule` does it: a rule naming what the corrections share goes first; refused when they share nothing. Its mutant
# `make-rule-click-dead` answers the click and adds nothing.
_stand_in_click = H.FakeApp.v_click


def _click_with_teaching(self, ax):
    if ax.startswith(PAGE) and ax.endswith(".teaching.refit"):
        # the engine refuses every refit until a calibration fit exists (fix-teachcore): the ask is counted and the banner says so; the
        # mutant `refit-claimed` says nothing (as if the refit had happened)
        if self.engine != "local" or getattr(self, "examples", 0) < D["teaching"]["local_refit_at"]:
            return {"ok": False, "error": "did nothing"}
        self.core_["decision_teaching/refits"] = (self.core_.get("decision_teaching/refits") or 0) + 1
        self.refit_note = "" if "refit-claimed" in self.m else "Whoosh cannot refit the model on this Mac yet: your corrections are kept for when it can."
        return {"ok": True}
    if ax.startswith(PAGE) and ax.endswith(".teaching.make-rule"):
        page = ax[len(PAGE):].split(".")[0]
        c = getattr(self, "corr", None)
        if not c or c["page"] != page or c["who"] == "mixed":
            return {"ok": False, "error": "did nothing"}
        if "make-rule-click-dead" not in self.m:
            self.lists[page].insert(0, {"kind": "rule", "word": "from @%s" % c["who"], "outcome": "Primary", "custom": True})
            self._renumber(page)
        return {"ok": True}
    return _stand_in_click(self, ax)


H.FakeApp.v_click = _click_with_teaching


def wait_preview(ctl, page, tries=300, every=0.02):
    """The harness's wait (settingsaimac-check.py `_wait_core`: read until the value leaves what it was, at most `tries` reads, a bound and never a sleep that waits it out) on the
    page's served preview state: returns once it says done or refused (the state it ends on is returned; `running` after `tries` reads is the answer when it never ends)."""
    state = None
    for _ in range(tries):
        got = rows(ctl, PAGE + page + ".preview.state")
        state = got[0]["state"] if got else None
        if state in ("done", "refused"):
            return state
        time.sleep(every)
    return state


def open_decisions(ctl):
    act(ctl, "click", A["palette_decisions"])


def set_engine(ctl, engine):
    act(ctl, "click", ENG + engine)


def use_jev(ctl):
    """Jev needs its key and the owner's consent before the core takes it as the engine: key, consent, then the choice."""
    act(ctl, "click", "mac-settings-ai.decisions.jev.key.field")
    act(ctl, "type", D["jev_key"])
    act(ctl, "click", "mac-settings-ai.decisions.jev.connect")
    act(ctl, "click", "mac-settings-ai.decisions.jev.consent.allow")
    set_engine(ctl, "jev")


def jev_ready(ctl, env, consent=True):
    open_decisions(ctl)
    # the core refuses Jev before its key and the owner's consent: key first, then consent, then the choice
    act(ctl, "click", "mac-settings-ai.decisions.engine.jev")
    act(ctl, "click", "mac-settings-ai.decisions.jev.key.field")
    act(ctl, "type", D["jev_key"])
    act(ctl, "click", "mac-settings-ai.decisions.jev.connect")
    if consent:
        act(ctl, "fixture", "consent", "jev", "yes")
        act(ctl, "click", "mac-settings-ai.decisions.jev.consent.allow")
    set_engine(ctl, "jev")


def decide(ctl, env, page, msgk, engine, says):
    open_decisions(ctl)
    if engine == "jev":
        jev_ready(ctl, env)
    else:
        set_engine(ctl, engine)
    env["jev"].answer = lambda body, s=says: H.jev_reply(body, s[0], s[1])      # Jev's real answer shape (round 5)
    act(ctl, "fixture", "decide", page, msgk, says[0], str(says[1]))
    return (rows(ctl, A["dec_result"]) or [{}])[0]  # an empty result is a red case, never a crash


def cases_ok(ctl, env, ids, keys=("entry", "outcome", "model_called")):
    """Each case on its own: a failure names the case. The set must be non-empty and every id real."""
    assert ids and all(i in CASES for i in ids), "case ids not in fixtures/tests-j5.json"
    bad = []
    for i in ids:
        c = CASES[i]
        H.reset(ctl, env)
        r = decide(ctl, env, c["page"], c["msg"], c["engine"], c["says"])
        for k in keys + (("why",) if "why" in c["expect"] else ()):
            if r.get(k) != c["expect"][k]:
                bad.append("%s %s: expected %r got %r" % (i, k, c["expect"][k], r.get(k)))
        if not r.get("why", "").startswith("%s · entry %d · " % (next(p["title"] for p in D["pages"] if p["id"] == c["page"]), c["expect"]["entry"])):
            bad.append("%s why %r does not name page and entry %d" % (i, r.get("why"), c["expect"]["entry"]))
    return not bad, "; ".join(bad) or "cases %s as v2 says" % ",".join(ids)


# ---- the structure v2 settles
@S.check("J5.8/one-page-per-decision", "pages-by-method", "the pages are Rules and Decision model (version 1)")
def c_pages(ctl, env):
    open_decisions(ctl)
    return eq([(p["id"], p["title"]) for p in rows(ctl, A["dec_pages"])], [(p["id"], p["title"]) for p in D["pages"]])


@S.check("J5.8/shipped-lists-per-page", "model-entry-missing", "the Which tab list ships without its model entry")
def c_shipped(ctl, env):
    open_decisions(ctl)
    got, bad = {}, []
    for pid, want in D["shipped"].items():
        have = [(r["kind"], r.get("trust")) for r in rows(ctl, PAGE + pid + ".entries")]
        if have != [(e["kind"], e.get("trust")) for e in want]:
            bad.append("%s: expected %r got %r" % (pid, [(e["kind"], e.get("trust")) for e in want], have))
    return not bad, "; ".join(bad) or "six lists as DECISION-MODEL.md section 1"


@S.check("J5.8/every-list-ends-with-a-default", "no-default", "the Which tab list has no closing default")
def c_default(ctl, env):
    open_decisions(ctl)
    bad = [pid for pid in D["shipped"] if [r["kind"] for r in rows(ctl, PAGE + pid + ".entries")][-1:] != ["default"]]
    return not bad, "pages whose last entry is not a default: %r" % bad


@S.check("J5.8/engine-choice-saved", "engine-not-saved", "the engine choice is not saved")
def c_engine(ctl, env):
    open_decisions(ctl)
    ids = [r["id"] for r in rows(ctl, A["dec_engine"])]
    got = []
    for e in ("local", "jev", "off"):
        use_jev(ctl) if e == "jev" else set_engine(ctl, e)
        got.append(core(ctl, "decision_model/provider"))
    # the Off segment is saved as the core's `rules` provider (AISettingsStore.swift chooseEngine: `case .off: m.provider = .rules`)
    return ids == ["local", "jev", "off"] and got == ["local", "jev", "rules"],"engines %r, saved %r" % (ids, got)


# ---- how a list decides
@S.check("J5.8/first-entry-that-answers-wins", "first-match-ignored", "every entry is tried and the last answer wins")
def c_first(ctl, env):
    return cases_ok(ctl, env, ["D1", "D2", "D7", "D8", "D12", "D15", "D16", "D17", "D18", "D19", "D20"])


@S.check("J5.8/rule-entries-never-call-the-model", "model-asked-anyway", "the model is asked even when a rule already answered")
def c_nocall(ctl, env):
    return cases_ok(ctl, env, ["D1", "D2", "D7", "D8", "D12", "D15", "D17"], keys=("model_called",))


@S.check("J5.8/model-answer-above-trust-decides", "why-without-entry", "the why drops the entry number")
def c_model(ctl, env):
    return cases_ok(ctl, env, ["D3", "D9", "D13"])


@S.check("J5.8/trust-level-is-inclusive", "trust-exclusive", "an answer exactly at the trust level is dropped")
def c_incl(ctl, env):
    return cases_ok(ctl, env, ["D5", "D11"])


@S.check("J5.8/below-trust-the-list-continues", "trust-ignored", "an answer below the trust level still decides")
def c_below(ctl, env):
    return cases_ok(ctl, env, ["D4", "D10", "D14"])


@S.check("J5.8/engine-off-skips-model-entries", "off-still-asks", "Off still asks the model")
def c_off(ctl, env):
    ok1, d1 = cases_ok(ctl, env, ["D6"])
    ok2, d2 = cases_ok(ctl, env, ["D3"], keys=("model_called",))  # control: the same message with a model on does call it
    return ok1 and ok2, "engine off: %s | control (local): %s" % (d1, d2)


# ---- editing a list
@S.check("J5.8/reordering-moves-the-model-entry", "reorder-ignored", "the move-up control does nothing")
def c_reorder(ctl, env):
    ro = D["reorder"]
    c0 = CASES["D1"]
    before = decide(ctl, env, ro["page"], c0["msg"], "local", ro["cases"][0]["says"])
    for _ in range(ro["move_entry"] - ro["to"]):
        act(ctl, "click", "%s%s.entry.%d.move-up" % (PAGE, ro["page"], ro["move_entry"] - _))
    first = rows(ctl, PAGE + ro["page"] + ".entries")[0]["kind"]
    bad = []
    for c in ro["cases"]:
        r = decide(ctl, env, ro["page"], c["msg"], "local", c["says"])
        if (r["entry"], r["outcome"]) != (c["expect"]["entry"], c["expect"]["outcome"]):
            bad.append("%s: expected %r got %r" % (c["id"], (c["expect"]["entry"], c["expect"]["outcome"]), (r["entry"], r["outcome"])))
    ctrl = (before["entry"], before["outcome"]) == (1, "Primary")  # control: before the move the VIP rule answers
    return ctrl and first == "model" and not bad, "control %r, first entry %r, %s" % (ctrl, first, "; ".join(bad) or "reordered cases as expected")


@S.check("J5.9/a-new-rule-lands-first-and-wins", "rule-appended-at-end", "a new rule is added after the model entry")
def c_newrule(ctl, env):
    nr = D["new_rule"]
    before = decide(ctl, env, nr["page"], nr["msg"], "local", nr["says"])
    act(ctl, "fixture", "add-rule", nr["page"], nr["words"], nr["outcome"])
    after = decide(ctl, env, nr["page"], nr["msg"], "local", nr["says"])
    ok = (before["entry"], before["outcome"]) == (nr["before"]["entry"], nr["before"]["outcome"]) and (after["entry"], after["outcome"]) == (nr["after"]["entry"], nr["after"]["outcome"])
    return ok, "before %r, after %r" % ((before["entry"], before["outcome"]), (after["entry"], after["outcome"]))


@S.check("J5.9/rule-from-message-prefilled", "prefill-empty", "Create a rule from this message opens an empty editor")
def c_prefill(ctl, env):
    pf = D["prefill"]
    act(ctl, "click", A["list_row_1_create_rule"])
    surf = ctl.uistate().get("surface")
    ed = rows(ctl, A["dec_rule_editor"])
    ok = surf == "mac-settings-ai" and len(ed) == 1 and ed[0]["from"] == pf["from"] and ed[0]["domain"] == pf["domain"]
    return ok, "surface %r, editor %r" % (surf, ed)


# ---- preview and teaching
@S.check("J5.8/preview-runs-on-20-and-writes-nothing", "preview-writes", "the preview run changes the stored decisions")
def c_preview(ctl, env):
    open_decisions(ctl)
    act(ctl, "click", PAGE + "which-tab.preview")
    wait_preview(ctl, "which-tab")
    pv = rows(ctl, PAGE + "which-tab.preview")
    applied = core(ctl, "decision_preview/applied")
    act(ctl, "click", PAGE + "which-tab.apply")
    after = core(ctl, "decision_preview/applied")  # control: Apply is what writes
    per_row = all(isinstance(r["entry"], int) and r["entry"] >= 1 and r["outcome"] for r in pv)
    return len(pv) == const("DECISION_PREVIEW_N") and per_row and (applied, after) == (False, True), "%d rows (want %d), each names entry and outcome %r, applied before/after Apply %r" % (len(pv), const("DECISION_PREVIEW_N"), per_row, (applied, after))


@S.check("J5.8/preview-state-is-served-and-ends-done", "preview-state-never-done", "the preview never leaves running")
def c_preview_state(ctl, env):
    open_decisions(ctl)
    before = [r["state"] for r in rows(ctl, PAGE + "which-tab.preview.state")]      # the control: the page is idle before the click
    act(ctl, "click", PAGE + "which-tab.preview")
    final = wait_preview(ctl, "which-tab")
    return (before, final) == (["idle"], "done"), "state before the click %r, after waiting on it %r" % (before, final)


@S.check("J5.8/preview-count-is-the-constant", "preview-n-wrong", "the preview runs on one message too few")
def c_previewn(ctl, env):
    open_decisions(ctl)
    act(ctl, "click", PAGE + "which-tab.preview")
    wait_preview(ctl, "which-tab")
    return eq(len(rows(ctl, PAGE + "which-tab.preview")), const("DECISION_PREVIEW_N"))


@S.check("J5.8/unticking-vip-removes-it-from-the-preview", "untick-ignored", "unticking the VIP rule leaves VIP mail decided by that rule in the preview")
def c_untick_vip(ctl, env):
    page = "which-tab"
    n = next(e["n"] for e in D["shipped"][page] if e["kind"] == "rule" and e["word"] == "VIP")      # the VIP rule's place in the list, from the shipped list (docs/DECISION-MODEL.md section 1)
    open_decisions(ctl)
    act(ctl, "click", PAGE + page + ".preview")
    wait_preview(ctl, page)
    before = [r["message"] for r in rows(ctl, PAGE + page + ".preview") if r["entry"] == n]      # the control: before the untick, VIP mail names this entry
    act(ctl, "click", "%s%s.entry.%d.enabled" % (PAGE, page, n))
    switch = [r.get("enabled") for r in rows(ctl, PAGE + page + ".entries") if r["n"] == n]
    act(ctl, "click", PAGE + page + ".preview")
    wait_preview(ctl, page)
    pv = rows(ctl, PAGE + page + ".preview")
    after = [r["message"] for r in pv if r["entry"] == n]
    act(ctl, "click", "%s%s.entry.%d.enabled" % (PAGE, page, n))      # put the switch back for the checks that follow
    back = [r.get("enabled") for r in rows(ctl, PAGE + page + ".entries") if r["n"] == n]
    ok = len(before) > 0 and switch == [False] and after == [] and len(pv) == const("DECISION_PREVIEW_N") and back == [True]
    return ok, "entry %d (VIP): %d of the preview's rows name it before the untick (control), its switch reads %r after, %d rows name it in the next preview of %d rows, the switch reads %r once ticked again" % (
        n, len(before), switch, len(after), len(pv), back)


@S.check("J5.8/editing-the-prompt-then-run-shows-20-rows-with-a-verdict-and-a-tab", "preview-row-without-tab", "after the prompt is edited the preview rows lack their verdict or their tab")
def c_prompt_preview(ctl, env):
    page = "which-tab"
    n = next(e["n"] for e in D["shipped"][page] if e["kind"] == "model")      # the model entry: its question is the prompt
    typed = " Invoices are Updates."
    open_decisions(ctl)
    old = [r["text"] for r in rows(ctl, PAGE + page + ".entries") if r["n"] == n]
    act(ctl, "click", "%s%s.entry.%d.question" % (PAGE, page, n))
    act(ctl, "type", typed)
    new = [r["text"] for r in rows(ctl, PAGE + page + ".entries") if r["n"] == n]
    act(ctl, "click", PAGE + page + ".preview")
    wait_preview(ctl, page)
    pv = rows(ctl, PAGE + page + ".preview")
    tabs = {t["name"] for t in json.load(open(os.path.join(H.ROOT, "docs", "design", "fixtures", "mac-main-sample.json"), encoding="utf-8"))["tabs"]}
    no_verdict = [r["message"] for r in pv if not r.get("decided_by")]      # the verdict: which entry decided it and how
    no_tab = [r["message"] for r in pv if r.get("outcome") not in tabs]      # the tab the message would land in: one of the design file's tabs
    edited = len(old) == 1 and new == [old[0] + typed]      # the control: the prompt did change
    ok = edited and len(pv) == const("DECISION_PREVIEW_N") and not no_verdict and not no_tab
    return ok, "prompt of entry %d %s; the preview has %d rows (want %d); rows without a verdict %s, rows whose outcome is no tab of the design file %s" % (
        n, "changed" if edited else "did not change (%r -> %r)" % (old, new), len(pv), const("DECISION_PREVIEW_N"), no_verdict or "none", no_tab or "none")


@S.check("J5.8/make-a-rule-only-for-a-shared-sender", "make-rule-for-mixed", "corrections from different senders offer a rule")
def c_makerule(ctl, env):
    t = D["teaching"]
    open_decisions(ctl)
    act(ctl, "fixture", "corrections", t["page"], str(t["corrections"]), t["make_rule_domain"])
    same = rows(ctl, PAGE + t["page"] + ".teaching")[0]
    act(ctl, "fixture", "corrections", t["page"], str(t["corrections"]), "mixed")
    mixed = rows(ctl, PAGE + t["page"] + ".teaching")[0]
    return (same["count"], same["offers"], mixed["offers"]) == ("%d corrections" % t["corrections"], ["make-rule"], []), "shared %r, mixed %r" % (same, mixed)


@S.check("J5.8/make-rule-from-corrections-puts-the-rule-first", "make-rule-click-dead", "Make a rule from these adds no rule")
def c_makerule_click(ctl, env):
    t = D["teaching"]
    open_decisions(ctl)
    act(ctl, "fixture", "corrections", t["page"], str(t["corrections"]), t["make_rule_domain"])
    before = rows(ctl, PAGE + t["page"] + ".entries")
    act(ctl, "click", PAGE + t["page"] + ".teaching.make-rule")
    after = rows(ctl, PAGE + t["page"] + ".entries")
    first = after[0] if after else {}
    ok = len(after) == len(before) + 1 and first.get("kind") == "rule" and t["make_rule_domain"] in str(first.get("text", ""))
    return ok, "entries %d -> %d; the first now %r (want a rule naming %s)" % (len(before), len(after), (first.get("kind"), first.get("text")), t["make_rule_domain"])


@S.check("J5.8/refit-offered-from-35-examples-local", "refit-at-34", "the refit is offered at 34 examples")
def c_refit(ctl, env):
    t = D["teaching"]
    open_decisions(ctl)
    set_engine(ctl, "local")
    got = []
    for n in (t["local_refit_at"] - 1, t["local_refit_at"]):
        act(ctl, "fixture", "examples", str(n))
        got.append("refit" in rows(ctl, PAGE + t["page"] + ".teaching")[0]["offers"])
    return got == [False, True], "refit offered at %d and %d examples: %r" % (t["local_refit_at"] - 1, t["local_refit_at"], got)


@S.check("J5.8/refit-says-it-is-not-built-and-claims-nothing", "refit-claimed", "Refit the model answers nothing, as if the refit had happened")
def c_refit_refused(ctl, env):
    """Round 4 (fix-teachcore): Refit runs `Command.refitDecisionModel` on either engine; the engine counts the ask (`decision_teaching/refits`) and
    refuses it as not built yet. The banner must say why in a sentence (`page.<id>.teaching` field `note`), never stay silent as if it refitted."""
    t = D["teaching"]
    open_decisions(ctl)
    set_engine(ctl, "local")
    act(ctl, "fixture", "examples", str(t["local_refit_at"]))
    offered = "refit" in rows(ctl, PAGE + t["page"] + ".teaching")[0]["offers"]      # the control: the refit is offered
    before = H.core_or_none(ctl, "decision_teaching/refits") or 0
    act(ctl, "click", PAGE + t["page"] + ".teaching.refit")
    after = H.core_or_none(ctl, "decision_teaching/refits") or 0
    note = rows(ctl, PAGE + t["page"] + ".teaching")[0].get("note", "")
    return offered and after == before + 1 and bool(note), "offered %r; refits asked %r -> %r; the banner says %r" % (offered, before, after, note)


@S.check("J5.8/jev-never-offers-a-refit", "refit-for-jev", "Jev offers a refit it cannot do")
def c_jevrefit(ctl, env):
    t = D["teaching"]
    open_decisions(ctl)
    act(ctl, "fixture", "examples", str(t["local_refit_at"] + 10))
    set_engine(ctl, "local")
    control = "refit" in rows(ctl, PAGE + t["page"] + ".teaching")[0]["offers"]
    use_jev(ctl)
    jev = "refit" in rows(ctl, PAGE + t["page"] + ".teaching")[0]["offers"]
    return (control, jev) == (True, False), "refit offered with local %r, with Jev %r" % (control, jev)


# ---- Jev: what leaves the Mac
def jev_requests(env):
    return [r for r in env["jev"].requests if r["path"] == JEVPATH]


@S.check("J5.8/no-mail-reaches-jev-before-consent", "model-before-consent", "the first message goes out before the owner agrees")
def c_consent(ctl, env):
    c = D["jev_cases"][0]
    jev_ready(ctl, env, consent=False)
    env["jev"].answer = lambda body: H.jev_reply(body, c["says"][0], c["says"][1])
    act(ctl, "fixture", "decide", c["page"], c["msg"], c["says"][0], str(c["says"][1]))
    before = (len(jev_requests(env)), [r["state"] for r in rows(ctl, A["dec_consent"])])
    act(ctl, "fixture", "consent", "jev", "yes")
    act(ctl, "fixture", "decide", c["page"], c["msg"], c["says"][0], str(c["says"][1]))
    after = len(jev_requests(env))  # control: the same call after consent does reach the stub
    return (before, after) == ((0, ["asked"]), 1), "before consent (requests, consent) %r; after consent requests %r" % (before, after)


@S.check("J5.8/jev-body-has-only-the-listed-fields", "attachments-sent", "attachments travel to Jev")
def c_fields(ctl, env):
    c = D["jev_cases"][0]
    r = decide(ctl, env, c["page"], c["msg"], "jev", c["says"])
    reqs = jev_requests(env)
    # round 5: Jev's real body is {state, model, questions}; the mail's fields are the "name: value" lines of `state` before its blank line (the body after it).
    # Before: the body's own keys were the fields. A key added beside the three (an api_key, attachments) is red.
    body = reqs[-1]["body"] if reqs else {}
    keys = set(body) if isinstance(body, dict) else set()
    head = str(body.get("state", "")).split("\n\n", 1)[0] if isinstance(body, dict) else ""
    fields = {line.split(": ", 1)[0].replace("list header", "list_header") for line in head.split("\n") if line}
    ok = (bool(reqs) and keys == {"state", "model", "questions"} and fields <= set(D["sent_fields_allowed"]) and "subject" in fields
          and (r["entry"], r["outcome"]) == (c["expect"]["entry"], c["expect"]["outcome"]))
    return ok, "keys sent %r, state fields %r (allowed %r), result %r" % (sorted(keys), sorted(fields), D["sent_fields_allowed"], (r["entry"], r["outcome"]))


@S.check("J5.8/jev-key-in-header-never-in-body", "key-in-body", "the Jev key is inside the request body")
def c_keyhdr(ctl, env):
    c = D["jev_cases"][0]
    decide(ctl, env, c["page"], c["msg"], "jev", c["says"])
    reqs = jev_requests(env)
    ok = bool(reqs) and reqs[-1]["auth"] == "Bearer " + D["jev_key"] and D["jev_key"] not in reqs[-1]["raw"]
    return ok, "requests %d, auth %r, key in body %r" % (len(reqs), reqs[-1]["auth"] if reqs else None, bool(reqs) and D["jev_key"] in reqs[-1]["raw"])


@S.check("J5.8/jev-body-cut-at-the-constant", "body-not-truncated", "the whole body is sent")
def c_trunc(ctl, env):
    got, bad = {}, []
    for c in D["jev_cases"][1:]:
        H.reset(ctl, env)
        decide(ctl, env, c["page"], c["msg"], "jev", c["says"])
        reqs = jev_requests(env)
        got[c["id"]] = len(str(reqs[-1]["body"].get("state", "")).split("\n\n", 1)[-1]) if reqs and "\n\n" in str(reqs[-1]["body"].get("state", "")) else None   # the text after the state's blank line
        if got[c["id"]] != c["expect"]["body_sent"]:
            bad.append("%s: expected %r got %r" % (c["id"], c["expect"]["body_sent"], got[c["id"]]))
    limit_ok = const("DECISION_BODY_CHARS") == 2000  # the fixture's expectation is this constant, read once
    return limit_ok and not bad, "body characters sent %r; %s" % (got, "; ".join(bad) or "2500 cut to the limit, 1500 sent whole")


@S.check("J5.8/jev-key-in-keychain-not-db", "jev-key-in-db", "the Jev key is stored in the database")
def c_jevkey(ctl, env):
    jev_ready(ctl, env, consent=False)
    # the key is never read back: the core answers `decision_model/key` with "no core value at", which for this path IS the answer None
    # (settingsaimac-check.py core_or_none; the same reading as J5's key-in-keychain-not-db). A stored key served there still reads as the key: red.
    got = (core(ctl, "provider_credential/jev/stored"), H.core_or_none(ctl, "decision_model/key"))
    return eq(got, (True, None))


if __name__ == "__main__":
    sys.exit(H.main(S))
