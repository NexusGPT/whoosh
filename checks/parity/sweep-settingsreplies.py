#!/usr/bin/env python3
"""sweep-settingsreplies: the control sweep of Settings > Replies (`mac-settings-replies`: the six panes Auto draft, On demand, Sources, Retrieval, Voice,
The prompt, and the page Which messages). Owner, 2026-11-11: "you haven't properly tested every button/feature".

ONE case per control: it clicks that control once through its door (`mac-settings-replies.<door>`, the ids RepliesDoors.swift and RepliesTextDoors.swift
register and `mac-settings-replies.controls` lists), then reads the outcome twice, from the core (`core state reply/...`) and from what the pane draws
(`rows mac-settings-replies.state`, `.sources`, `.editors`, `.place`). A control that is dead makes its case red (a refused click or an unchanged value),
never skipped. A family of controls the app builds from its data (the tone choices, the label ticks, the account cards and their folder tiles, the local
folders, the calendars, the tone, note and version rows) is ONE case that clicks every member once, its members listed by the app's own `controls` row.
The sample texts the text doors type are the ones RepliesDoors.swift types (the socket's `type` verb reaches the window's search field only).

    python3 checks/parity/sweep-settingsreplies.py              drive the running rig copy (the hidden test copy, --fixture-core: no network, no real account)
    python3 checks/parity/sweep-settingsreplies.py --dry        the same code with no socket: every case is `unjudged`, the count is exact
    python3 checks/parity/sweep-settingsreplies.py --selftest   every case against a model of the pane (checks/settingsreplies-check.py FakeApp, imported, not edited),
                                                                clean, then once per case against its own broken copy: each must go red

Prints one `name  pass|fail|unjudged  detail` line per case (print_cases of journeys/lib/harness.py), the MUTANTS table, then 'N checks, F failed, U not judged'.
Findings (a control with no door or no readable outcome) are in docs/lanes/gap-sweep-b.md and in the table at the end of docs/COVERAGE.md.
"""
import fnmatch
import importlib.util
import json
import os
import shutil
import socketserver
import sys
import tempfile
import threading

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, Unreadable                                     # noqa: E402
from lib.harness import Journey, run_journey, print_cases, declared_mutants   # noqa: E402

_spec = importlib.util.spec_from_file_location("sr", os.path.join(ROOT, "checks", "settingsreplies-check.py"))
sr = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(sr)                                            # read for its reads, helpers and FakeApp; never edited

ID = "par-settingsreplies-sweep"
TITLE = "Settings > Replies: every control clicked once"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "each control of the six panes and of Which messages changes the value the core holds and the words the pane draws"
P = sr.P
FACTS = sr.FACTS
CASES = []      # (name, doors (patterns the case covers), noop (the door the broken copy disables), fn)


def case(name, doors, noop, fn):
    CASES.append((name, list(doors), noop, fn))


# ---------------------------------------------------------------- reads and acts
def hit(ctl, door):
    """One click on a door. A refusal ("did nothing", "nothing to click") is the app's answer: False, and the outcome read after it then shows the dead control."""
    try:
        ctl.click("%s.%s" % (P, door))
        return True
    except Unreadable as e:
        if "nothing" in str(e):
            return False
        raise


def need(cond, words):
    if not cond:
        raise sr.Red(words)


state, core, controls = sr.state, sr.core, sr.controls


def family(ctl, prefix, suffix=""):
    return [d for d in controls(ctl) if d.startswith(prefix) and d.endswith(suffix) and (suffix or not d.endswith(".remove"))]


# ---------------------------------------------------------------- case builders
def flip(name, door, read, doors=None):
    """A switch: one click turns over the value the core holds and the value the pane draws."""
    def fn(ctl):
        c0, s0 = read(ctl)
        hit(ctl, door)
        c1, s1 = read(ctl)
        need(c1 == (not c0) and s1 == c1, "%s: core %r -> %r, the pane draws %r" % (door, c0, c1, s1))
        return "%s: core %r -> %r, pane draws %r" % (door, c0, c1, s1)
    case(name, doors or [door], door, fn)


def choose(name, door, other, read, want_value):
    """A choice: another choice is made first (setup, not the control under test), then this one once; core and pane both read the value."""
    def fn(ctl):
        hit(ctl, other)
        hit(ctl, door)
        c, s = read(ctl)
        need(c == want_value and s == want_value, "%s: core %r, the pane draws %r, expected %r" % (door, c, s, want_value))
        return "%s: core %r, pane draws %r" % (door, c, s)
    case(name, [door], door, fn)


def opens(name, door, popover=None, page=None, before=None):
    """A row or button that opens a popover or goes to a page."""
    def fn(ctl):
        if before:
            before(ctl)
        hit(ctl, door)
        ui = state(ctl)["ui"]
        if page:
            need(sr.page(ctl) == page and ui["page"] == page, "%s: the page shows %r (the store says %r), expected %r" % (door, sr.page(ctl), ui["page"], page))
            return "%s: page %s" % (door, page)
        need(ui["popover"] == popover, "%s: popover %r, expected %r" % (door, ui["popover"], popover))
        return "%s: popover %s" % (door, popover)
    case(name, [door], door, fn)


# ---------------------------------------------------------------- pages and the three links of each use
for _p in ["auto", "filter", "on-demand", "sources", "retrieval", "voice", "prompt"]:
    opens("page-" + _p, "page." + _p, page=_p, before=(lambda ctl, p=_p: hit(ctl, "page.voice" if p != "voice" else "page.auto")))

def _popover_open_then_closed(ctl):
    hit(ctl, "auto.try")
    open_ = state(ctl)["ui"]["popover"]
    hit(ctl, "popover.close")
    ui = state(ctl)["ui"]["popover"]
    need(open_ == "trial" and ui is None, "popover.close: the trial popover was %r, after the click %r" % (open_, ui))
    return "popover.close: trial -> closed"


case("popover-close", ["popover.close"], "popover.close", _popover_open_then_closed)


def on_screen(ctl, page):
    """A key goes where a person presses it: the Settings window open on this Replies page. The reset before each case closes every window
    (FixtureEvents.reset), and a door click opens none, so main88 sent Escape and ⌘N to the inbox (state surface=mac-main): the keys were
    never asked of the pane. The window's own doors: the page's sidebar row, then `mac-settings-window.open` (it opens on the pane chosen)."""
    for ax in ("mac-settings-window.pane-row.replies.%s" % page, "mac-settings-window.open"):
        try:
            ctl.click(ax)
        except Unreadable:
            pass


def _popover_escape(ctl):
    """F-rep-1 (turned into a case by fix-screens): Esc closes an open popover (SettingsRepliesMacFeature.keyBindings "Escape")."""
    on_screen(ctl, "auto")
    hit(ctl, "auto.try")
    open_ = state(ctl)["ui"]["popover"]
    ctl.key("Escape")
    after = state(ctl)["ui"]["popover"]
    if after is not None:
        hit(ctl, "popover.close")              # put the screen back
    need(open_ == "trial" and after is None, "Escape: the trial popover was %r, after the key %r" % (open_, after))
    return "Escape: trial popover -> closed"


case("popover-escape-closes-it", [], "key.escape", _popover_escape)

flip("auto-enabled", "auto.enabled", lambda c: (core(c, "reply/use/auto")["enabled"], state(c)["auto_enabled"]))
flip("auto-match-thread", "auto.match-thread", lambda c: (core(c, "reply/use/auto")["match_thread"], state(c)["auto_match_thread"]))
flip("on-demand-match-thread", "on-demand.match-thread", lambda c: (core(c, "reply/use/on_demand")["match_thread"], state(c)["on_demand_match_thread"]))
flip("on-demand-instruction-before", "on-demand.instruction-before", lambda c: (core(c, "reply/on_demand")["instruction_before"], state(c)["instruction_before"]))
flip("on-demand-change-after", "on-demand.change-after", lambda c: (core(c, "reply/on_demand")["change_after"], state(c)["change_after"]))
for _k in ["entry", "ai", "exit"]:
    opens("auto-step-" + _k, "auto.step." + _k, page="filter", before=lambda ctl: hit(ctl, "page.voice"))
for _k in ["auto", "on-demand"]:
    opens(_k + "-sources-row", _k + ".sources", page="sources", before=lambda ctl: hit(ctl, "page.voice"))
    opens(_k + "-prompt-row", _k + ".prompt", page="prompt", before=lambda ctl: hit(ctl, "page.voice"))


def _trial(name, door, kind):
    def fn(ctl):
        hit(ctl, door)
        ui = state(ctl)["ui"]
        need(ui["popover"] == "trial" and ui["trial_kind"] == kind, "%s: popover %r, trial %r, expected trial of %s" % (door, ui["popover"], ui["trial_kind"], kind))
        return "%s: popover trial, kind %s" % (door, kind)
    case(name, [door], door, fn)


_trial("auto-try", "auto.try", "auto")
_trial("retrieval-try", "retrieval.try", "on_demand")


def _tones(use, path, field):
    def fn(ctl):
        doors = family(ctl, use + ".tone.")
        need(len(doors) >= 3, "%s: %d tone choices, expected my voice and two tones" % (use, len(doors)))
        seen = []
        for i, d in enumerate(doors):
            hit(ctl, doors[i - 1])          # another tone first, so a dead door cannot pass on the value it already holds
            hit(ctl, d)
            tone = d.split(".tone.", 1)[1]
            c, s = core(ctl, path)["tone_id"], state(ctl)[field]
            need(c == tone and s == tone, "%s: core holds %r, the pane draws %r" % (d, c, s))
            seen.append(tone)
        return "%s: each of %s chosen, core and pane agree" % (use, seen)
    case(use + "-tone-choices", [use + ".tone.*"], use + ".tone.tone-formal", fn)


_tones("auto", "reply/use/auto", "auto_tone")
_tones("on-demand", "reply/use/on_demand", "on_demand_tone")

# ---------------------------------------------------------------- Which messages
for _k in ["entry", "ai", "exit"]:
    flip("filter-%s-switch" % _k, "filter.%s.switch" % _k, lambda c, k=_k: (core(c, "reply/filter")[k]["enabled"], state(c)["%s_on" % k]))


def _scope_all(ctl):
    fam = family(ctl, "filter.scope.label.")
    need(fam, "no label tick listed")
    hit(ctl, fam[0])
    mid = core(ctl, "reply/filter")["scope"]
    hit(ctl, "filter.scope.all")
    c, s = core(ctl, "reply/filter")["scope"], state(ctl)["scope_all"]
    need(c["all"] is True and s is True and not c["label_ids"], "filter.scope.all: core %r (before: %r), the pane draws all=%r" % (c, mid, s))
    return "filter.scope.all: scope %r -> all labels" % (mid,)


case("filter-scope-all", ["filter.scope.all"], "filter.scope.all", _scope_all)


def _labels(ctl):
    done = []
    for d in family(ctl, "filter.scope.label."):
        lid = d[len("filter.scope.label."):]
        before = lid in core(ctl, "reply/filter")["scope"]["label_ids"]
        hit(ctl, d)
        after = core(ctl, "reply/filter")["scope"]
        need((lid in after["label_ids"]) == (not before) and (lid in state(ctl)["scope_label_ids"]) == (lid in after["label_ids"]),
             "%s: ticked %r -> %r in the core, the pane draws %r" % (d, before, lid in after["label_ids"], lid in state(ctl)["scope_label_ids"]))
        done.append(lid)
    need(done, "no label tick listed")
    return "label ticks %s each turned over, core and pane agree" % done


case("filter-scope-labels", ["filter.scope.label.*"], "filter.scope.label.needs-reply", _labels)

for _k in ["entry", "exit"]:
    for _m, _o in [("all", "any"), ("any", "all")]:
        choose("filter-%s-match-%s" % (_k, _m), "filter.%s.match.%s" % (_k, _m), "filter.%s.match.%s" % (_k, _o),
               lambda c, k=_k: (core(c, "reply/filter")[k]["rule"]["match"], state(c)["%s_match" % k]), _m)
    for _f in FACTS:
        for _s in ["any", "yes", "no"]:
            choose("filter-%s-fact-%s-%s" % (_k, _f.replace("_", "-"), _s), "filter.%s.fact.%s.%s" % (_k, _f, _s), "filter.%s.fact.%s.%s" % (_k, _f, "yes" if _s != "yes" else "no"),
                   lambda c, k=_k, f=_f: (core(c, "reply/filter")[k]["rule"]["facts"][f], state(c)["%s_facts" % k][f]), _s)


def _sender(ctl):
    hit(ctl, "filter.entry.sender")
    rows = core(ctl, "reply/filter")["entry"]["rule"]["rows"]
    got = state(ctl)["sender"]
    need({"kind": "from", "words": sr.SENDER} in rows and got == sr.SENDER, "filter.entry.sender: core rows %r, the pane draws %r" % (rows, got))
    return "filter.entry.sender: core rows %r, pane draws %r" % (rows, got)


def _sender_clear(ctl):
    hit(ctl, "filter.entry.sender")
    need(state(ctl)["sender"] == sr.SENDER, "setup: the sender is %r" % state(ctl)["sender"])
    hit(ctl, "filter.entry.sender.clear")
    rows = core(ctl, "reply/filter")["entry"]["rule"]["rows"]
    need(rows == [] and state(ctl)["sender"] == "", "filter.entry.sender.clear: core rows %r, the pane draws %r" % (rows, state(ctl)["sender"]))
    return "filter.entry.sender.clear: core rows [], pane draws ''"


case("filter-entry-sender", ["filter.entry.sender"], "filter.entry.sender", _sender)
case("filter-entry-sender-clear", ["filter.entry.sender.clear"], "filter.entry.sender.clear", _sender_clear)
for _r, _f in [("subject", "subject"), ("body", "body"), ("rest", "rest_of_thread"), ("attachments", "attachments")]:
    flip("filter-ai-reads-" + _r, "filter.ai.reads." + _r, lambda c, r=_r, f=_f: (core(c, "reply/filter")["ai"][f], state(c)["reads"][r]))


# ---------------------------------------------------------------- Sources
def tile(ctl, ax):
    return [r for r in ctl.rows("%s.sources" % P) if r["ax"] == "%s.%s" % (P, ax)][0]


def _picker(door):
    def fn(ctl):
        asks = sr._asks(ctl)
        hit(ctl, door)
        asked = state(ctl)["ui"]["picker_open"] or sr._asks(ctl) == asks + 1
        hit(ctl, "sources.picker.cancel")         # the system's own Cancel: a seam, not a control of the pane
        need(asked, "%s: no folder chooser asked for" % door)
        return "%s: the folder chooser was asked for" % door
    return fn


case("sources-add", ["sources.add"], "sources.add", _picker("sources.add"))
case("sources-add-folder", ["sources.add-folder"], "sources.add-folder", _picker("sources.add-folder"))


def _add_command_n(ctl):
    """F-rep-2 (turned into a case by fix-screens): the key of Add a source, ⌘N (SourcesPane.swift `.keyboardShortcut("n", modifiers: .command)`), a real key event."""
    on_screen(ctl, "sources")
    hit(ctl, "page.sources")
    asks = sr._asks(ctl)
    try:
        r = ctl.send("keyevent", "⌘n") or {}
    except Unreadable as e:
        raise Unreadable("keyevent ⌘n: %s" % e)
    asked = state(ctl)["ui"]["picker_open"] or sr._asks(ctl) == asks + 1
    hit(ctl, "sources.picker.cancel")             # the system's own Cancel: a seam, not a control of the pane
    # round 7: the verb's own report names where the key went (main93 still red after the pane's ⌘N binding: the route decides the next fix)
    seen = {k: r.get(k) for k in ("window_id", "focus", "route", "handled", "seen", "key_before")} if isinstance(r, dict) else r
    need(asked, "⌘N on the Sources page: no folder chooser asked for; the key verb said %r" % (seen,))
    return "⌘N: the folder chooser was asked for"


case("sources-add-command-n", [], "key.cmd-n", _add_command_n)


def _add_sample(ctl):
    n = len(core(ctl, "reply/sources")["local_folders"])
    hit(ctl, "sources.add-folder.sample")
    paths = [s["source"]["path"] for s in core(ctl, "reply/sources")["local_folders"]]
    need(sr.FOLDER in paths and len(paths) == n + 1, "sources.add-folder.sample: the folders are %r (%d before)" % (paths, n))
    return "sources.add-folder.sample: %s is a source (%d -> %d folders)" % (sr.FOLDER, n, len(paths))


case("sources-add-folder-sample", ["sources.add-folder.sample"], "sources.add-folder.sample", _add_sample)


TYPED_FOLDER = "~/Typed/Notes"


def _typed_folder(ctl):
    """F-rep-3: the typed-words door beside the fixed sample: focus the folder field, `type` the words, commit; the folder is a source."""
    n = len(core(ctl, "reply/sources")["local_folders"])
    hit(ctl, "sources.add-folder.typed.focus")
    ctl.type(TYPED_FOLDER)
    said = ctl.rows(P + ".typed")
    hit(ctl, "sources.add-folder.typed.commit")
    paths = [x["source"]["path"] for x in core(ctl, "reply/sources")["local_folders"]]
    need(said and said[0]["words"] == TYPED_FOLDER and TYPED_FOLDER in paths and len(paths) == n + 1,
         "typed folder: the buffer said %r, the folders are %r (%d before)" % (said, paths, n))
    return "typed folder: %s typed through `type`, committed, now a source (%d -> %d folders)" % (TYPED_FOLDER, n, len(paths))


case("sources-add-folder-typed", [], "typed.sources-add-folder", _typed_folder)


def _remove(ctl):
    hit(ctl, "sources.add-folder.sample")
    doors = sorted(family(ctl, "sources.local.", ".remove"))
    need(doors, "no local folder to remove")
    n = len(core(ctl, "reply/sources")["local_folders"])
    hit(ctl, doors[0])
    after = len(core(ctl, "reply/sources")["local_folders"])
    need(after == n - 1, "%s: %d local folders before, %d after" % (doors[0], n, after))
    return "%s: %d -> %d local folders" % (doors[0], n, after)


case("sources-local-remove", ["sources.local.*.remove"], "sources.local.0.remove", _remove)


def _cards(ctl):
    done = []
    for d in [d for d in family(ctl, "sources.account.") if ".folder." not in d]:
        o0 = tile(ctl, d)["open"]
        hit(ctl, d)
        need(tile(ctl, d)["open"] == (not o0), "%s: the card did not open or close (was %r)" % (d, o0))
        done.append(d.split(".")[-1])
    need(done, "no account card listed")
    return "account cards %s each turned over" % done


def _tiles(match):
    def fn(ctl):
        done = []
        for d in [d for d in controls(ctl) if match(d)]:
            e0, n0 = tile(ctl, d)["enabled"], core(ctl, "reply/sources")["enabled_total"]
            hit(ctl, d)
            n1 = core(ctl, "reply/sources")["enabled_total"]
            need(tile(ctl, d)["enabled"] == (not e0) and n1 == n0 + (-1 if e0 else 1), "%s: tile %r -> %r, the core's count %d -> %d" % (d, e0, tile(ctl, d)["enabled"], n0, n1))
            done.append(d[len("sources."):])
        need(done, "no such tile listed")
        return "%d tiles each turned over, the core's count followed: %s" % (len(done), done)
    return fn


case("sources-account-cards", ["sources.account.*"], "sources.account.acc-1", _cards)
case("sources-folder-tiles", ["sources.account.*.folder.*"], "sources.account.acc-1.folder.0", _tiles(lambda d: ".folder." in d and d.startswith("sources.account.")))
case("sources-local-tiles", ["sources.local.*"], "sources.local.0", _tiles(lambda d: d.startswith("sources.local.") and not d.endswith(".remove")))
case("sources-calendar-switches", ["sources.calendar.*"], "sources.calendar.cal-work", _tiles(lambda d: d.startswith("sources.calendar.")))
for _sc, _o in [("this_account", "all_accounts"), ("all_accounts", "this_account")]:
    choose("sources-scope-" + _sc.replace("_", "-"), "sources.scope." + _sc, "sources.scope." + _o, lambda c: (core(c, "reply/retrieval")["scope"], state(c)["search_scope"]), _sc)

# ---------------------------------------------------------------- Retrieval
for _n, _o in [(3, 5), (5, 3), (8, 3), (10, 3)]:
    choose("retrieval-per-question-%d" % _n, "retrieval.per-question.%d" % _n, "retrieval.per-question.%d" % _o,
           lambda c: (core(c, "reply/retrieval")["excerpts_per_question"], state(c)["retrieval"]["per_question"]), _n)
for _z, _o in [("sentence", "paragraph"), ("paragraph", "sentence"), ("section", "sentence")]:
    choose("retrieval-size-" + _z, "retrieval.size." + _z, "retrieval.size." + _o, lambda c: (core(c, "reply/retrieval")["excerpt_size"], state(c)["retrieval"]["size"]), _z)
for _n, _o in [(3, 6), (6, 3), (10, 3), (20, 3)]:
    choose("retrieval-keep-%d" % _n, "retrieval.keep.%d" % _n, "retrieval.keep.%d" % _o, lambda c: (core(c, "reply/retrieval")["keep_n"], state(c)["retrieval"]["keep"]), _n)
for _r, _o in [("keep_both_dated", "keep_newest"), ("keep_newest", "keep_oldest"), ("keep_oldest", "keep_newest")]:
    choose("retrieval-clash-" + _r.replace("_", "-"), "retrieval.clash." + _r, "retrieval.clash." + _o, lambda c: (core(c, "reply/retrieval")["clash_rule"], state(c)["retrieval"]["clash"]), _r)
opens("retrieval-fields-row", "retrieval.fields", popover="fields")
opens("retrieval-ai-filter-row", "retrieval.ai-filter", popover="aiFilter")


def _field(f):
    def fn(ctl):
        hit(ctl, "retrieval.fields")
        need(state(ctl)["ui"]["popover"] == "fields", "setup: the fields popover is %r" % state(ctl)["ui"]["popover"])
        c0 = core(ctl, "reply/retrieval")["attached_fields"][f]
        hit(ctl, "retrieval.fields." + f)
        c1 = core(ctl, "reply/retrieval")["attached_fields"][f]
        words = f in sr._fields_line(ctl)
        need(c1 == (not c0) and words == c1, "retrieval.fields.%s: core %r -> %r, the row's words name it: %r" % (f, c0, c1, words))
        return "retrieval.fields.%s: core %r -> %r, the row's words follow" % (f, c0, c1)
    return fn


for _f in ["date", "sender", "folder", "subject"]:
    case("retrieval-field-" + _f, ["retrieval.fields." + _f], "retrieval.fields." + _f, _field(_f))


def _ai_filter(door, setup, want_q):
    def fn(ctl):
        if setup:
            hit(ctl, setup)
        hit(ctl, door)
        c, s = core(ctl, "reply/retrieval")["ai_filter_question"], state(ctl)["retrieval"]["ai_filter"]
        need(c == want_q and s == want_q, "%s: the core holds %r, the pane draws %r, expected %r" % (door, c, s, want_q))
        return "%s: question %r (core and pane)" % (door, c)
    case("retrieval-" + door.split("retrieval.")[1].replace(".", "-"), [door], door, fn)


_ai_filter("retrieval.ai-filter.field", "retrieval.ai-filter.off", sr.FILTER_Q)
_ai_filter("retrieval.ai-filter.save", "retrieval.ai-filter.off", sr.FILTER_Q)
_ai_filter("retrieval.ai-filter.off", "retrieval.ai-filter.field", None)

# ---------------------------------------------------------------- Voice
for _n, _o in [(50, 100), (100, 50), (200, 50), (500, 50)]:
    def _sample(ctl, n=_n, o=_o):
        hit(ctl, "voice.sample.%d" % o)
        hit(ctl, "voice.sample.%d" % n)
        got = state(ctl)["voice"]["picked_sample"]
        need(got == n, "voice.sample.%d: the pane holds %r" % (n, got))
        return "voice.sample.%d: picked sample %r" % (n, got)
    case("voice-sample-%d" % _n, ["voice.sample.%d" % _n], "voice.sample.%d" % _n, _sample)


def _analyse(ctl):
    hit(ctl, "voice.sample.200")
    at = core(ctl, "reply/voice")["voice"]["analysed_at_ms"]
    hit(ctl, "voice.analyse")
    v = core(ctl, "reply/voice")["voice"]
    need(v["sample_size"] == 200 and v["analysed_at_ms"] != at, "voice.analyse: sample %r, analysed at %r (was %r)" % (v["sample_size"], v["analysed_at_ms"], at))
    return "voice.analyse: sample %r, analysed at %r (was %r)" % (v["sample_size"], v["analysed_at_ms"], at)


case("voice-analyse", ["voice.analyse"], "voice.analyse", _analyse)
for _n, _o in [(0, 3), (1, 3), (3, 5), (5, 3)]:
    choose("voice-examples-%d" % _n, "voice.examples.%d" % _n, "voice.examples.%d" % _o,
           lambda c: (core(c, "reply/voice")["voice"]["same_person_examples"], state(c)["voice"]["examples"]), _n)
opens("voice-tones-row", "voice.tones", popover="tones")


def _tone_add(ctl):
    hit(ctl, "voice.tones")
    n = len(core(ctl, "reply/voice")["tones"])
    hit(ctl, "voice.tone.add")
    names = [t["name"] for t in core(ctl, "reply/voice")["tones"]]
    need(len(names) == n + 1 and "Chaleureux" in names and "Chaleureux" in state(ctl)["voice"]["tones"], "voice.tone.add: the core holds %r (%d before), the pane %r" % (names, n, state(ctl)["voice"]["tones"]))
    return "voice.tone.add: tones %r" % names


def _tone_delete(ctl):
    hit(ctl, "voice.tones")
    done = []
    for d in family(ctl, "voice.tone.", ".delete"):
        tid = d[len("voice.tone."):-len(".delete")]
        hit(ctl, d)
        need(tid not in [t["id"] for t in core(ctl, "reply/voice")["tones"]], "%s removed nothing" % d)
        done.append(tid)
    need(done, "no tone delete listed")
    return "tones %s each removed" % done


case("voice-tone-add", ["voice.tone.add"], "voice.tone.add", _tone_add)
case("voice-tone-delete", ["voice.tone.*.delete"], "voice.tone.tone-formal.delete", _tone_delete)

# ---------------------------------------------------------------- The prompt
opens("prompt-variables-row", "prompt.variables", popover="variables")
opens("prompt-notes-row", "prompt.notes", popover="notes")
opens("prompt-versions-row", "prompt.versions", popover="versions")


def _preview(ctl):
    hit(ctl, "prompt.preview")
    pv = state(ctl)["prompt"]["preview"]
    need(state(ctl)["ui"]["popover"] == "preview" and pv and "{{" not in pv and "//" not in pv, "prompt.preview: popover %r, text %r" % (state(ctl)["ui"]["popover"], (pv or "")[:60]))
    return "prompt.preview: popover preview, %d characters of text" % len(pv)


case("prompt-preview", ["prompt.preview"], "prompt.preview", _preview)


def _note_add(ctl):
    n = len(core(ctl, "reply/notes"))
    hit(ctl, "prompt.note.add")
    n_core, n_pane = len(core(ctl, "reply/notes")), len(state(ctl)["prompt"]["notes"])
    need(n_core == n + 1 and n_pane == n + 1, "prompt.note.add: the core holds %d notes and the pane %d, expected %d" % (n_core, n_pane, n + 1))
    return "prompt.note.add: %d -> %d notes (core and pane)" % (n, n_core)


def _note_delete(ctl):
    done = []
    for d in family(ctl, "prompt.note.", ".delete"):
        n = len(core(ctl, "reply/notes"))
        hit(ctl, d)
        need(len(core(ctl, "reply/notes")) == n - 1, "%s removed nothing" % d)
        done.append(d.split(".")[2])
    need(done, "no note delete listed")
    return "notes %s each removed" % done


def _restore(ctl):
    done = []
    for d in family(ctl, "prompt.version.", ".restore"):
        v = int(d.split(".")[2])
        old = [x for x in core(ctl, "reply/prompt/versions") if x["version"] == v][0]["text"]
        top = max(x["version"] for x in core(ctl, "reply/prompt/versions"))
        hit(ctl, d)
        cur = core(ctl, "reply/prompt/current")
        need(cur["version"] == top + 1 and cur["text"] == old and state(ctl)["prompt"]["version"] == top + 1, "%s: the current version is %r, expected %d" % (d, cur["version"], top + 1))
        done.append(v)
    need(done, "no version to restore listed")
    return "versions %s each restored as a new current version" % done


case("prompt-note-add", ["prompt.note.add"], "prompt.note.add", _note_add)
case("prompt-note-delete", ["prompt.note.*.delete"], "prompt.note.note-1.delete", _note_delete)
case("prompt-version-restore", ["prompt.version.*.restore"], "prompt.version.1.restore", _restore)


def _variable(v):
    def fn(ctl):
        sr.open_editor(ctl, "prompt")
        n = sr.pills(sr.editors(ctl)["prompt"]["text"])
        hit(ctl, "prompt.variables")
        hit(ctl, "prompt.variable." + v)
        after, pop = sr.pills(sr.editors(ctl)["prompt"]["text"]), state(ctl)["ui"]["popover"]
        need(after == n + 1 and pop is None, "prompt.variable.%s: %d pills before, %d after, popover %r" % (v, n, after, pop))
        return "prompt.variable.%s: pills %d -> %d, popover closed" % (v, n, after)
    return fn


for _v in sr.VARS:
    case("prompt-variable-" + _v, ["prompt.variable." + _v], "prompt.variable." + _v, _variable(_v))


# ---------------------------------------------------------------- the long texts: the editor of each, its Save and its Revert
def _editor_click(k):
    def fn(ctl):
        sr.goto_editor(ctl, k)
        hit(ctl, "editor." + k)
        e = sr.editors(ctl)[k]
        need(e.get("focused") is True and isinstance(e.get("cursor"), int), "editor.%s: after one click focused %r, cursor %r" % (k, e.get("focused"), e.get("cursor")))
        return "editor.%s: focused, cursor %r" % (k, e["cursor"])
    return fn


def _editor_save(k):
    def fn(ctl):
        sr.open_editor(ctl, k)
        sr.keys(ctl, "w")
        need(sr.editors(ctl)[k]["dirty"], "setup: typing left editor.%s clean" % k)
        n = len(core(ctl, "reply/notes"))
        hit(ctl, "editor.%s.save" % k)
        e = sr.editors(ctl)[k]
        if k == "note.new":
            need(len(core(ctl, "reply/notes")) == n + 1, "editor.%s.save: the notes are %d, expected %d" % (k, len(core(ctl, "reply/notes")), n + 1))
            return "editor.%s.save: notes %d -> %d" % (k, n, n + 1)
        need(not e["dirty"], "editor.%s.save: still dirty" % k)
        return "editor.%s.save: saved, no longer dirty" % k
    return fn


def _editor_revert(k):
    def fn(ctl):
        sr.open_editor(ctl, k)
        sr.keys(ctl, "w")
        need(sr.editors(ctl)[k]["dirty"], "setup: typing left editor.%s clean" % k)
        hit(ctl, "editor.%s.revert" % k)
        ask = sr.editors(ctl)[k]
        need(ask["confirm"] and ask["dirty"], "editor.%s.revert: the first click did not ask (confirm %r, dirty %r)" % (k, ask["confirm"], ask["dirty"]))
        hit(ctl, "editor.%s.revert" % k)
        need(not sr.editors(ctl)[k]["dirty"], "editor.%s.revert: the second click left the change" % k)
        return "editor.%s.revert: asked, then threw the change away" % k
    return fn


def _each_note(make):
    def fn(ctl):
        keys = [k for k in sr.editors(ctl) if k.startswith("note.") and k != "note.new"]
        need(keys, "no saved note has an editor")
        return "; ".join(make(k)(ctl) for k in keys)
    return fn


for _k, _label in [("prompt", "prompt"), ("retrievalPrompt", "retrieval-prompt"), ("voiceText", "voice-text"), ("aiDescription", "ai-description"), ("note.new", "new-note")]:
    case("editor-%s-click" % _label, ["editor." + _k], "editor." + _k, _editor_click(_k))
    case("editor-%s-save" % _label, ["editor.%s.save" % _k], "editor.%s.save" % _k, _editor_save(_k))
    case("editor-%s-revert" % _label, ["editor.%s.revert" % _k], "editor.%s.revert" % _k, _editor_revert(_k))
case("editor-notes-click", ["editor.note.*"], "editor.note.note-1", _each_note(_editor_click))
case("editor-notes-save", ["editor.note.*.save"], "editor.note.note-1.save", _each_note(_editor_save))
case("editor-notes-revert", ["editor.note.*.revert"], "editor.note.note-1.revert", _each_note(_editor_revert))


def _listed(ctl):
    pats = [p for _n, ds, _m, _f in CASES for p in ds] + ["sources.picker.cancel"]
    listed = controls(ctl)
    extra = [d for d in listed if not any(fnmatch.fnmatchcase(d, p) for p in pats)]
    need(len(listed) > 100 and not extra, "the app lists %d controls; no case drives: %s" % (len(listed), extra))
    return "the app lists %d controls and every one is driven by a case above" % len(listed)


case("every-listed-control-has-a-case", [], "extra-control", _listed)


# ---------------------------------------------------------------- the run
def run_case(s, name, fn):
    def run():
        ctl = s.ctl
        sr.reset(ctl)
        try:
            return True, fn(ctl)
        except sr.Red as e:
            return False, str(e)
    s._record(name, run)


def drive(j, only=None):
    s = j.step(ID, TITLE)
    for name, _doors, _noop, fn in CASES:
        if only is None or name == only:
            run_case(s, name, fn)


# the editor doors are built from a runtime key (RepliesEditorDoors.add): the lines the generator cannot match by their text
SRC = {}
for _n, _s in [("click", '"editor.\\(k.raw)", "text"'), ("save", '"editor.\\(k.raw).save"'), ("revert", '"editor.\\(k.raw).revert"')]:
    for _w in ["prompt", "retrieval-prompt", "voice-text", "ai-description", "new-note", "notes"]:
        SRC["editor-%s-%s" % (_w, _n)] = "RepliesEditorDoors.swift:" + _s
SRC["every-listed-control-has-a-case"] = "RepliesDoors.swift:uses(s, add)"
CHANGE = {n: "the editor door does nothing: its action closure becomes `{ }`" for n in SRC}
CHANGE["every-listed-control-has-a-case"] = "one more control is listed that no case drives: a line `add(.sources, \"sources.new-thing\", \"button\") { }` is added after this one"


# MUTANTS: per case, the one-line change of the app that must turn it red; the selftest makes the same change in its model (a dead door).
# Format: # <step>/<case> | <file>:<line>: the change
# par-settingsreplies-sweep/page-auto | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:25: the control `page.auto` does nothing: its action closure becomes `{ }` (line: for p in RepliesPage.allCases { add(p, "page.\(p.rawValue)", "page") { RepliesRouter.go(p) } })
# par-settingsreplies-sweep/page-filter | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:25: the control `page.filter` does nothing: its action closure becomes `{ }` (line: for p in RepliesPage.allCases { add(p, "page.\(p.rawValue)", "page") { RepliesRouter.go(p) } })
# par-settingsreplies-sweep/page-on-demand | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:25: the control `page.on-demand` does nothing: its action closure becomes `{ }` (line: for p in RepliesPage.allCases { add(p, "page.\(p.rawValue)", "page") { RepliesRouter.go(p) } })
# par-settingsreplies-sweep/page-sources | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:25: the control `page.sources` does nothing: its action closure becomes `{ }` (line: for p in RepliesPage.allCases { add(p, "page.\(p.rawValue)", "page") { RepliesRouter.go(p) } })
# par-settingsreplies-sweep/page-retrieval | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:25: the control `page.retrieval` does nothing: its action closure becomes `{ }` (line: for p in RepliesPage.allCases { add(p, "page.\(p.rawValue)", "page") { RepliesRouter.go(p) } })
# par-settingsreplies-sweep/page-voice | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:25: the control `page.voice` does nothing: its action closure becomes `{ }` (line: for p in RepliesPage.allCases { add(p, "page.\(p.rawValue)", "page") { RepliesRouter.go(p) } })
# par-settingsreplies-sweep/page-prompt | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:25: the control `page.prompt` does nothing: its action closure becomes `{ }` (line: for p in RepliesPage.allCases { add(p, "page.\(p.rawValue)", "page") { RepliesRouter.go(p) } })
# par-settingsreplies-sweep/popover-escape-closes-it | Sources/Features/SettingsRepliesMac/SettingsRepliesMacFeature.swift:55: Escape no longer closes an open popover: `s.popover != nil` becomes `s.popover == nil` (line: guard m.surface.hasPrefix("mac-settings"), s.popover != nil else { return false })
# par-settingsreplies-sweep/sources-add-command-n | Sources/Features/SettingsRepliesMac/SourcesPane.swift:17: Add a source loses its key: `.keyboardShortcut("n", modifiers: .command)` becomes `.keyboardShortcut("m", modifiers: .command)`
# par-settingsreplies-sweep/popover-close | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:26: the control `popover.close` does nothing: its action closure becomes `{ }` (line: add(.auto, "popover.close", "button") { s.popover = nil })
# par-settingsreplies-sweep/auto-enabled | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:34: the control `auto.enabled` does nothing: its action closure becomes `{ }` (line: add(.auto, "auto.enabled", "switch") { s.setEnabled(.auto, !(s.use(.auto)?.enabled ?? false)) })
# par-settingsreplies-sweep/auto-match-thread | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:40: the control `auto.match-thread` does nothing: its action closure becomes `{ }` (line: add(page, "\(key).match-thread", "switch") { s.setMatchThread(kind, !(s.use(kind)?.matchThread ?? false)) })
# par-settingsreplies-sweep/on-demand-match-thread | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:40: the control `on-demand.match-thread` does nothing: its action closure becomes `{ }` (line: add(page, "\(key).match-thread", "switch") { s.setMatchThread(kind, !(s.use(kind)?.matchThread ?? false)) })
# par-settingsreplies-sweep/on-demand-instruction-before | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:43: the control `on-demand.instruction-before` does nothing: its action closure becomes `{ }` (line: add(.onDemand, "on-demand.instruction-before", "switch") { s.setOption(.instructionBefore, !s.onDemand.inst...)
# par-settingsreplies-sweep/on-demand-change-after | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:44: the control `on-demand.change-after` does nothing: its action closure becomes `{ }` (line: add(.onDemand, "on-demand.change-after", "switch") { s.setOption(.changeAfter, !s.onDemand.changeAfter) })
# par-settingsreplies-sweep/auto-step-entry | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:36: the control `auto.step.entry` does nothing: its action closure becomes `{ }` (line: for k in ["entry", "ai", "exit"] { add(.auto, "auto.step.\(k)", "row") { RepliesRouter.go(.filter) } })
# par-settingsreplies-sweep/auto-step-ai | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:36: the control `auto.step.ai` does nothing: its action closure becomes `{ }` (line: for k in ["entry", "ai", "exit"] { add(.auto, "auto.step.\(k)", "row") { RepliesRouter.go(.filter) } })
# par-settingsreplies-sweep/auto-step-exit | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:36: the control `auto.step.exit` does nothing: its action closure becomes `{ }` (line: for k in ["entry", "ai", "exit"] { add(.auto, "auto.step.\(k)", "row") { RepliesRouter.go(.filter) } })
# par-settingsreplies-sweep/auto-sources-row | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:38: the control `auto.sources` does nothing: its action closure becomes `{ }` (line: add(page, "\(key).sources", "row") { RepliesRouter.go(.sources) })
# par-settingsreplies-sweep/auto-prompt-row | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:39: the control `auto.prompt` does nothing: its action closure becomes `{ }` (line: add(page, "\(key).prompt", "row") { RepliesRouter.go(.prompt) })
# par-settingsreplies-sweep/on-demand-sources-row | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:38: the control `on-demand.sources` does nothing: its action closure becomes `{ }` (line: add(page, "\(key).sources", "row") { RepliesRouter.go(.sources) })
# par-settingsreplies-sweep/on-demand-prompt-row | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:39: the control `on-demand.prompt` does nothing: its action closure becomes `{ }` (line: add(page, "\(key).prompt", "row") { RepliesRouter.go(.prompt) })
# par-settingsreplies-sweep/auto-try | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:35: the control `auto.try` does nothing: its action closure becomes `{ }` (line: add(.auto, "auto.try", "button") { s.tryOnMessage(.auto) })
# par-settingsreplies-sweep/retrieval-try | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:34: the control `retrieval.try` does nothing: its action closure becomes `{ }` (line: add(.retrieval, "retrieval.try", "button") { s.tryOnMessage(.onDemand) })
# par-settingsreplies-sweep/auto-tone-choices | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:41: the control `auto.tone.tone-formal` does nothing: its action closure becomes `{ }` (line: for id in ["voice"] + s.tones.map(\.id) { add(page, "\(key).tone.\(id)", "choice") { s.setTone(kind, id) } })
# par-settingsreplies-sweep/on-demand-tone-choices | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:41: the control `on-demand.tone.tone-formal` does nothing: its action closure becomes `{ }` (line: for id in ["voice"] + s.tones.map(\.id) { add(page, "\(key).tone.\(id)", "choice") { s.setTone(kind, id) } })
# par-settingsreplies-sweep/filter-entry-switch | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:53: the control `filter.entry.switch` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).switch", "switch") {)
# par-settingsreplies-sweep/filter-ai-switch | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:53: the control `filter.ai.switch` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).switch", "switch") {)
# par-settingsreplies-sweep/filter-exit-switch | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:53: the control `filter.exit.switch` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).switch", "switch") {)
# par-settingsreplies-sweep/filter-scope-all | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:59: the control `filter.scope.all` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.scope.all", "check") { s.setScope(.all) })
# par-settingsreplies-sweep/filter-scope-labels | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:60: the control `filter.scope.label.needs-reply` does nothing: its action closure becomes `{ }` (line: for l in s.labels { add(.filter, "filter.scope.label.\(l.id)", "check") { s.toggleLabel(l.id) } })
# par-settingsreplies-sweep/filter-entry-match-all | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:62: the control `filter.entry.match.all` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).match.all", "choice") { s.setMode(card, .all) })
# par-settingsreplies-sweep/filter-entry-match-any | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:63: the control `filter.entry.match.any` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).match.any", "choice") { s.setMode(card, .any) })
# par-settingsreplies-sweep/filter-entry-fact-unread-any | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.unread.any` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-unread-yes | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.unread.yes` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-unread-no | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.unread.no` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-unanswered-any | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.unanswered.any` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-unanswered-yes | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.unanswered.yes` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-unanswered-no | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.unanswered.no` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-has-attachment-any | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.has_attachment.any` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-has-attachment-yes | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.has_attachment.yes` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-has-attachment-no | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.has_attachment.no` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-starred-any | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.starred.any` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-starred-yes | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.starred.yes` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-starred-no | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.starred.no` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-vip-sender-any | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.vip_sender.any` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-vip-sender-yes | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.vip_sender.yes` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-vip-sender-no | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.vip_sender.no` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-cc-me-any | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.cc_me.any` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-cc-me-yes | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.cc_me.yes` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-cc-me-no | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.cc_me.no` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-mailing-list-any | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.mailing_list.any` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-mailing-list-yes | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.mailing_list.yes` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-mailing-list-no | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.mailing_list.no` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-replied-by-me-any | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.replied_by_me.any` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-replied-by-me-yes | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.replied_by_me.yes` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-fact-replied-by-me-no | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.entry.fact.replied_by_me.no` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-match-all | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:62: the control `filter.exit.match.all` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).match.all", "choice") { s.setMode(card, .all) })
# par-settingsreplies-sweep/filter-exit-match-any | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:63: the control `filter.exit.match.any` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).match.any", "choice") { s.setMode(card, .any) })
# par-settingsreplies-sweep/filter-exit-fact-unread-any | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.unread.any` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-unread-yes | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.unread.yes` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-unread-no | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.unread.no` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-unanswered-any | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.unanswered.any` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-unanswered-yes | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.unanswered.yes` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-unanswered-no | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.unanswered.no` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-has-attachment-any | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.has_attachment.any` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-has-attachment-yes | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.has_attachment.yes` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-has-attachment-no | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.has_attachment.no` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-starred-any | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.starred.any` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-starred-yes | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.starred.yes` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-starred-no | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.starred.no` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-vip-sender-any | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.vip_sender.any` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-vip-sender-yes | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.vip_sender.yes` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-vip-sender-no | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.vip_sender.no` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-cc-me-any | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.cc_me.any` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-cc-me-yes | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.cc_me.yes` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-cc-me-no | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.cc_me.no` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-mailing-list-any | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.mailing_list.any` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-mailing-list-yes | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.mailing_list.yes` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-mailing-list-no | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.mailing_list.no` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-replied-by-me-any | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.replied_by_me.any` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-replied-by-me-yes | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.replied_by_me.yes` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-exit-fact-replied-by-me-no | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:67: the control `filter.exit.fact.replied_by_me.no` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) })
# par-settingsreplies-sweep/filter-entry-sender | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:71: the control `filter.entry.sender` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.entry.sender", "field") { s.setSender(RepliesDoors.sampleSender) })
# par-settingsreplies-sweep/filter-entry-sender-clear | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:72: the control `filter.entry.sender.clear` does nothing: its action closure becomes `{ }` (line: add(.filter, "filter.entry.sender.clear", "button") { s.setSender("") })
# par-settingsreplies-sweep/filter-ai-reads-subject | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:73: the control `filter.ai.reads.subject` does nothing: its action closure becomes `{ }` (line: for r in RepliesReadInput.allCases { add(.filter, "filter.ai.reads.\(r.rawValue)", "check") { s.toggleRead(...)
# par-settingsreplies-sweep/filter-ai-reads-body | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:73: the control `filter.ai.reads.body` does nothing: its action closure becomes `{ }` (line: for r in RepliesReadInput.allCases { add(.filter, "filter.ai.reads.\(r.rawValue)", "check") { s.toggleRead(...)
# par-settingsreplies-sweep/filter-ai-reads-rest | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:73: the control `filter.ai.reads.rest` does nothing: its action closure becomes `{ }` (line: for r in RepliesReadInput.allCases { add(.filter, "filter.ai.reads.\(r.rawValue)", "check") { s.toggleRead(...)
# par-settingsreplies-sweep/filter-ai-reads-attachments | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:73: the control `filter.ai.reads.attachments` does nothing: its action closure becomes `{ }` (line: for r in RepliesReadInput.allCases { add(.filter, "filter.ai.reads.\(r.rawValue)", "check") { s.toggleRead(...)
# par-settingsreplies-sweep/sources-add | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:17: the control `sources.add` does nothing: its action closure becomes `{ }` (line: add(.sources, "sources.add", "button") { RepliesFolderPicker.choose() })
# par-settingsreplies-sweep/sources-add-folder | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:18: the control `sources.add-folder` does nothing: its action closure becomes `{ }` (line: add(.sources, "sources.add-folder", "button") { RepliesFolderPicker.choose() })
# par-settingsreplies-sweep/sources-add-folder-typed | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:typedClickTargets: the `sources.add-folder.typed.commit` door does nothing: its action closure becomes `{ }`
# par-settingsreplies-sweep/sources-add-folder-sample | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:19: the control `sources.add-folder.sample` does nothing: its action closure becomes `{ }` (line: add(.sources, "sources.add-folder.sample", "field") { s.addFolder(RepliesDoors.sampleFolder) })
# par-settingsreplies-sweep/sources-local-remove | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:26: the control `sources.local.0.remove` does nothing: its action closure becomes `{ }` (line: if case .localFolder(let path) = f.source { add(.sources, "sources.local.\(i).remove", "menu") { s.removeFo...)
# par-settingsreplies-sweep/sources-account-cards | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:21: the control `sources.account.acc-1` does nothing: its action closure becomes `{ }` (line: add(.sources, "sources.account.\(a.accountId)", "row") { s.toggleAccount(a.accountId) })
# par-settingsreplies-sweep/sources-folder-tiles | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:22: the control `sources.account.acc-1.folder.0` does nothing: its action closure becomes `{ }` (line: for (i, f) in a.folders.enumerated() { add(.sources, "sources.account.\(a.accountId).folder.\(i)", "tile") ...)
# par-settingsreplies-sweep/sources-local-tiles | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:25: the control `sources.local.0` does nothing: its action closure becomes `{ }` (line: add(.sources, "sources.local.\(i)", "tile") { s.toggleSource(f) })
# par-settingsreplies-sweep/sources-calendar-switches | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:28: the control `sources.calendar.cal-work` does nothing: its action closure becomes `{ }` (line: for c in s.sources.calendars { add(.sources, "sources.calendar.\(c.calendarId)", "switch") { s.toggleCalend...)
# par-settingsreplies-sweep/sources-scope-this-account | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:29: the control `sources.scope.this_account` does nothing: its action closure becomes `{ }` (line: add(.sources, "sources.scope.this_account", "segment") { s.setSearchScope(.thisAccount) })
# par-settingsreplies-sweep/sources-scope-all-accounts | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:30: the control `sources.scope.all_accounts` does nothing: its action closure becomes `{ }` (line: add(.sources, "sources.scope.all_accounts", "segment") { s.setSearchScope(.allAccounts) })
# par-settingsreplies-sweep/retrieval-per-question-3 | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:35: the control `retrieval.per-question.3` does nothing: its action closure becomes `{ }` (line: for n in RepliesChoices.perQuestion { add(.retrieval, "retrieval.per-question.\(n)", "choice") { s.setRetri...)
# par-settingsreplies-sweep/retrieval-per-question-5 | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:35: the control `retrieval.per-question.5` does nothing: its action closure becomes `{ }` (line: for n in RepliesChoices.perQuestion { add(.retrieval, "retrieval.per-question.\(n)", "choice") { s.setRetri...)
# par-settingsreplies-sweep/retrieval-per-question-8 | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:35: the control `retrieval.per-question.8` does nothing: its action closure becomes `{ }` (line: for n in RepliesChoices.perQuestion { add(.retrieval, "retrieval.per-question.\(n)", "choice") { s.setRetri...)
# par-settingsreplies-sweep/retrieval-per-question-10 | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:35: the control `retrieval.per-question.10` does nothing: its action closure becomes `{ }` (line: for n in RepliesChoices.perQuestion { add(.retrieval, "retrieval.per-question.\(n)", "choice") { s.setRetri...)
# par-settingsreplies-sweep/retrieval-size-sentence | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:37: the control `retrieval.size.sentence` does nothing: its action closure becomes `{ }` (line: add(.retrieval, "retrieval.size.\(excerptSizeKey(v: z))", "choice") { s.setRetrieval(.excerptSize(size: z)) })
# par-settingsreplies-sweep/retrieval-size-paragraph | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:37: the control `retrieval.size.paragraph` does nothing: its action closure becomes `{ }` (line: add(.retrieval, "retrieval.size.\(excerptSizeKey(v: z))", "choice") { s.setRetrieval(.excerptSize(size: z)) })
# par-settingsreplies-sweep/retrieval-size-section | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:37: the control `retrieval.size.section` does nothing: its action closure becomes `{ }` (line: add(.retrieval, "retrieval.size.\(excerptSizeKey(v: z))", "choice") { s.setRetrieval(.excerptSize(size: z)) })
# par-settingsreplies-sweep/retrieval-keep-3 | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:41: the control `retrieval.keep.3` does nothing: its action closure becomes `{ }` (line: for n in RepliesChoices.keep { add(.retrieval, "retrieval.keep.\(n)", "choice") { s.setRetrieval(.keepN(n: ...)
# par-settingsreplies-sweep/retrieval-keep-6 | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:41: the control `retrieval.keep.6` does nothing: its action closure becomes `{ }` (line: for n in RepliesChoices.keep { add(.retrieval, "retrieval.keep.\(n)", "choice") { s.setRetrieval(.keepN(n: ...)
# par-settingsreplies-sweep/retrieval-keep-10 | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:41: the control `retrieval.keep.10` does nothing: its action closure becomes `{ }` (line: for n in RepliesChoices.keep { add(.retrieval, "retrieval.keep.\(n)", "choice") { s.setRetrieval(.keepN(n: ...)
# par-settingsreplies-sweep/retrieval-keep-20 | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:41: the control `retrieval.keep.20` does nothing: its action closure becomes `{ }` (line: for n in RepliesChoices.keep { add(.retrieval, "retrieval.keep.\(n)", "choice") { s.setRetrieval(.keepN(n: ...)
# par-settingsreplies-sweep/retrieval-clash-keep-both-dated | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:43: the control `retrieval.clash.keep_both_dated` does nothing: its action closure becomes `{ }` (line: add(.retrieval, "retrieval.clash.\(clashRuleKey(v: r))", "choice") { s.setRetrieval(.clash(rule: r)) })
# par-settingsreplies-sweep/retrieval-clash-keep-newest | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:43: the control `retrieval.clash.keep_newest` does nothing: its action closure becomes `{ }` (line: add(.retrieval, "retrieval.clash.\(clashRuleKey(v: r))", "choice") { s.setRetrieval(.clash(rule: r)) })
# par-settingsreplies-sweep/retrieval-clash-keep-oldest | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:43: the control `retrieval.clash.keep_oldest` does nothing: its action closure becomes `{ }` (line: add(.retrieval, "retrieval.clash.\(clashRuleKey(v: r))", "choice") { s.setRetrieval(.clash(rule: r)) })
# par-settingsreplies-sweep/retrieval-fields-row | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:39: the control `retrieval.fields` does nothing: its action closure becomes `{ }` (line: add(.retrieval, "retrieval.fields", "row") { s.popover = .fields })
# par-settingsreplies-sweep/retrieval-ai-filter-row | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:45: the control `retrieval.ai-filter` does nothing: its action closure becomes `{ }` (line: add(.retrieval, "retrieval.ai-filter", "row") { s.popover = .aiFilter })
# par-settingsreplies-sweep/retrieval-field-date | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:40: the control `retrieval.fields.date` does nothing: its action closure becomes `{ }` (line: for f in RepliesField.allCases { add(.retrieval, "retrieval.fields.\(f.rawValue)", "check") { s.toggleField...)
# par-settingsreplies-sweep/retrieval-field-sender | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:40: the control `retrieval.fields.sender` does nothing: its action closure becomes `{ }` (line: for f in RepliesField.allCases { add(.retrieval, "retrieval.fields.\(f.rawValue)", "check") { s.toggleField...)
# par-settingsreplies-sweep/retrieval-field-folder | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:40: the control `retrieval.fields.folder` does nothing: its action closure becomes `{ }` (line: for f in RepliesField.allCases { add(.retrieval, "retrieval.fields.\(f.rawValue)", "check") { s.toggleField...)
# par-settingsreplies-sweep/retrieval-field-subject | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:40: the control `retrieval.fields.subject` does nothing: its action closure becomes `{ }` (line: for f in RepliesField.allCases { add(.retrieval, "retrieval.fields.\(f.rawValue)", "check") { s.toggleField...)
# par-settingsreplies-sweep/retrieval-ai-filter-field | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:46: the control `retrieval.ai-filter.field` does nothing: its action closure becomes `{ }` (line: add(.retrieval, "retrieval.ai-filter.field", "field") { s.setRetrieval(.aiFilterQuestion(question: RepliesD...)
# par-settingsreplies-sweep/retrieval-ai-filter-save | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:47: the control `retrieval.ai-filter.save` does nothing: its action closure becomes `{ }` (line: add(.retrieval, "retrieval.ai-filter.save", "button") { s.setRetrieval(.aiFilterQuestion(question: RepliesD...)
# par-settingsreplies-sweep/retrieval-ai-filter-off | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:48: the control `retrieval.ai-filter.off` does nothing: its action closure becomes `{ }` (line: add(.retrieval, "retrieval.ai-filter.off", "button") { s.setRetrieval(.aiFilterQuestion(question: nil)) })
# par-settingsreplies-sweep/voice-sample-50 | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:52: the control `voice.sample.50` does nothing: its action closure becomes `{ }` (line: for n in RepliesChoices.sample { add(.voice, "voice.sample.\(n)", "choice") { s.voiceSample = n } })
# par-settingsreplies-sweep/voice-sample-100 | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:52: the control `voice.sample.100` does nothing: its action closure becomes `{ }` (line: for n in RepliesChoices.sample { add(.voice, "voice.sample.\(n)", "choice") { s.voiceSample = n } })
# par-settingsreplies-sweep/voice-sample-200 | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:52: the control `voice.sample.200` does nothing: its action closure becomes `{ }` (line: for n in RepliesChoices.sample { add(.voice, "voice.sample.\(n)", "choice") { s.voiceSample = n } })
# par-settingsreplies-sweep/voice-sample-500 | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:52: the control `voice.sample.500` does nothing: its action closure becomes `{ }` (line: for n in RepliesChoices.sample { add(.voice, "voice.sample.\(n)", "choice") { s.voiceSample = n } })
# par-settingsreplies-sweep/voice-analyse | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:53: the control `voice.analyse` does nothing: its action closure becomes `{ }` (line: add(.voice, "voice.analyse", "button") { s.analyse(s.voiceSample ?? s.voice?.sampleSize ?? 100) })
# par-settingsreplies-sweep/voice-examples-0 | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:54: the control `voice.examples.0` does nothing: its action closure becomes `{ }` (line: for n in RepliesChoices.examples { add(.voice, "voice.examples.\(n)", "choice") { s.setExamples(n) } })
# par-settingsreplies-sweep/voice-examples-1 | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:54: the control `voice.examples.1` does nothing: its action closure becomes `{ }` (line: for n in RepliesChoices.examples { add(.voice, "voice.examples.\(n)", "choice") { s.setExamples(n) } })
# par-settingsreplies-sweep/voice-examples-3 | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:54: the control `voice.examples.3` does nothing: its action closure becomes `{ }` (line: for n in RepliesChoices.examples { add(.voice, "voice.examples.\(n)", "choice") { s.setExamples(n) } })
# par-settingsreplies-sweep/voice-examples-5 | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:54: the control `voice.examples.5` does nothing: its action closure becomes `{ }` (line: for n in RepliesChoices.examples { add(.voice, "voice.examples.\(n)", "choice") { s.setExamples(n) } })
# par-settingsreplies-sweep/voice-tones-row | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:55: the control `voice.tones` does nothing: its action closure becomes `{ }` (line: add(.voice, "voice.tones", "row") { s.popover = .tones })
# par-settingsreplies-sweep/voice-tone-add | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:41: the control `voice.tone.add` does nothing: its action closure becomes `{ }` (line: for id in ["voice"] + s.tones.map(\.id) { add(page, "\(key).tone.\(id)", "choice") { s.setTone(kind, id) } })
# par-settingsreplies-sweep/voice-tone-delete | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:56: the control `voice.tone.tone-formal.delete` does nothing: its action closure becomes `{ }` (line: for t in s.tones { add(.voice, "voice.tone.\(t.id).delete", "button") { s.deleteTone(t.id) } })
# par-settingsreplies-sweep/prompt-variables-row | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:62: the control `prompt.variables` does nothing: its action closure becomes `{ }` (line: add(.prompt, "prompt.variables", "row") { s.popover = .variables })
# par-settingsreplies-sweep/prompt-notes-row | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:64: the control `prompt.notes` does nothing: its action closure becomes `{ }` (line: add(.prompt, "prompt.notes", "row") { s.popover = .notes })
# par-settingsreplies-sweep/prompt-versions-row | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:67: the control `prompt.versions` does nothing: its action closure becomes `{ }` (line: add(.prompt, "prompt.versions", "row") { s.popover = .versions })
# par-settingsreplies-sweep/prompt-preview | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:61: the control `prompt.preview` does nothing: its action closure becomes `{ }` (line: add(.prompt, "prompt.preview", "button") { s.openPreview() })
# par-settingsreplies-sweep/prompt-note-add | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:66: the control `prompt.note.add` does nothing: its action closure becomes `{ }` (line: add(.prompt, "prompt.note.add", "button") { s.saveNote(RepliesDoors.sampleNote) })
# par-settingsreplies-sweep/prompt-note-delete | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:65: the control `prompt.note.note-1.delete` does nothing: its action closure becomes `{ }` (line: for n in s.notes { add(.prompt, "prompt.note.\(n.id).delete", "button") { s.deleteNote(n.id) } })
# par-settingsreplies-sweep/prompt-version-restore | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:68: the control `prompt.version.1.restore` does nothing: its action closure becomes `{ }` (line: for v in s.versions where !v.current { add(.prompt, "prompt.version.\(v.version).restore", "button") { s.re...)
# par-settingsreplies-sweep/prompt-variable-message | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:63: the control `prompt.variable.message` does nothing: its action closure becomes `{ }` (line: for v in s.variables { add(.prompt, "prompt.variable.\(v.id)", "row") { s.placeVariable(v) } })
# par-settingsreplies-sweep/prompt-variable-thread | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:63: the control `prompt.variable.thread` does nothing: its action closure becomes `{ }` (line: for v in s.variables { add(.prompt, "prompt.variable.\(v.id)", "row") { s.placeVariable(v) } })
# par-settingsreplies-sweep/prompt-variable-instruction | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:63: the control `prompt.variable.instruction` does nothing: its action closure becomes `{ }` (line: for v in s.variables { add(.prompt, "prompt.variable.\(v.id)", "row") { s.placeVariable(v) } })
# par-settingsreplies-sweep/prompt-variable-excerpts | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:63: the control `prompt.variable.excerpts` does nothing: its action closure becomes `{ }` (line: for v in s.variables { add(.prompt, "prompt.variable.\(v.id)", "row") { s.placeVariable(v) } })
# par-settingsreplies-sweep/prompt-variable-calendar | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:63: the control `prompt.variable.calendar` does nothing: its action closure becomes `{ }` (line: for v in s.variables { add(.prompt, "prompt.variable.\(v.id)", "row") { s.placeVariable(v) } })
# par-settingsreplies-sweep/prompt-variable-tone | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:63: the control `prompt.variable.tone` does nothing: its action closure becomes `{ }` (line: for v in s.variables { add(.prompt, "prompt.variable.\(v.id)", "row") { s.placeVariable(v) } })
# par-settingsreplies-sweep/prompt-variable-notes | Sources/Features/SettingsRepliesMac/RepliesTextDoors.swift:63: the control `prompt.variable.notes` does nothing: its action closure becomes `{ }` (line: for v in s.variables { add(.prompt, "prompt.variable.\(v.id)", "row") { s.placeVariable(v) } })
# par-settingsreplies-sweep/editor-prompt-click | Sources/Features/SettingsRepliesMac/RepliesEditorDoors.swift:19: the editor door does nothing: its action closure becomes `{ }` (line: add(page, "editor.\(k.raw)", "text") { _ = cursor(k, at: .max) })
# par-settingsreplies-sweep/editor-prompt-save | Sources/Features/SettingsRepliesMac/RepliesEditorDoors.swift:20: the editor door does nothing: its action closure becomes `{ }` (line: add(page, "editor.\(k.raw).save", "button") { s.save(k) })
# par-settingsreplies-sweep/editor-prompt-revert | Sources/Features/SettingsRepliesMac/RepliesEditorDoors.swift:21: the editor door does nothing: its action closure becomes `{ }` (line: add(page, "editor.\(k.raw).revert", "button") { let e = s.editor(k); if e.confirmRevert { e.revert() } else...)
# par-settingsreplies-sweep/editor-retrieval-prompt-click | Sources/Features/SettingsRepliesMac/RepliesEditorDoors.swift:19: the editor door does nothing: its action closure becomes `{ }` (line: add(page, "editor.\(k.raw)", "text") { _ = cursor(k, at: .max) })
# par-settingsreplies-sweep/editor-retrieval-prompt-save | Sources/Features/SettingsRepliesMac/RepliesEditorDoors.swift:20: the editor door does nothing: its action closure becomes `{ }` (line: add(page, "editor.\(k.raw).save", "button") { s.save(k) })
# par-settingsreplies-sweep/editor-retrieval-prompt-revert | Sources/Features/SettingsRepliesMac/RepliesEditorDoors.swift:21: the editor door does nothing: its action closure becomes `{ }` (line: add(page, "editor.\(k.raw).revert", "button") { let e = s.editor(k); if e.confirmRevert { e.revert() } else...)
# par-settingsreplies-sweep/editor-voice-text-click | Sources/Features/SettingsRepliesMac/RepliesEditorDoors.swift:19: the editor door does nothing: its action closure becomes `{ }` (line: add(page, "editor.\(k.raw)", "text") { _ = cursor(k, at: .max) })
# par-settingsreplies-sweep/editor-voice-text-save | Sources/Features/SettingsRepliesMac/RepliesEditorDoors.swift:20: the editor door does nothing: its action closure becomes `{ }` (line: add(page, "editor.\(k.raw).save", "button") { s.save(k) })
# par-settingsreplies-sweep/editor-voice-text-revert | Sources/Features/SettingsRepliesMac/RepliesEditorDoors.swift:21: the editor door does nothing: its action closure becomes `{ }` (line: add(page, "editor.\(k.raw).revert", "button") { let e = s.editor(k); if e.confirmRevert { e.revert() } else...)
# par-settingsreplies-sweep/editor-ai-description-click | Sources/Features/SettingsRepliesMac/RepliesEditorDoors.swift:19: the editor door does nothing: its action closure becomes `{ }` (line: add(page, "editor.\(k.raw)", "text") { _ = cursor(k, at: .max) })
# par-settingsreplies-sweep/editor-ai-description-save | Sources/Features/SettingsRepliesMac/RepliesEditorDoors.swift:20: the editor door does nothing: its action closure becomes `{ }` (line: add(page, "editor.\(k.raw).save", "button") { s.save(k) })
# par-settingsreplies-sweep/editor-ai-description-revert | Sources/Features/SettingsRepliesMac/RepliesEditorDoors.swift:21: the editor door does nothing: its action closure becomes `{ }` (line: add(page, "editor.\(k.raw).revert", "button") { let e = s.editor(k); if e.confirmRevert { e.revert() } else...)
# par-settingsreplies-sweep/editor-new-note-click | Sources/Features/SettingsRepliesMac/RepliesEditorDoors.swift:19: the editor door does nothing: its action closure becomes `{ }` (line: add(page, "editor.\(k.raw)", "text") { _ = cursor(k, at: .max) })
# par-settingsreplies-sweep/editor-new-note-save | Sources/Features/SettingsRepliesMac/RepliesEditorDoors.swift:20: the editor door does nothing: its action closure becomes `{ }` (line: add(page, "editor.\(k.raw).save", "button") { s.save(k) })
# par-settingsreplies-sweep/editor-new-note-revert | Sources/Features/SettingsRepliesMac/RepliesEditorDoors.swift:21: the editor door does nothing: its action closure becomes `{ }` (line: add(page, "editor.\(k.raw).revert", "button") { let e = s.editor(k); if e.confirmRevert { e.revert() } else...)
# par-settingsreplies-sweep/editor-notes-click | Sources/Features/SettingsRepliesMac/RepliesEditorDoors.swift:19: the editor door does nothing: its action closure becomes `{ }` (line: add(page, "editor.\(k.raw)", "text") { _ = cursor(k, at: .max) })
# par-settingsreplies-sweep/editor-notes-save | Sources/Features/SettingsRepliesMac/RepliesEditorDoors.swift:20: the editor door does nothing: its action closure becomes `{ }` (line: add(page, "editor.\(k.raw).save", "button") { s.save(k) })
# par-settingsreplies-sweep/editor-notes-revert | Sources/Features/SettingsRepliesMac/RepliesEditorDoors.swift:21: the editor door does nothing: its action closure becomes `{ }` (line: add(page, "editor.\(k.raw).revert", "button") { let e = s.editor(k); if e.confirmRevert { e.revert() } else...)
# par-settingsreplies-sweep/every-listed-control-has-a-case | Sources/Features/SettingsRepliesMac/RepliesDoors.swift:27: one more control is listed that no case drives: a line `add(.sources, "sources.new-thing", "button") { }` is added after this one (line: uses(s, add))
# MUTANTS-END


# ---------------------------------------------------------------- the self-test: the model is checks/settingsreplies-check.py FakeApp, one door at a time made dead
class Model(sr.FakeApp):
    """FakeApp plus the Retrieval figures the real `state` row serves (RepliesRows.state: per_question, size, keep, clash), which the older model never drew."""
    def state(self):
        s = super().state()
        r = self.retrieval
        s["retrieval"].update(per_question=r["excerpts_per_question"], size=r["excerpt_size"], keep=r["keep_n"], clash=r["clash_rule"])
        return s


    def handle(self, verb, args):
        if verb == "click" and args[0] in (P + ".sources.add-folder.typed.focus", P + ".sources.add-folder.typed.commit"):
            if args[0].endswith("focus"):
                self.typed_on, self.typed = True, ""
            elif getattr(self, "typed_on", False) and self.typed and "noop:typed.sources-add-folder" not in self.m:
                self.local.append({"source": {"kind": "local_folder", "path": self.typed}, "enabled": True, "item_count": 0, "progress": {"state": "syncing"}})
                self.typed_on = False
            return {}
        if verb == "type" and getattr(self, "typed_on", False):
            self.typed += args[0]
            return {}
        if verb == "rows" and args[0] == P + ".typed":
            return {"rows": [{"field": "sources.add-folder" if getattr(self, "typed_on", False) else "", "words": getattr(self, "typed", "")}]}
        if verb == "key":          # the socket's key verb: Esc closes an open popover (the feature's key binding)
            if args[:1] == ["Escape"] and self.ui["popover"] is not None and "noop:key.escape" not in self.m:
                self.ui["popover"] = None
            return {}
        return super().handle(verb, args)

    def keyevent(self, chord):
        if chord == "⌘n" and self.ui["page"] == "sources":     # the shortcut of Add a source is a real key event: it runs the button's action
            if "noop:key.cmd-n" not in self.m:
                self.doors()["sources.add"]()
            return {"window": "Whoosh", "window_id": "mac-settings", "focus": "none", "route": "keyEquivalent", "handled": True}
        return super().keyevent(chord)


def serve(mutants):
    d = tempfile.mkdtemp(prefix="sw-sr-")
    srv = socketserver.ThreadingUnixStreamServer(os.path.join(d, "whoosh.sock"), sr._Handler)
    srv.app = Model(mutants)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return d, srv


def model_run(mutants, only=None):
    d, srv = serve(mutants)
    ctl = Ctl(d).connect()
    try:
        j = Journey(ID, ctl)
        drive(j, only)
        return {c.label.split("/", 1)[1]: (c.state, c.detail) for c in j.checks()}
    finally:
        ctl.close()
        srv.shutdown()
        srv.server_close()
        shutil.rmtree(d, ignore_errors=True)


def selftest():
    bad = 0
    clean = model_run(())
    for name, (st, why) in clean.items():
        print("model %-44s %-8s %s" % (name, st, " ".join(why.split())[:150]))
        if st != "pass":
            print("RED on the clean model: %s %s" % (name, why))
            bad += 1
    doors = set(Model().doors()) | {"key.escape", "key.cmd-n", "typed.sources-add-folder"}     # two keys, not doors: the model answers them in Model.handle and Model.keyevent
    declared = {l.split(" | ")[0][2:].strip().split("/", 1)[1] for l in open(os.path.abspath(__file__), encoding="utf-8") if l.startswith("# " + ID + "/")}
    for name, _ds, noop, _fn in CASES:
        mutant = noop if noop == "extra-control" else "noop:" + noop
        if noop != "extra-control" and noop not in doors:
            print("NO DOOR in the model for %s: %s" % (name, noop))
            bad += 1
            continue
        if name not in declared:
            print("NO MUTANT declared for %s" % name)
            bad += 1
        st = model_run((mutant,), only=name)[name][0]
        print("mutant %-44s %-34s -> %s" % (name, mutant, "RED" if st == "fail" else "NOT RED (%s)" % st))
        bad += st != "fail"
    for l in sorted(declared - {c[0] for c in CASES}):
        print("STRAY MUTANT %s" % l)
        bad += 1
    dry = Journey(ID, None)
    drive(dry)
    n_dry = len({c.label for c in dry.checks()})
    print("whole check, undriven: %d cases (all unjudged: %s)" % (n_dry, all(c.state == "unjudged" for c in dry.checks())))
    bad += n_dry != len(CASES) or not all(c.state == "unjudged" for c in dry.checks())
    print("selftest: %d cases, %d on the clean model green, %d mutants, %d not red or missing" % (len(CASES), sum(1 for v in clean.values() if v[0] == "pass"), len(declared), bad))
    if not bad:
        print("selftest: every check was seen red on its broken model")
    return 1 if bad else 0


def main():
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    print_cases(r)
    print("MUTANTS: %d declared in %s (a check is green only after it was seen red on its broken copy)" % (len(declared_mutants(path)), os.path.basename(path)))
    for line in r["fail_list"]:
        sys.stderr.write("FAIL %s\n" % line)
    for line in r["no_mutant"]:
        sys.stderr.write("NO MUTANT %s\n" % line)
    for line in r["stray_mutant"]:
        sys.stderr.write("STRAY MUTANT %s\n" % line)
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    return 0 if r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"] else 1


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else main())
