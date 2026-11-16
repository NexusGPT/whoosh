#!/usr/bin/env python3
"""settingsreplies-check: J10, Settings > Réponses (docs/JOURNEYS.md J10; screens docs/design/boards/ai-replies-r7; contract docs/lanes/replies-contract.md).

    python3 checks/settingsreplies-check.py              drive the running rig copy ($WHOOSH_SOCKET_DIR, --fixture-core); exit 1 when F>0 or U>0
    python3 checks/settingsreplies-check.py --selftest   run every check against a stand-in app: clean, then one mutant PER CONTROL

Every control of the six panes and of « Quels messages » is a click door (`mac-settings-replies.controls` lists them). Each case clicks
doors and reads TWO things after: the core's own value (`core state reply/...`, the delta 20 contract) and what the pane draws
(`rows mac-settings-replies.state`), so a door that writes nothing, or a pane that does not re-read, is red. The last case proves the cases
cover every control the app lists (none missing). Expected values are literals from the contract and the board, never from FakeApp.
FakeApp (--selftest only) stands in for the app on the control-socket wire; its mutant `noop:<door>` makes that one door do nothing.
"""
import fnmatch
import json
import os
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable  # noqa: E402

P = "mac-settings-replies"

CAPDIR = os.path.join(ROOT, "_captures", "settingsreplies-mac")
# the fixed samples the text doors type (RepliesDoors.swift); the board's French
SENDER, DESCRIPTION, FOLDER = "laura@example.com", "Les messages d’un client qui pose une question.", "~/Documents/Notes"
FILTER_Q, APPEND = "Cet extrait parle-t-il de ce client ?", "Réponds en français."
FACTS = ["unread", "unanswered", "has_attachment", "starred", "vip_sender", "cc_me", "mailing_list", "replied_by_me"]


class Red(Exception):
    """The app answered and the answer is wrong: the case is red."""


class Excluded(Exception):
    """A case that can never be judged on this run's engine, by design (lead's ruling, round 16): its words start with the engine that does judge it.
    It prints `ok <case> excluded, <words>` and counts as neither failed nor not judged."""


CASES = []   # (label, door patterns, fn)


def case(label, *doors):
    def deco(fn):
        CASES.append((label, list(doors), fn))
        return fn
    return deco


# ---------------------------------------------------------------- reads and acts
def click(ctl, door):
    try:
        ctl.click("%s.%s" % (P, door))
    except Unreadable as e:
        raise Red("click %s refused: %s" % (door, e))


def state(ctl):
    return ctl.rows("%s.state" % P)[0]


def core(ctl, path):
    return ctl.core(path)


def page(ctl):
    return ctl.rows("%s.place" % P)[0]["page"]


def controls(ctl):
    return [r["ax"][len(P) + 1:] for r in ctl.rows("%s.controls" % P)]


def want(cond, words):
    if not cond:
        raise Red(words)


def flip(ctl, door, read):
    """A switch: the value the core holds AND the value the pane draws both turn over."""
    c0, s0 = read(ctl)
    click(ctl, door)
    c1, s1 = read(ctl)
    want(c1 == (not c0), "%s: core %r -> %r, expected the opposite" % (door, c0, c1))
    want(s1 == c1, "%s: the pane draws %r, the core holds %r" % (door, s1, c1))


def reset(ctl):
    try:
        ctl.send("fixture", "reset")
        ctl.click("%s.fixture.reset" % P)
    except Unreadable:
        pass


# ---------------------------------------------------------------- the cases
@case("pages-open", "page.*")
def _pages(ctl):
    for p in ["filter", "auto", "on-demand", "sources", "retrieval", "voice", "prompt"]:   # never the page already shown first
        click(ctl, "page." + p)
        want(page(ctl) == p, "page.%s shows %r" % (p, page(ctl)))
        want(state(ctl)["ui"]["page"] == p, "page.%s: the store says %r" % (p, state(ctl)["ui"]["page"]))
    return True


@case("links-go-where-they-say", "auto.step.*", "auto.sources", "auto.prompt", "on-demand.sources", "on-demand.prompt")
def _links(ctl):
    for door, to in [("auto.step.entry", "filter"), ("auto.step.ai", "filter"), ("auto.step.exit", "filter"), ("auto.sources", "sources"),
                     ("auto.prompt", "prompt"), ("on-demand.sources", "sources"), ("on-demand.prompt", "prompt")]:
        click(ctl, "page.voice")
        click(ctl, door)
        want(page(ctl) == to, "%s opened %r, not %r" % (door, page(ctl), to))
    return True


@case("use-switches", "auto.enabled", "auto.match-thread", "on-demand.match-thread", "on-demand.instruction-before", "on-demand.change-after")
def _switches(ctl):
    flip(ctl, "auto.enabled", lambda c: (core(c, "reply/use/auto")["enabled"], state(c)["auto_enabled"]))
    flip(ctl, "auto.match-thread", lambda c: (core(c, "reply/use/auto")["match_thread"], state(c)["auto_match_thread"]))
    flip(ctl, "on-demand.match-thread", lambda c: (core(c, "reply/use/on_demand")["match_thread"], state(c)["on_demand_match_thread"]))
    flip(ctl, "on-demand.instruction-before", lambda c: (core(c, "reply/on_demand")["instruction_before"], state(c)["instruction_before"]))
    flip(ctl, "on-demand.change-after", lambda c: (core(c, "reply/on_demand")["change_after"], state(c)["change_after"]))
    return True


@case("tones-of-each-use", "auto.tone.*", "on-demand.tone.*")
def _tones(ctl):
    for key, path, field in [("auto", "reply/use/auto", "auto_tone"), ("on-demand", "reply/use/on_demand", "on_demand_tone")]:
        doors = [d for d in controls(ctl) if d.startswith(key + ".tone.")]
        want(len(doors) >= 3, "%s: %d tone choices, expected my voice and the board's two tones" % (key, len(doors)))
        for d in sorted(doors, key=lambda d: d.endswith(".voice")):   # my voice last, so every click changes the value
            tone = d.split(".tone.", 1)[1]
            click(ctl, d)
            want(core(ctl, path)["tone_id"] == tone, "%s: the core holds %r" % (d, core(ctl, path)["tone_id"]))
            want(state(ctl)[field] == tone, "%s: the pane draws %r" % (d, state(ctl)[field]))
    return True


@case("trial-on-a-message", "auto.try", "retrieval.try", "popover.close")
def _trial(ctl):
    for door, kind in [("auto.try", "auto"), ("retrieval.try", "on_demand")]:
        click(ctl, door)
        ui = state(ctl)["ui"]
        want(ui["popover"] == "trial" and ui["trial_kind"] == kind, "%s: popover %r, trial %r" % (door, ui["popover"], ui["trial_kind"]))
        click(ctl, "popover.close")
        want(state(ctl)["ui"]["popover"] is None, "popover.close left %r open" % state(ctl)["ui"]["popover"])
    return True


@case("filter-cards", "filter.entry.switch", "filter.ai.switch", "filter.exit.switch")
def _cards(ctl):
    for k in ["entry", "ai", "exit"]:
        flip(ctl, "filter.%s.switch" % k, lambda c, k=k: (core(c, "reply/filter")[k]["enabled"], state(c)["%s_on" % k]))
    return True


@case("filter-labels", "filter.scope.all", "filter.scope.label.*")
def _labels(ctl):
    for d in [d for d in controls(ctl) if d.startswith("filter.scope.label.")]:
        lid = d[len("filter.scope.label."):]
        before = lid in core(ctl, "reply/filter")["scope"]["label_ids"]
        click(ctl, d)
        after = core(ctl, "reply/filter")["scope"]
        want((lid in after["label_ids"]) == (not before), "%s: ticked %r -> %r" % (d, before, lid in after["label_ids"]))
        want((lid in state(ctl)["scope_label_ids"]) == (lid in after["label_ids"]), "%s: the pane disagrees" % d)
    click(ctl, "filter.scope.all")
    want(core(ctl, "reply/filter")["scope"]["all"] is True and state(ctl)["scope_all"] is True, "filter.scope.all: not all labels")
    return True


@case("filter-match", "filter.entry.match.*", "filter.exit.match.*")
def _match(ctl):
    for k in ["entry", "exit"]:
        for m in ["any", "all"]:
            click(ctl, "filter.%s.match.%s" % (k, m))
            want(core(ctl, "reply/filter")[k]["rule"]["match"] == m, "filter.%s.match.%s: core %r" % (k, m, core(ctl, "reply/filter")[k]["rule"]["match"]))
            want(state(ctl)["%s_match" % k] == m, "filter.%s.match.%s: the pane draws %r" % (k, m, state(ctl)["%s_match" % k]))
    return True


def _facts_case(k):
    def fn(ctl):
        for f in FACTS:
            for s in ["yes", "no", "any"]:
                click(ctl, "filter.%s.fact.%s.%s" % (k, f, s))
                got = core(ctl, "reply/filter")[k]["rule"]["facts"][f]
                want(got == s, "filter.%s.fact.%s.%s: core %r" % (k, f, s, got))
                want(state(ctl)["%s_facts" % k][f] == s, "filter.%s.fact.%s.%s: the pane draws %r" % (k, f, s, state(ctl)["%s_facts" % k][f]))
        return True
    return fn


case("filter-entry-facts", "filter.entry.fact.*")(_facts_case("entry"))
case("filter-exit-facts", "filter.exit.fact.*")(_facts_case("exit"))


@case("filter-sender-reads", "filter.entry.sender", "filter.entry.sender.clear", "filter.ai.reads.*")
def _texts(ctl):
    click(ctl, "filter.entry.sender")
    rows = core(ctl, "reply/filter")["entry"]["rule"]["rows"]
    want({"kind": "from", "words": SENDER} in rows and state(ctl)["sender"] == SENDER, "sender: rows %r, pane %r" % (rows, state(ctl)["sender"]))
    click(ctl, "filter.entry.sender.clear")
    want(core(ctl, "reply/filter")["entry"]["rule"]["rows"] == [] and state(ctl)["sender"] == "", "sender.clear left %r" % core(ctl, "reply/filter")["entry"]["rule"]["rows"])
    for r, field in [("subject", "subject"), ("body", "body"), ("rest", "rest_of_thread"), ("attachments", "attachments")]:
        flip(ctl, "filter.ai.reads." + r, lambda c, f=field, r=r: (core(c, "reply/filter")["ai"][f], state(c)["reads"][r]))
        click(ctl, "filter.ai.reads." + r)   # back, so the AI always reads something
    return True


@case("sources-tiles-calendars-scope", "sources.account.*", "sources.local.*", "sources.calendar.*", "sources.scope.*")
def _sources(ctl):
    def tile(c, ax):
        return [r for r in c.rows("%s.sources" % P) if r["ax"] == "%s.%s" % (P, ax)][0]
    doors = controls(ctl)
    for d in [d for d in doors if d.startswith("sources.account.") and ".folder." not in d]:
        o0 = tile(ctl, d)["open"]
        click(ctl, d)
        want(tile(ctl, d)["open"] == (not o0), "%s: the card did not open or close (was %r, open accounts now %r)" % (d, o0, state(ctl)["ui"].get("open_accounts")))
    for d in [d for d in doors if (".folder." in d or d.startswith("sources.local.") or d.startswith("sources.calendar.")) and not d.endswith(".remove")]:
        e0 = tile(ctl, d)["enabled"]
        n0 = core(ctl, "reply/sources")["enabled_total"]
        click(ctl, d)
        want(tile(ctl, d)["enabled"] == (not e0), "%s: the tile did not turn over" % d)
        want(core(ctl, "reply/sources")["enabled_total"] == n0 + (-1 if e0 else 1), "%s: the core's count did not follow" % d)
    for sc in ["all_accounts", "this_account"]:
        click(ctl, "sources.scope." + sc)
        want(core(ctl, "reply/retrieval")["scope"] == sc and state(ctl)["search_scope"] == sc, "sources.scope.%s not kept" % sc)
    return True


@case("sources-add-and-remove-folder", "sources.add", "sources.add-folder", "sources.picker.cancel", "sources.add-folder.sample", "sources.local.*.remove")
def _folders(ctl):
    for d in ["sources.add", "sources.add-folder"]:
        asks = _asks(ctl)
        click(ctl, d)
        want(state(ctl)["ui"]["picker_open"] or _asks(ctl) == asks + 1, "%s: no folder chooser asked for" % d)
        click(ctl, "sources.picker.cancel")
        want(not state(ctl)["ui"]["picker_open"], "sources.picker.cancel left the chooser open")
    click(ctl, "sources.add-folder.sample")
    paths = [s["source"]["path"] for s in core(ctl, "reply/sources")["local_folders"]]
    want(FOLDER in paths, "the sample folder is not a source: %r" % paths)
    for d in sorted([d for d in controls(ctl) if d.endswith(".remove") and d.startswith("sources.local.")], reverse=True):
        n = len(core(ctl, "reply/sources")["local_folders"])
        click(ctl, d)
        want(len(core(ctl, "reply/sources")["local_folders"]) == n - 1, "%s removed nothing" % d)
    return True


# ---------------------------------------------------------------- delta 24: a mail folder gone from the server
# The rig reaches the state through the pane's own seam `fixture.folder-gone|folder-back.<account>.<i>` (fixture core only), until the socket
# verb `fixture reply-folder-gone <account> <folder> [back]` (lane probes-r1) exists. On a core without the seam these cases are NOT JUDGED.
# ---------------------------------------------------------------- hid-ai round 6: the AI card's description in the app's language ("don't mix languages")
# The shipped description is a catalogue sentence; while the stored text is still that default (the engine's English seed or any of its 14 texts) the box
# shows it in the app's language; text the owner typed and saved is shown as typed in every language (RepliesWords.aiDescriptionShown).
LANG_DOOR = "mac-settings-general.appearance.language.%s"
DESC_KEY = "Emails that wait for a reply from me: a question, a request, a decision that someone is waiting for."
with open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8") as _c:
    _DESC = {k: v["stringUnit"]["value"] for k, v in json.load(_c)["strings"][DESC_KEY]["localizations"].items()}
LANG_MUTANTS = {"description-not-localized": "the shipped description shows in the seed's English whatever the app's language",
                "typed-description-replaced": "the owner's saved description is replaced by the default when the language changes"}

# ---------------------------------------------------------------- hid-ai round 18: the shipped defaults the model reads follow the app's language
# The lead's ruling (round 18): while untouched, the reply prompt, the retrieval prompt and the AI filter question are SHOWN and SENT in the app's
# language (the catalogue's text of the engine's English default); text the owner wrote stays as written (RepliesStore.followLanguage).
PROMPT_KEY = ("You are replying to {{message}}. You have read {{thread}} and {{excerpts}}. My availability is in {{calendar}}.\n"
              "Write the reply in this tone: {{tone}}. Follow {{instruction}} when there is one. Always keep in mind: {{notes}}.\n"
              "Answer the question first. Never promise a price or a deadline.\n"
              "// One sentence per rule. This line is a note: the model does not read it.")
RETR_KEY = "From {{message}} and {{instruction}}, write up to 3 short questions to find what I need to know before I reply."
FILTER_KEY = "Does this excerpt help to answer?"
SHIPPED_AT = {"prompt": PROMPT_KEY, "retrieval_prompt": RETR_KEY, "ai_filter_question": FILTER_KEY}
with open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8") as _c:
    _cat = json.load(_c)["strings"]
    _SHIPPED = {k: {l: v["stringUnit"]["value"] for l, v in _cat[k]["localizations"].items()} for k in SHIPPED_AT.values() if k in _cat}
DEFAULT_MUTANTS = {"defaults-stay-english": "the shipped defaults stay in the engine's English whatever the app's language",
                   "typed-text-replaced": "a text the owner wrote is replaced by the default when the language changes"}


def _shipped(key, lang):
    return _SHIPPED.get(key, {}).get(lang)


@case("shipped-defaults-follow-the-app-language")
def _defaults_follow(ctl):
    want(len(_SHIPPED) == 3, "the catalogue lacks a shipped default: %r" % sorted(set(SHIPPED_AT.values()) - set(_SHIPPED)))

    def relaunch_in(code):
        ctl.click(LANG_DOOR % code)
        click(ctl, "fixture.relaunch")      # the pane reads the core again (RepliesStore.load: the defaults follow first)

    def held():
        r, cur, ed = core(ctl, "reply/retrieval"), core(ctl, "reply/prompt/current"), editors(ctl)
        return {"prompt": (cur["text"], ed["prompt"]["stored"]), "retrieval_prompt": (r["retrieval_prompt"], ed["retrievalPrompt"]["stored"]),
                "ai_filter_question": (r["ai_filter_question"], r["ai_filter_question"])}
    try:
        relaunch_in("fr")
        if core(ctl, "reply/prompt/current")["text"] not in set(_SHIPPED[PROMPT_KEY].values()):
            click(ctl, "prompt.version.1.restore")   # the seed's current prompt is the owner's own (version 3); version 1 is the shipped one
        h = held()
        off = {k: v for k, v in h.items() if v[0] != _shipped(SHIPPED_AT[k], "fr") or v[1] != v[0]}
        want(not off, "in French, (core, pane) %r; want the catalogue's French for each, the pane showing what the core holds" % off)
        third = _shipped(PROMPT_KEY, "fr").split("\n")[2]
        pv = core(ctl, "reply/prompt/preview") or ""
        want(third in pv and "Answer the question first." not in pv and "//" not in pv,
             "what the model receives (reply/prompt/preview) does not carry the French default: %r" % pv[:160])
        click(ctl, "retrieval.ai-filter.save")      # the door saves the sample question (RepliesDoors.sampleFilter): the owner's own words
        typed = core(ctl, "reply/retrieval")["ai_filter_question"]
        want(typed not in set(_SHIPPED[FILTER_KEY].values()), "the sample question %r is a shipped default: the case cannot tell an edit" % typed)
        relaunch_in("en")
        h2 = held()
        want(h2["ai_filter_question"][0] == typed, "the owner's question %r became %r after the language changed" % (typed, h2["ai_filter_question"][0]))
        want(h2["prompt"][0] == _shipped(PROMPT_KEY, "en") and h2["retrieval_prompt"][0] == _shipped(RETR_KEY, "en"),
             "back in English the untouched defaults read %r" % ({k: v[0][:50] for k, v in h2.items()},))
    finally:
        try:
            ctl.click(LANG_DOOR % "system")
        except Unreadable:
            pass
    return True


SOURCE_MUTANTS = {"card-reopens-on-read": "the first account's card opens again on the next read once the owner closed it (RepliesStore.load, main76)"}
GONE_MUTANTS = {"gone-stays-on": "a gone folder stays switched on", "gone-loses-count": "a gone folder loses its last count",
                "gone-no-line": "a gone tile says nothing", "gone-switch-enables": "a click on a gone tile switches it on",
                "back-forgets-choice": "a folder that returns is always switched on", "count-holds-gone": "the counts still hold a gone folder",
                "pane-keeps-own-memory": "the pane flips its own memory of the choice, not the core's owner_enabled"}


@case("ai-description-in-the-app-language")
def _desc_language(ctl):
    def shown_in(code):
        ctl.click(LANG_DOOR % code)
        click(ctl, "fixture.relaunch")      # the pane reads the core again, in the language now chosen
        return editors(ctl)["aiDescription"]["text"]
    try:
        fr, en = shown_in("fr"), shown_in("en")
        want((fr, en) == (_DESC["fr"], _DESC["en"]), "the default description read %r in French and %r in English (want the catalogue's %r, %r)" % (fr, en, _DESC["fr"], _DESC["en"]))
        open_editor(ctl, "aiDescription")
        keys(ctl, "space")
        keys(ctl, "zz")
        click(ctl, "editor.aiDescription.save")
        typed = editors(ctl)["aiDescription"]["text"]
        again = shown_in("fr")
        want(typed.endswith("zz") and again == typed, "after typing and saving the box read %r; in French then %r (want it as typed)" % (typed[-40:], again[-40:]))
    finally:
        try:
            ctl.click(LANG_DOOR % "system")
        except Unreadable:
            pass
    return True


def seam(ctl, word, account, i):
    kind = ctl.uistate().get("core")
    if kind == "rust":
        raise Excluded("test engine only: this run's core is 'rust'; the gone seam calls the fixture core's own entry (RepliesGoneSeam.swift: a real core "
                       "answers false), so the real engine can never stage a folder that left the server")
    try:
        ctl.click("%s.fixture.%s.%s.%d" % (P, word, account, i))
    except Unreadable as e:
        raise Unreadable("the gone seam is not served (fixture core only; or the verb fixture reply-folder-gone of lane probes-r1): %s" % e)


def first_account(ctl):
    return [r["ax"].split(".")[3] for r in ctl.rows("%s.sources" % P) if r.get("kind", "account") == "account" and ".folder." not in r["ax"]
            and r["ax"].startswith("%s.sources.account." % P)][0]


def tile_row(ctl, account, i):
    return [r for r in ctl.rows("%s.sources" % P) if r["ax"] == "%s.sources.account.%s.folder.%d" % (P, account, i)][0]


def core_folder(ctl, account, i):
    a = [x for x in core(ctl, "reply/sources")["accounts"] if x["account_id"] == account][0]
    return a, a["folders"][i]


def on_index(ctl, account, on):
    return [i for i, f in enumerate(core_folder(ctl, account, 0)[0]["folders"]) if f["enabled"] == on][0]


# ---------------------------------------------------------------- the model hand-over (item 4f131cf1, finding F2 of docs/lanes/tail-settings.md)
# Run alone with --handover, on a copy of the REAL engine launched with a model folder (WHOOSH_EMBED_WEIGHTS, or the search model downloaded on its run folder by
# settingsaimac-check J5.9 before a relaunch) and WHOOSH_RIG_EMBED_HANDOVER_DELAY_MS (item 26a6f568: a test copy holds the hand-over that long after its start), so the
# folders are read while the model is held and again after the hand-over. The test engine scripts its folders' progress and never indexes: not judged there.
HANDOVER_CASES = []
BACKGROUND = "mac-main.background"        # BackgroundWorkFeature.region: `embedder` reads held, handed or missing
HANDOVER_WAIT_S = 70.0                     # a check bound: HANDOVER_RIG_DELAY_MAX_MS (60 s) and a margin, for the hand-over to arrive
INDEX_WAIT_S = 60.0                        # a check bound: after the hand-over, for one held-back folder to read up to date in the core and on its tile
READ_EVERY_S = 1.0                         # the reads "some seconds apart"
HANDOVER_DELAY_MS = 60000                  # held this long from the copy's start: the cap (HANDOVER_RIG_DELAY_MAX_MS), so the account is added and crawled in time
# hid-ai round 22 (main112 lead-handover.log: a real-engine copy with no account and no source, "no switched-on folder was short of up to date"): the case
# needs MAIL whose indexing waits for the model. A folder of this Mac would not do: the engine walks it into the excerpt index without the model
# (core/crates/engine/src/replies_work/usecases/sync_sources.rs `run`), so it reads up to date before any hand-over. A mail folder is indexed only when the
# model embeds its chunks (`mail_folders`: indexed = every chunk embedded). So `--handover-launch <bundle>` starts realcore-check's local mail server
# (checks/realcore_mailfake.py, 127.0.0.1 only) with HANDOVER_MAIL messages, launches the real-engine copy with it and the model held, and the case adds the
# account through the sheet and switches its INBOX on before it reads.
HANDOVER_MAIL = 12
HANDOVER_LAUNCH = ("the launch: WHOOSH_EMBED_WEIGHTS=$S/weights-real python3 checks/settingsreplies-check.py --handover-launch <bundle> (it starts the local mail "
                   "server and launches the real-engine copy with WHOOSH_CORE_PLAIN_LOOPBACK=1 WHOOSH_CORE_CHECK_SERVERS=<imap>:<smtp> WHOOSH_EMBED_WEIGHTS "
                   "WHOOSH_RIG_EMBED_HANDOVER_DELAY_MS=%d)" % HANDOVER_DELAY_MS)
SETUP_WAIT_S = 50.0                        # a check bound: the sheet's detection, the account's first sync and its folders on the Sources pane


def _mail_setup(ctl):
    """--handover-launch's copy (WHOOSH_HANDOVER_SETUP=1): the local account added through the sheet (the steps of realcore-check.py `flow_drive`), then its INBOX
    switched on in Sources (mail folders are listed off until the owner ticks one). Words for the evidence; Unreadable naming the step that did not happen."""
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake  # noqa: E402
    A = "mac-add-account"
    try:
        ctl.click(A + ".open")
        ctl.click(A + ".address"); ctl.type(fake.ADDRESS)
        ctl.click(A + ".password"); ctl.type(fake.PASSWORD)
        ctl.click(A + ".continue")
        end = time.time() + SETUP_WAIT_S
        while time.time() < end and ((ctl.rows(A + ".state") or [{}])[0].get("stage") in ("detecting", "entry")):
            time.sleep(0.3)
        ctl.click(A + ".add")
        while time.time() < end and ((ctl.rows(A + ".state") or [{}])[0].get("stage") not in ("added", "summary")):
            time.sleep(0.3)
        stage = (ctl.rows(A + ".state") or [{}])[0].get("stage")
        ctl.click(A + ".done")
    except Unreadable as e:
        raise Unreadable("precondition not met: the local account could not be added through the sheet: %s" % e)
    if stage not in ("added", "summary"):
        raise Unreadable("precondition not met: the sheet ended at stage %r, the local account was not added" % stage)
    while time.time() < end:
        for a in core(ctl, "reply/sources")["accounts"]:
            for i, f in enumerate(a["folders"]):
                if f["source"].get("folder") == "INBOX":
                    if not f["enabled"]:
                        click(ctl, "sources.account.%s.folder.%d" % (a["account_id"], i))
                    return "account %s added (sheet stage %s), its INBOX (folder %d) switched on" % (a["account_id"], stage, i)
        time.sleep(READ_EVERY_S)
    raise Unreadable("precondition not met: %.0f s after the account was added, the Sources pane lists no INBOX for it (reply/sources accounts %r)"
                     % (SETUP_WAIT_S, [a["account_id"] for a in core(ctl, "reply/sources")["accounts"]]))


def _missing(ctl, held_reads):
    """Which precondition is missing when no folder was held back: no account, no folder switched on, or every switched-on folder already up to date."""
    accounts = core(ctl, "reply/sources")["accounts"]
    if not accounts:
        return "this copy has no mail account (%d held reads): nothing waits for the model to be indexed; %s" % (held_reads, HANDOVER_LAUNCH)
    on = [(a["account_id"], f["source"].get("folder"), f["progress"]["state"]) for a in accounts for f in a["folders"] if f["enabled"]]
    if not on:
        return "no mail folder is switched on in Sources (%d accounts, %d held reads): nothing is indexed for replies" % (len(accounts), held_reads)
    return ("every switched-on mail folder already read up to date while the model was held (%d reads): %r; the hand-over cannot be seen to move it"
            % (held_reads, on))


def handover_case(label):
    def deco(fn):
        HANDOVER_CASES.append((label, [], fn))
        return fn
    return deco


def _embedder(ctl):
    return ctl.rows(BACKGROUND)[0].get("embedder")


def _held_back(ctl):
    """{(account, folder index): (core state, tile word)} for every switched-on mail folder the core does not read up to date."""
    tiles = {r["ax"]: r for r in ctl.rows("%s.sources" % P)}
    out = {}
    for a in core(ctl, "reply/sources")["accounts"]:
        for i, f in enumerate(a["folders"]):
            st = f["progress"]["state"]
            if f["enabled"] and st not in ("up_to_date", "gone"):
                out[(a["account_id"], i)] = (st, tiles.get("%s.sources.account.%s.folder.%d" % (P, a["account_id"], i), {}).get("progress"))
    return out


@handover_case("sources-progress-reaches-up-to-date-after-the-model-is-handed-over")
def _progress_after_handover(ctl):
    kind = ctl.uistate().get("core")
    if kind != "rust":
        raise Excluded("real engine only: this run's core is %r; the test engine scripts its folders' progress (FixtureReplies) and never indexes, so a hand-over "
                       "moves nothing there (python3 checks/settingsreplies-check.py --handover on a real-engine copy)" % kind)
    try:
        click(ctl, "page.sources")
    except Red:
        pass                                 # the tile rows are the store's, read with the page shown or not
    setup = _mail_setup(ctl) if os.environ.get("WHOOSH_HANDOVER_SETUP") == "1" else None
    first = _embedder(ctl)
    if first != "held":
        raise Unreadable("the model was %r at the first read, not held: launch the copy with a model folder and WHOOSH_RIG_EMBED_HANDOVER_DELAY_MS so the folders "
                         "can be read before the hand-over (%s)" % (first, HANDOVER_LAUNCH))
    before, held_reads, t0 = _held_back(ctl), 0, time.time()
    while time.time() - t0 < HANDOVER_WAIT_S:
        e = _embedder(ctl)
        if e == "handed":
            break
        if e == "missing":
            raise Unreadable("the copy found no model (embedder 'missing'): nothing is handed over on this copy")
        before, held_reads = _held_back(ctl), held_reads + 1
        time.sleep(READ_EVERY_S)
    else:
        raise Unreadable("no hand-over within %.0f s (embedder still %r)" % (HANDOVER_WAIT_S, _embedder(ctl)))
    if not before:
        raise Unreadable("precondition not met: " + _missing(ctl, held_reads))
    t1, after = time.time(), {}
    while time.time() - t1 < INDEX_WAIT_S:
        sources = core(ctl, "reply/sources")
        tiles = {r["ax"]: r for r in ctl.rows("%s.sources" % P)}
        after = {}
        for (acc, i), (st0, word0) in before.items():
            a = [x for x in sources["accounts"] if x["account_id"] == acc][0]
            after[(acc, i)] = (a["folders"][i]["progress"]["state"], tiles.get("%s.sources.account.%s.folder.%d" % (P, acc, i), {}).get("progress"), word0)
        done = [k for k, (st, word, word0) in after.items() if st == "up_to_date" and word and word != word0 and "%" not in word]
        if done:
            print("    hand-over: %s%d held read(s), then %s up to date after %.1f s; core and tile %r" % (setup + "; " if setup else "", held_reads, done, time.time() - t1, after))
            return True
        time.sleep(READ_EVERY_S)
    raise Red("%.0f s after the hand-over no held-back folder reads up to date in the core AND on its tile: (state, tile word, tile word before) %r; before the hand-over %r"
              % (INDEX_WAIT_S, after, before))


@case("gone-tile-off-with-count-and-line", "seam:folder-gone")
def _gone_tile(ctl):
    acc = first_account(ctl)
    i = on_index(ctl, acc, True)
    n = core_folder(ctl, acc, i)[1]["item_count"]
    seam(ctl, "folder-gone", acc, i)
    _a, f = core_folder(ctl, acc, i)
    want(f["progress"]["state"] == "gone" and f["enabled"] is False and f["item_count"] == n, "core row %r (count before %r)" % (f, n))
    t = tile_row(ctl, acc, i)
    want(t["gone"] is True and t["enabled"] is False and t["line"], "the tile reads %r" % t)
    return True


@case("gone-switch-changes-no-count", "seam:folder-gone")
def _gone_switch(ctl):
    acc = first_account(ctl)
    i = on_index(ctl, acc, False)
    seam(ctl, "folder-gone", acc, i)
    total, a = core(ctl, "reply/sources")["enabled_total"], core_folder(ctl, acc, i)[0]
    click(ctl, "sources.account.%s.folder.%d" % (acc, i))
    a2, f = core_folder(ctl, acc, i)
    want(core(ctl, "reply/sources")["enabled_total"] == total and a2["enabled_folders"] == a["enabled_folders"], "a click on a gone tile moved a count")
    want(f["enabled"] is False and tile_row(ctl, acc, i)["enabled"] is False, "a click switched a gone folder on")
    want(tile_row(ctl, acc, i)["wish"] is True, "the click did not record the wish: %r" % tile_row(ctl, acc, i)["wish"])
    return True


@case("gone-back-restores-owner-choice", "seam:folder-back")
def _gone_back(ctl):
    acc = first_account(ctl)
    on, off = on_index(ctl, acc, True), on_index(ctl, acc, False)
    for i, clicks, want_on in [(on, 0, True), (on, 1, False), (on, 2, True), (off, 1, True)]:   # each click flips the choice the core holds
        reset(ctl)
        seam(ctl, "folder-gone", acc, i)
        for _ in range(clicks):
            click(ctl, "sources.account.%s.folder.%d" % (acc, i))
            ctl.click("%s.fixture.relaunch" % P)   # the pane's model starts again: the choice must come back from the core
            _a, f = core_folder(ctl, acc, i)
            want(tile_row(ctl, acc, i)["wish"] == f["owner_enabled"], "after a relaunch the tile shows %r, the core holds %r" % (tile_row(ctl, acc, i)["wish"], f["owner_enabled"]))
        seam(ctl, "folder-back", acc, i)
        f = core_folder(ctl, acc, i)[1]
        want(f["progress"]["state"] != "gone" and f["enabled"] is want_on, "folder %d, %d click(s): back %r, expected on=%r" % (i, clicks, f, want_on))
        t = tile_row(ctl, acc, i)
        want(t["gone"] is False and not t["line"] and t["enabled"] is want_on, "the tile after the return reads %r" % t)
    return True


@case("gone-left-out-of-the-count", "seam:folder-gone")
def _gone_count(ctl):
    acc = first_account(ctl)
    i = on_index(ctl, acc, True)
    total, a = core(ctl, "reply/sources")["enabled_total"], core_folder(ctl, acc, i)[0]
    line = state(ctl)["sources_line"]
    seam(ctl, "folder-gone", acc, i)
    a2 = core_folder(ctl, acc, i)[0]
    want(core(ctl, "reply/sources")["enabled_total"] == total - 1 and a2["enabled_folders"] == a["enabled_folders"] - 1, "the counts still hold the gone folder")
    want(state(ctl)["sources_line"] != line and str(total - 1) in state(ctl)["sources_line"], "the « Sources » line still reads %r" % state(ctl)["sources_line"])
    return True


def _asks(ctl):
    try:
        return ctl.core("system-ui/save-panel") or 0
    except Unreadable:
        return 0


@case("retrieval-settings", "retrieval.per-question.*", "retrieval.size.*", "retrieval.keep.*", "retrieval.clash.*", "retrieval.fields", "retrieval.fields.*")
def _retrieval(ctl):
    for d, field, conv in [("retrieval.per-question.", "excerpts_per_question", int), ("retrieval.size.", "excerpt_size", str),
                           ("retrieval.keep.", "keep_n", int), ("retrieval.clash.", "clash_rule", str)]:
        doors = [x for x in controls(ctl) if x.startswith(d)]
        want(len(doors) >= 3, "%s*: %d choices" % (d, len(doors)))
        for x in doors + doors[:1]:   # the first again at the end: every click changes the value
            v = conv(x[len(d):])
            click(ctl, x)
            want(core(ctl, "reply/retrieval")[field] == v, "%s: core %r" % (x, core(ctl, "reply/retrieval")[field]))
    click(ctl, "retrieval.fields")
    want(state(ctl)["ui"]["popover"] == "fields", "retrieval.fields did not open its list")
    for f in ["date", "sender", "folder", "subject"]:
        flip(ctl, "retrieval.fields." + f, lambda c, f=f: (core(c, "reply/retrieval")["attached_fields"][f], f in _fields_line(c)))
    return True


def _fields_line(ctl):
    """The words of the row « Joint à chaque extrait », as keys (Date, Expéditeur, Dossier, Sujet in the app's language)."""
    line = state(ctl)["retrieval"]["fields"].lower()
    words = {"date": ["date"], "sender": ["sender", "expéditeur"], "folder": ["folder", "dossier"], "subject": ["subject", "sujet", "objet"]}
    return [k for k, ws in words.items() if any(w in line for w in ws)]


@case("retrieval-ai-filter", "retrieval.ai-filter", "retrieval.ai-filter.field", "retrieval.ai-filter.save", "retrieval.ai-filter.off")
def _ai_filter(ctl):
    click(ctl, "retrieval.ai-filter")
    want(state(ctl)["ui"]["popover"] == "aiFilter", "retrieval.ai-filter did not open")
    for d in ["retrieval.ai-filter.off", "retrieval.ai-filter.field", "retrieval.ai-filter.off", "retrieval.ai-filter.save"]:
        click(ctl, d)
        q = None if d.endswith(".off") else FILTER_Q
        want(core(ctl, "reply/retrieval")["ai_filter_question"] == q and state(ctl)["retrieval"]["ai_filter"] == q, "%s: filter %r" % (d, core(ctl, "reply/retrieval")["ai_filter_question"]))
    return True


@case("voice", "voice.sample.*", "voice.analyse", "voice.examples.*", "voice.tones", "voice.tone.add", "voice.tone.*.delete")
def _voice(ctl):
    for d in [x for x in controls(ctl) if x.startswith("voice.sample.")]:
        n = int(d.rsplit(".", 1)[1])
        click(ctl, d)
        want(state(ctl)["voice"]["picked_sample"] == n, "%s: picked %r" % (d, state(ctl)["voice"]["picked_sample"]))
    click(ctl, "voice.sample.200")
    at = core(ctl, "reply/voice")["voice"]["analysed_at_ms"]
    click(ctl, "voice.analyse")
    v = core(ctl, "reply/voice")["voice"]
    want(v["sample_size"] == 200 and v["analysed_at_ms"] != at, "voice.analyse: sample %r, analysed %r" % (v["sample_size"], v["analysed_at_ms"]))
    for d in [x for x in controls(ctl) if x.startswith("voice.examples.")] + ["voice.examples.3"]:
        n = int(d.rsplit(".", 1)[1])
        click(ctl, d)
        want(core(ctl, "reply/voice")["voice"]["same_person_examples"] == n and state(ctl)["voice"]["examples"] == n, "%s not kept" % d)
    click(ctl, "voice.tones")
    want(state(ctl)["ui"]["popover"] == "tones", "voice.tones did not open")
    click(ctl, "voice.tone.add")
    want("Chaleureux" in [t["name"] for t in core(ctl, "reply/voice")["tones"]] and "Chaleureux" in state(ctl)["voice"]["tones"], "voice.tone.add added nothing")
    for d in [x for x in controls(ctl) if x.startswith("voice.tone.") and x.endswith(".delete")]:
        tid = d[len("voice.tone."):-len(".delete")]
        click(ctl, d)
        want(tid not in [t["id"] for t in core(ctl, "reply/voice")["tones"]], "%s removed nothing" % d)
    return True


@case("prompt-lists", "prompt.preview", "prompt.variables", "prompt.notes", "prompt.note.add", "prompt.note.*.delete", "prompt.versions", "prompt.version.*.restore")
def _prompt(ctl):
    click(ctl, "prompt.preview")
    pv = state(ctl)["prompt"]["preview"]
    want(state(ctl)["ui"]["popover"] == "preview" and pv and "{{" not in pv and "//" not in pv, "prompt.preview: %r" % (pv or "")[:60])
    for d, pop in [("prompt.variables", "variables"), ("prompt.notes", "notes"), ("prompt.versions", "versions")]:
        click(ctl, d)
        want(state(ctl)["ui"]["popover"] == pop, "%s did not open its list" % d)
    n = len(core(ctl, "reply/notes"))
    click(ctl, "prompt.note.add")
    n_core, n_pane = len(core(ctl, "reply/notes")), len(state(ctl)["prompt"]["notes"])
    want(n_core == n + 1 and n_pane == n + 1, "prompt.note.add: the core holds %d notes and the pane %d, expected %d (the error line: %r)" % (n_core, n_pane, n + 1, state(ctl).get("error")))
    for d in [x for x in controls(ctl) if x.startswith("prompt.note.") and x.endswith(".delete")]:
        n = len(core(ctl, "reply/notes"))
        click(ctl, d)
        want(len(core(ctl, "reply/notes")) == n - 1, "%s removed nothing" % d)
    for d in [x for x in controls(ctl) if x.startswith("prompt.version.") and x.endswith(".restore")]:
        v = int(d.split(".")[2])
        old = [x for x in core(ctl, "reply/prompt/versions") if x["version"] == v][0]["text"]
        top = max(x["version"] for x in core(ctl, "reply/prompt/versions"))
        click(ctl, d)
        cur = core(ctl, "reply/prompt/current")
        want(cur["version"] == top + 1 and cur["text"] == old and state(ctl)["prompt"]["version"] == top + 1, "%s: current %r" % (d, cur["version"]))
    return True


# ---------------------------------------------------------------- A124: one editor, always live, for every long text
# Typed keys go through the rig's real key path (`keyevent`). Three seams stand in for what `keyevent` and the socket cannot do:
# `editor.<key>.cursor.start|middle|end` (a click at that place), `.at` (the « @ » key: `keyevent` has no code for « @ »),
# `.paste-sample` (« Écris comme @tone. » on the pasteboard, then the paste path). Rows: `mac-settings-replies.editors`.
EDITOR_MUTANTS = {"cursor-ignored": "a click does not put the cursor where it lands", "old-double-click": "the old double-click edit mode is still there",
                  "pill-backspace-char": "Backspace eats one letter of a pill", "paste-stays-text": "a pasted « @tone » stays words",
                  "unknown-silent": "an unknown « @mot » is not said", "menu-no-filter": "typing after « @ » does not filter the list",
                  "undo-skips-pill": "⌘Z does not take a pill back", "note-late": "a « // » line is drawn as a note only once saved",
                  "save-shown-clean": "Save shows with nothing to save", "esc-reverts-at-once": "Esc throws the changes away without asking",
                  "esc-clean-destroys": "Esc with no change empties the text", "draft-lost-on-leave": "leaving the page loses the draft",
                  "save-no-version": "a save of the prompt adds no version", "preview-reads-saved": "the preview shows the saved text while a draft differs",
                  "one-place-other-component": "one long text uses another editor",
                  "keys-eaten-by-shell": "Esc (the engine's first) and ⌘S, ⌘↩ (not listed by the view's ownsChord) stop at the app's key monitor",
                  "undo-keeps-menu": "⌘Z with the @ list open leaves the list open",
                  "typing-not-undoable": "typed words leave no undo step (main74: undo_depth 0 after three changes)",
                  "text-outside-card": "the text is drawn below its card (main74 rest-1.png)",
                  "keyevent-main-window": "the key verb builds its keys for the main window while the editor is in the Settings window (main76 probe)",
                  "click-without-window": "a click in an editor whose view has no window yet answers true and gives no focus (main76 voiceText)",
                  "keyevent-main-window-text-arrives": "the verb builds its keys for the main window (as KeyEventVerb.swift:82 did) though the letters still reach the editor"}
PLACES = {"prompt": "prompt", "retrievalPrompt": "retrieval", "voiceText": "voice", "aiDescription": "filter", "note.new": "prompt"}


def editors(ctl):
    return {r["key"]: r for r in ctl.rows("%s.editors" % P)}


def hang_sample(err):
    """Round 12/13: when the copy stops answering (a timed-out verb), what its threads are doing: `/usr/bin/sample <pid> 2` of the rig copy. Round 13:
    the file is kept in the repo's _captures/hang-samples (the run folder is removed after the run: main104's was gone), and the line names the main
    thread's DEEPEST frames: of the main thread's lines that hold every sample (the stack that never moved), the last 14, innermost last. '' when
    nothing timed out or no rig copy is found."""
    run = os.environ.get("WHOOSH_SOCKET_DIR") or ""
    if "timed out" not in err or not run:
        return ""
    try:
        import subprocess
        out = subprocess.run(["ps", "-axo", "pid=,command="], capture_output=True, text=True).stdout
        pids = [l.split(None, 1)[0] for l in out.splitlines() if len(l.split(None, 1)) == 2 and l.split(None, 1)[1].startswith(os.path.join(run, "rig"))]
        if not pids:
            return " | no rig copy found to sample"
        keep = os.path.join(ROOT, "_captures", "hang-samples")
        os.makedirs(keep, exist_ok=True)
        path = os.path.join(keep, "%s-%s.sample" % (time.strftime("%Y%m%d-%H%M%S"), pids[0]))
        subprocess.run(["/usr/bin/sample", pids[0], "2", "-file", path], capture_output=True, text=True, timeout=30)
        text = open(path, encoding="utf-8", errors="replace").read() if os.path.exists(path) else ""
        return " | the copy's stack during the hang: %s; main thread's deepest frames (innermost last): %s" % (path, " > ".join(deepest_frames(text)) or "unread")
    except Exception as e:      # noqa: BLE001  the sample is evidence; its failure is said, never a crash of the check
        return " | sample failed: %s" % e


def deepest_frames(text, n=14):
    """The main thread's stuck chain from a `sample` report: the lines of the first thread block whose count equals the block's top count (a frame
    present in every sample), the last `n` of them, each cut to its symbol."""
    if "Thread_" not in text:
        return []
    block = text.split("Thread_", 1)[1].split("Thread_", 1)[0]
    rows = []
    for l in block.splitlines()[1:]:
        m = re.match(r"^[\s+!:|]*(\d+)\s+(.*)$", l)
        if m and m.group(2).split("  [")[0].strip():
            rows.append((int(m.group(1)), m.group(2).split("  [")[0].strip()))
    if not rows:
        return []
    top = max(c for c, _ in rows)
    return [f for c, f in rows if c == top][-n:]


def keys(ctl, chord):
    """One key press through the rig's key verb; returns the verb's reply (the window it built the key for, the route it took)."""
    try:
        r = ctl.send("keyevent", chord)
    except Unreadable as e:
        raise Unreadable("keyevent %r: %s%s" % (chord, e, hang_sample(str(e))))
    # a letter with no text focus runs as a one-key command of the app (X selects, Z undoes a mail action): never a typed letter (main76)
    want(r.get("focus") not in (None, "none", ""), "keyevent %r went to no text focus (reply focus %r, window %r): it would run as a command, not type" % (chord, r.get("focus"), r.get("window_id")))
    return r


def open_editor(ctl, k):
    """The page of a text on screen (and the Notes list for a note), then a click in the text: the cursor at its end."""
    try:
        ctl.click("mac-settings-window.open")
    except Unreadable:
        pass
    click(ctl, "page." + editors(ctl)[k]["page"])
    if k.startswith("note."):
        click(ctl, "prompt.notes")
    # The page was just opened: SwiftUI places its text view a moment later, and a click on a view with no window is refused (RepliesEditorDoors.cursor).
    # Retry the click for up to 3 s, then require the focus: a key typed before that would run as a command of the app (main76: voiceText).
    refused = ""
    for _ in range(15):
        try:
            click(ctl, "editor." + k)
            refused = ""
        except Red as e:
            refused = str(e)
        if not refused and editors(ctl)[k].get("focused") is True:
            return
        time.sleep(0.2)
    want(False, "%s: the click in the editor left focus %r (last refusal %r)" % (k, editors(ctl)[k].get("focused"), refused))


def eseam(ctl, k, what):
    try:
        ctl.click("%s.editor.%s.%s" % (P, k, what))
    except Unreadable as e:
        raise Unreadable("editor seam %s.%s not served: %s" % (k, what, e))


def pills(text):
    return len(re.findall(r"\[@[^\]]+\]", text))


def words(text):
    """The text without its pills: what is typed as words."""
    return re.sub(r"\[@[^\]]+\]", "", text)


@case("editor-one-click-no-edit-mode", "editor.prompt")
def _ed_click(ctl):
    want(not [d for d in controls(ctl) if d.startswith("doc.")], "the double-click edit mode is still listed")
    want(all(r["double_click"] is False for r in editors(ctl).values()), "an editor still asks for a double click")
    open_editor(ctl, "prompt")
    end = editors(ctl)["prompt"]["cursor"]
    eseam(ctl, "prompt", "cursor.start")
    want(editors(ctl)["prompt"]["cursor"] == 0, "a click at the start left the cursor at %r" % editors(ctl)["prompt"]["cursor"])
    eseam(ctl, "prompt", "cursor.middle")
    mid = editors(ctl)["prompt"]["cursor"]
    want(0 < mid < end, "a click in the middle put the cursor at %r of %r" % (mid, end))
    return True


@case("editor-pill-is-one-unit", "seam:editor")
def _ed_pill(ctl):
    open_editor(ctl, "prompt")
    keys(ctl, "space")   # « @ » opens the list after a space or a mark, never glued to a word
    n0 = pills(editors(ctl)["prompt"]["text"])
    eseam(ctl, "prompt", "at")
    e = editors(ctl)["prompt"]
    want(e["menu_open"] and len(e["menu_items"]) == 7, "« @ » opened %r with %r" % (e["menu_open"], e["menu_items"]))
    keys(ctl, "to")
    e = editors(ctl)["prompt"]
    want(e["menu_query"] == "to" and e["menu_items"] == ["tone"], "typing « to » left the list at %r" % e["menu_items"])
    keys(ctl, "Return")
    e = editors(ctl)["prompt"]
    want(pills(e["text"]) == n0 + 1 and not e["menu_open"] and "@to" not in words(e["text"]), "Return placed %r" % e["text"][-40:])
    keys(ctl, "Delete")   # the space after the pill
    keys(ctl, "Delete")   # the pill, whole
    e = editors(ctl)["prompt"]
    want(pills(e["text"]) == n0 and "@" not in words(e["text"]), "one Backspace left %r" % e["text"][-30:])
    eseam(ctl, "prompt", "paste-sample")
    after = editors(ctl)["prompt"]["text"]
    want(pills(after) == n0 + 1, "a paste of one « @tone » left %d pills (was %d, expected %d); words %r; text %r around the cursor %r" % (
        pills(after), n0, n0 + 1, "@tone" in words(after), after[-30:], editors(ctl)["prompt"]["cursor"]))
    eseam(ctl, "prompt", "at")
    keys(ctl, "zz")
    keys(ctl, "Escape")   # closes the list; the words stay
    keys(ctl, "space")
    e = editors(ctl)["prompt"]
    want("@zz" in e["unknown"] and e["line"] and "@zz" in e["line"], "an unknown « @zz » is not said: %r" % e["line"])
    return True


@case("editor-undo-redo-pills", "seam:editor")
def _ed_undo(ctl):
    open_editor(ctl, "prompt")
    t0, d0 = editors(ctl)["prompt"]["text"], editors(ctl)["prompt"]["undo_depth"]
    keys(ctl, "space")
    keys(ctl, "mot")
    e = editors(ctl)["prompt"]
    want(e["undo_depth"] > d0 and e["text"].endswith(" mot"), "typed words left undo depth %r (was %r)" % (e["undo_depth"], d0))
    keys(ctl, "⌘z")
    want(not editors(ctl)["prompt"]["text"].endswith("mot"), "⌘Z did not take the typed word back: %r" % editors(ctl)["prompt"]["text"][-12:])
    keys(ctl, "space")
    eseam(ctl, "prompt", "at")
    keys(ctl, "to")
    want(editors(ctl)["prompt"]["menu_open"], "the list did not open")
    keys(ctl, "⌘z")
    e = editors(ctl)["prompt"]
    want(not e["menu_open"] and not words(e["text"]).endswith("@to"), "⌘Z with the list open left %r, list open %r" % (e["text"][-10:], e["menu_open"]))
    for _ in range(4):
        keys(ctl, "⌘z")
    want(editors(ctl)["prompt"]["text"] == t0, "⌘Z all the way did not reach the saved text")
    keys(ctl, "space")   # « @ » opens the list after a space or a mark, never glued to a word
    n0 = pills(editors(ctl)["prompt"]["text"])
    eseam(ctl, "prompt", "at")
    keys(ctl, "Return")
    d1 = editors(ctl)["prompt"]["undo_depth"]
    want(pills(editors(ctl)["prompt"]["text"]) == n0 + 1, "no pill placed")
    keys(ctl, "⌘z")
    e = editors(ctl)["prompt"]
    want(pills(e["text"]) == n0 and e["undo_depth"] == d1 - 1, "⌘Z left %d pills, undo depth %r" % (pills(e["text"]), e["undo_depth"]))
    keys(ctl, "⇧⌘z")
    want(pills(editors(ctl)["prompt"]["text"]) == n0 + 1, "⇧⌘Z did not bring the pill back")
    return True


@case("editor-keys-go-to-the-editor-window", "seam:editor")
def _ed_route(ctl):
    """Round 8 (main76 probe): the key verb must build each key for the window that holds the focused text view. The reply says which window it
    used, so a key built for the main window is red here even when the letters still arrive; ⌘Z is then proved by its effect."""
    open_editor(ctl, "prompt")
    keys(ctl, "space")
    r = keys(ctl, "mot")
    want(str(r.get("window_id", "")).startswith("mac-settings"), "the key verb built « mot » for window %r (title %r), the editor is in the Settings window" % (r.get("window_id"), r.get("window")))
    want(r.get("focus") not in (None, "none", ""), "the key verb found no text focus (reply focus %r) while the editor holds it" % r.get("focus"))
    want(editors(ctl)["prompt"]["text"].endswith(" mot"), "« mot » did not reach the editor: %r" % editors(ctl)["prompt"]["text"][-12:])
    r = keys(ctl, "⌘z")
    want(str(r.get("window_id", "")).startswith("mac-settings"), "the key verb built ⌘Z for window %r, the editor is in the Settings window" % r.get("window_id"))
    want(r.get("route") in ("keyEquivalent", "sendEvent") and r.get("handled") is True, "⌘Z took route %r and was handled %r" % (r.get("route"), r.get("handled")))
    want(not editors(ctl)["prompt"]["text"].endswith("mot"), "⌘Z left the typed word: %r" % editors(ctl)["prompt"]["text"][-12:])
    return True


@case("editor-text-inside-card", "seam:editor")
def _ed_inside(ctl):
    for k in ["prompt", "voiceText"]:
        open_editor(ctl, k)
        for _ in range(15):   # the frame of the text is read once the view is laid out (main76: voiceText answered null on the first read)
            e = editors(ctl)[k]
            if e["text_frame"] is not None:
                break
            time.sleep(0.2)
        c, t = e["card_frame"], e["text_frame"]
        want(t is not None and c["w"] > 0, "%s: no frames (card %r, text %r)" % (k, c, t))
        inside = t["x"] >= c["x"] - 1 and t["y"] >= c["y"] - 1 and t["x"] + t["w"] <= c["x"] + c["w"] + 1 and t["y"] + t["h"] <= c["y"] + c["h"] + 1
        want(inside, "%s: the text %r is drawn outside its card %r" % (k, t, c))
        want(t["y"] - c["y"] <= 24, "%s: the text starts %.0f pt under the card's top" % (k, t["y"] - c["y"]))
    return True


@case("editor-note-line-at-once", "seam:editor")
def _ed_note(ctl):
    open_editor(ctl, "prompt")
    n0 = len(editors(ctl)["prompt"]["lines"])
    keys(ctl, "Return")
    keys(ctl, "//note")
    e = editors(ctl)["prompt"]
    want(e["dirty"] and len(e["lines"]) == n0 + 1 and e["lines"][-1] == "note", "the new « // » line reads %r of %d lines before any save" % (e["lines"][-1:], len(e["lines"])))
    return True


@case("editor-save-revert-draft", "seam:editor")
def _ed_save(ctl):
    open_editor(ctl, "voiceText")
    want(not editors(ctl)["voiceText"]["save_shown"], "Save shows with nothing to save")
    keys(ctl, "x")
    e = editors(ctl)["voiceText"]
    want(e["dirty"] and e["save_shown"] and e["line"], "a change shows %r / %r" % (e["save_shown"], e["line"]))
    keys(ctl, "⌘s")
    want(not editors(ctl)["voiceText"]["dirty"] and core(ctl, "reply/voice")["voice"]["tone_text"].endswith("x"), "⌘S did not save")
    keys(ctl, "y")
    keys(ctl, "Escape")
    e = editors(ctl)["voiceText"]
    want(e["confirm"] and e["dirty"], "the first Esc did not ask (confirm %r, dirty %r)" % (e["confirm"], e["dirty"]))
    keys(ctl, "Escape")
    e = editors(ctl)["voiceText"]
    want(not e["dirty"] and e["stored"].endswith("x"), "the second Esc did not go back to the saved text")
    keys(ctl, "Escape")
    e = editors(ctl)["voiceText"]
    want(e["stored"].endswith("x") and not e["confirm"], "Esc with no change changed something")
    # round 7: Esc with nothing to lose lets the editor go (RepliesEditorKeys, 0dff03e97: the window's own Escape is next), so the next letter needs a
    # click in the text first, as a person does; before, the case typed « z » at once and main93 sent it to no text focus (it ran as a command)
    open_editor(ctl, "voiceText")
    keys(ctl, "z")
    click(ctl, "page.auto")
    click(ctl, "page.voice")
    e = editors(ctl)["voiceText"]
    want(e["dirty"] and e["stored"].endswith("z") and e["line"], "leaving the page lost the draft: %r" % e["stored"][-10:])
    open_editor(ctl, "prompt")
    v = editors(ctl)["prompt"]["versions"]
    keys(ctl, "q")
    keys(ctl, "⌘s")
    want(editors(ctl)["prompt"]["versions"] == v + 1 and len(core(ctl, "reply/prompt/versions")) == v + 1, "a save of the prompt added no version")
    return True


@case("editor-preview-says-which", "seam:editor")
def _ed_preview(ctl):
    open_editor(ctl, "prompt")
    click(ctl, "prompt.preview")
    want(state(ctl)["prompt"]["preview_of_draft"] is False, "a clean prompt previewed as a draft")
    click(ctl, "popover.close")
    open_editor(ctl, "prompt")
    keys(ctl, "Return")   # a line of its own: the last line may be a « // » note the model never reads
    keys(ctl, "Bonjour")
    click(ctl, "prompt.preview")
    st = state(ctl)["prompt"]
    want(st["preview_of_draft"] is True and "Bonjour" in (st["preview"] or ""), "the preview read %r" % (st["preview"] or "")[-40:])
    return True


@case("editor-every-place-one-component", "editor.*")
def _ed_places(ctl):
    eds = editors(ctl)
    notes = ["note." + n["id"] for n in core(ctl, "reply/notes")]
    want(set(PLACES) | set(notes) <= set(eds), "places missing: %s" % sorted(set(PLACES) | set(notes) - set(eds)))
    want(all(r["component"] == "RepliesTextEditor" for r in eds.values()), "another editor: %r" % {k: r["component"] for k, r in eds.items()})
    for k in sorted(eds):
        open_editor(ctl, k)
        keys(ctl, "w")
        want(editors(ctl)[k]["dirty"], "%s: typing changed nothing" % k)
        click(ctl, "editor.%s.revert" % k)
        click(ctl, "editor.%s.revert" % k)
        want(not editors(ctl)[k]["dirty"], "%s: Revert left the change" % k)
        open_editor(ctl, k)
        keys(ctl, "w")
        n = len(core(ctl, "reply/notes"))
        click(ctl, "editor.%s.save" % k)
        want(not editors(ctl)[k]["dirty"] or k == "note.new", "%s: Save left it unsaved" % k)
        if k == "note.new":
            want(len(core(ctl, "reply/notes")) == n + 1, "the new note was not added")
    return True


@case("prompt-variables-place", "prompt.variable.*")
def _variables(ctl):
    doors = [x for x in controls(ctl) if x.startswith("prompt.variable.")]
    want(len(doors) == 7, "%d variables, the board says 7" % len(doors))
    open_editor(ctl, "prompt")
    for d in doors:
        n = pills(editors(ctl)["prompt"]["text"])
        click(ctl, "prompt.variables")
        click(ctl, d)
        want(pills(editors(ctl)["prompt"]["text"]) == n + 1 and state(ctl)["ui"]["popover"] is None, "%s placed no pill" % d)
    return True


@case("board-figures", )
def _figures(ctl):
    """The board's own figures through the fixture: 31 of 128, 12 of 31, 9 of 12 (the contract derives them; the pane draws them)."""
    c = core(ctl, "reply/counts")
    want((c["received"], c["after_entry"], c["after_ai"], c["after_exit"], c["drafts"]) == (128, 31, 12, 9, 9), "counts %r" % c)
    lines = state(ctl)["auto_lines"]
    want([l.replace(" ", " ").replace(" ", " ") for l in lines] in (["31 sur 128", "12 sur 31", "9 sur 12"], ["31 of 128", "12 of 31", "9 of 12"]),
         "the three lines read %r" % lines)
    return True


@case("every-control-has-a-case", )
def _coverage(ctl):
    pats = [p for _l, ds, _f in CASES for p in ds]
    missing = [d for d in controls(ctl) if not any(fnmatch.fnmatchcase(d, p) for p in pats)]
    want(not missing, "controls no case drives: %s" % ", ".join(missing))
    return True


# Every sentence of the six panes and the page comes from the catalogue (item d58d8ad5: "every string from the catalogue"): a `Text("...")` literal of
# Sources/Features/SettingsRepliesMac that holds a word of letters is a sentence typed in the code. A literal with only symbols, digits and interpolations
# (`Text("@" + name)`, `Text("\\(n) %")`) is not a word. Static: it reads the source files, so it needs no door and no app.
REPLIES_SRC = os.path.join(ROOT, "Sources", "Features", "SettingsRepliesMac")


def bare_text_literals(sources):
    """{file: [literal, ...]} for each `Text("literal")` whose literal, minus its \\(interpolations), still holds two letters in a row."""
    out = {}
    for name, text in sorted(sources.items()):
        for m in re.finditer(r'\bText\("((?:[^"\\\n]|\\.)*)"', text):
            lit = re.sub(r"\\\([^)\n]*\)", "", m.group(1))
            if re.search(r"[^\W\d_]{2}", lit):
                out.setdefault(name, []).append(m.group(1))
    return out


def replies_sources():
    return {n: open(os.path.join(REPLIES_SRC, n), encoding="utf-8").read() for n in sorted(os.listdir(REPLIES_SRC)) if n.endswith(".swift")}


@case("no-text-literal-holds-a-bare-sentence", )
def _bare_text(ctl):
    src = replies_sources()
    want(len(src) >= 30, "only %d source files read in %s" % (len(src), REPLIES_SRC))
    bare = bare_text_literals(src)
    want(not bare, "Text( literals that are not catalogue words: %s" % "; ".join("%s: %s" % (k, v) for k, v in bare.items()))
    return True


# ---------------------------------------------------------------- A124 step `a124-prompt-editor`: the finish line, clause by clause, ONE click each
# What the socket serves (RepliesEditorDoors.rows, RepliesEditorDoors.swift:72-90): per editor `focused`, `cursor`, `text` (each pill as « [@nom] », read from the
# live text storage: a pill typed over into words shows as « @nom »), `menu_open`/`menu_items`, `dirty`, `versions` (prompt only). The click door `editor.<key>`
# is ONE socket click (makeFirstResponder + cursor): a case here clicks it ONCE and never retries (open_editor above retries up to 15 times, so it cannot prove
# "no second click"). SERVED since gap-settings-checks (RepliesEditorDoors.mouseClick, RepliesTextView.mouseDown; docs/lanes/miss-prompt-editor.md was the finding): a REAL mouse click. Before it no verb posted a mouse event, and the row had no count of
# mouse-downs the view needed to take focus, so the case `a124-prompt-editor:real-mouse-click` was NOT JUDGED until the app served (an older build still says so, and stays NOT JUDGED)
#   seam  `mac-settings-replies.editor.<key>.mouse-click`  (RepliesEditorDoors.seams, same pattern as `.at`: post leftMouseDown + leftMouseUp to the text view's window)
#   field `mouse_downs_to_focus` in the editors row (RepliesEditorDoors.rows: mouse-downs the view took to become first responder, null when not focused by mouse)
# (`double_click` in the row is the constant false, not a measured fact.)
A124 = "a124-prompt-editor"
A124_MUTANTS = {
    "a124-two-clicks": ("%s:one-click-focuses" % A124, "a click only arms the editor, a second click focuses it"),
    "a124-letter-lost": ("%s:letter-after-one-click" % A124, "the first letter typed after a click is lost"),
    "a124-pill-to-text": ("%s:typing-beside-pill" % A124, "typing turns the pills into plain « @nom » words"),
    "a124-menu-not-opened": ("%s:at-opens-menu-escape-closes" % A124, "« @ » opens no variable menu"),
    "a124-undo-noop": ("%s:undo-takes-pill-back" % A124, "⌘Z does nothing"),
    "a124-save-no-version": ("%s:cmd-s-saves-a-version" % A124, "⌘S saves the text but adds no version"),
    "a124-second-click-long": ("%s:one-click-every-long-text" % A124, "every long text but the prompt needs a second click"),
    "a124-mouse-needs-two": ("%s:real-mouse-click" % A124, "a real mouse click needs two mouse-downs to focus"),
}


def goto_editor(ctl, k):
    """The page of a text on screen, then wait (no click) until the view is placed in its window."""
    try:
        ctl.click("mac-settings-window.open")
    except Unreadable:
        pass
    click(ctl, "page." + editors(ctl)[k]["page"])
    if k.startswith("note."):
        click(ctl, "prompt.notes")
    # placed AND still: the same frame on two reads 0.2 s apart. main196 (lead, 16 Nov): one click meant for note-2 gave the focus to note-1, once in three
    # builds; the notes had just been opened by the click above, and a frame read while they open is not where the text ends up.
    last = None
    for _ in range(25):
        e = editors(ctl)[k]
        frame = e.get("text_frame")
        if e.get("live") is True and frame is not None and frame == last:
            return
        last = frame
        time.sleep(0.2)
    want(False, "%s: the view was not placed and still in its window after 5 s (live %r, frame %r)" % (k, e.get("live"), e.get("text_frame")))


def one_click(ctl, k):
    """goto_editor, then ONE click in the text: the editor must hold the focus right after it (no retry, no second click)."""
    goto_editor(ctl, k)
    click(ctl, "editor." + k)
    e = editors(ctl)[k]
    if e.get("focused") is not True:   # round 23: which editor holds the focus, and where this one is (so a failure says what the click met)
        held = [x for x, r in editors(ctl).items() if r.get("focused") is True]
        want(False, "%s: after ONE click the editor holds focus %r (cursor %r); focus held by %s; this editor live %r, text frame %r"
             % (k, e.get("focused"), e.get("cursor"), held or "no editor", e.get("live"), e.get("text_frame")))
    want(isinstance(e.get("cursor"), int), "%s: after one click the cursor is %r" % (k, e.get("cursor")))


def a124(label):
    def deco(fn):
        def run_then_reset(ctl):
            try:
                return fn(ctl)
            finally:
                reset(ctl)   # the page's own reset door
        return case("%s:%s" % (A124, label), "seam:a124")(run_then_reset)
    return deco


@a124("one-click-focuses")
def _a124_click(ctl):
    one_click(ctl, "prompt")
    return True


@a124("letter-after-one-click")
def _a124_letter(ctl):
    one_click(ctl, "prompt")
    before = editors(ctl)["prompt"]["text"]
    keys(ctl, "k")
    e = editors(ctl)["prompt"]
    want(e["text"] == before + "k" and e["dirty"], "a letter typed after one click: text ends %r (was %r), dirty %r" % (e["text"][-12:], before[-12:], e["dirty"]))
    return True


@a124("typing-beside-pill")
def _a124_pill(ctl):
    one_click(ctl, "prompt")
    click(ctl, "prompt.variables")
    click(ctl, "prompt.variable.tone")
    e = editors(ctl)["prompt"]
    n = pills(e["text"])
    want(n >= 1 and "[@tone]" in e["text"], "the variable menu placed no « tone » pill: %r" % e["text"][-30:])
    keys(ctl, "ab")   # right after the pill
    e = editors(ctl)["prompt"]
    want(pills(e["text"]) == n and "[@tone]" in e["text"] and words(e["text"]).endswith("ab"), "typing after a pill: %d pills (was %d), text ends %r" % (pills(e["text"]), n, e["text"][-24:]))
    eseam(ctl, "prompt", "cursor.middle")
    keys(ctl, "c")   # in the middle of the text
    e = editors(ctl)["prompt"]
    want(pills(e["text"]) == n and "[@tone]" in e["text"], "typing in the middle: %d pills (was %d)" % (pills(e["text"]), n))
    return True


@a124("at-opens-menu-escape-closes")
def _a124_menu(ctl):
    one_click(ctl, "prompt")
    keys(ctl, "space")
    eseam(ctl, "prompt", "at")   # keyevent has no code for « @ »
    e = editors(ctl)["prompt"]
    want(e["menu_open"] is True and len(e["menu_items"]) == 7, "« @ » left the menu open %r with %r" % (e["menu_open"], e["menu_items"]))
    keys(ctl, "Escape")
    e = editors(ctl)["prompt"]
    want(e["menu_open"] is False, "Escape left the menu open")
    return True


@a124("undo-takes-pill-back")
def _a124_undo(ctl):
    one_click(ctl, "prompt")
    t0 = editors(ctl)["prompt"]["text"]
    click(ctl, "prompt.variables")
    click(ctl, "prompt.variable.tone")
    want(pills(editors(ctl)["prompt"]["text"]) == pills(t0) + 1, "the variable menu placed no pill")
    keys(ctl, "⌘z")
    e = editors(ctl)["prompt"]
    want(pills(e["text"]) == pills(t0) and e["text"] == t0, "⌘Z after a pill from the menu left %r (before: %r)" % (e["text"][-24:], t0[-24:]))
    keys(ctl, "space")
    eseam(ctl, "prompt", "at")
    keys(ctl, "Return")   # the pill from the « @ » list
    want(pills(editors(ctl)["prompt"]["text"]) == pills(t0) + 1, "the « @ » list placed no pill")
    keys(ctl, "⌘z")
    want(pills(editors(ctl)["prompt"]["text"]) == pills(t0), "⌘Z after a pill from the « @ » list left the pill")
    return True


@a124("cmd-s-saves-a-version")
def _a124_save(ctl):
    one_click(ctl, "prompt")
    v = editors(ctl)["prompt"]["versions"]
    keys(ctl, "q")
    want(editors(ctl)["prompt"]["dirty"] is True, "a typed letter left the prompt clean")
    keys(ctl, "⌘s")
    e = editors(ctl)["prompt"]
    n = len(core(ctl, "reply/prompt/versions"))
    want(e["versions"] == v + 1 and n == v + 1 and e["dirty"] is False, "⌘S: versions %r -> %r (core %r), dirty %r" % (v, e["versions"], n, e["dirty"]))
    return True


@a124("one-click-every-long-text")
def _a124_every(ctl):
    keys_here = sorted(editors(ctl))
    want({"prompt", "retrievalPrompt", "voiceText", "aiDescription"} <= set(keys_here), "long texts missing: %r" % keys_here)
    for k in keys_here:
        reset(ctl)
        one_click(ctl, k)
        before = editors(ctl)[k]["text"]
        keys(ctl, "w")
        want(editors(ctl)[k]["text"] == before + "w", "%s: a letter after one click did not arrive: %r" % (k, editors(ctl)[k]["text"][-12:]))
    return True


@a124("real-mouse-click")
def _a124_mouse(ctl):
    goto_editor(ctl, "prompt")
    if "mouse_downs_to_focus" not in editors(ctl)["prompt"]:
        raise Unreadable("NOT JUDGED: the editors row serves no `mouse_downs_to_focus` (finding: miss-prompt-editor.md)")
    try:
        ctl.click("%s.editor.prompt.mouse-click" % P)
    except Unreadable as e:
        raise Unreadable("NOT JUDGED: seam editor.prompt.mouse-click not served: %s" % e)
    e = editors(ctl)["prompt"]
    want(e["focused"] is True and e["mouse_downs_to_focus"] == 1, "one real mouse click: focused %r after %r mouse-downs (the view received %s mouse-downs in all)"
         % (e["focused"], e["mouse_downs_to_focus"], e.get("mouse_downs_seen", "an unserved number of")))
    return True


# ---------------------------------------------------------------- FakeApp: the stand-in for --selftest ONLY
import copy  # noqa: E402
import re  # noqa: E402
import shutil  # noqa: E402
import socketserver  # noqa: E402
import tempfile  # noqa: E402
import threading  # noqa: E402

VARS = ["message", "thread", "instruction", "excerpts", "calendar", "tone", "notes"]
# test seams: clickable, never listed as controls (the system's own Cancel of the folder chooser)
SEAMS = {"sources.picker.cancel"}
FAKE_HANDOVER_S, FAKE_INDEX_S = 1.5, 1.0   # the stand-in holds the model 1.5 s after the first read of mac-main.background, then indexes in 1 s
HANDOVER_MUTANTS = {"progress-stuck-after-handover": "the folders' progress stays where it was after the model is handed over (the engine never indexes with it)",
                    "tile-keeps-old-progress": "the core reads up to date but the tile still draws the progress it had before the hand-over (the pane does not re-read)"}


def _rule(facts=None, match="all", rows=None):
    f = {k: "any" for k in FACTS}
    f.update(facts or {})
    return {"match": match, "facts": f, "rows": rows or []}


def _folder(acc, name, on, n, prog):
    return {"source": {"kind": "mail_folder", "account_id": acc, "folder": name}, "enabled": on, "item_count": n, "progress": {"state": prog}}


class FakeEditor:
    """One Replies editor as the build should behave: tokens (a character, or ("P", id) for a pill), the cursor, an undo stack, the @ list."""

    def __init__(self, app, key, saved):
        self.app, self.key, self.saved = app, key, saved
        self.tokens, self.cursor, self.undo, self.redo = self.parse(saved), 0, [], []
        self.menu, self.closed_at, self.index, self.confirm, self.draft = None, None, 0, False, False
        self.step_open = True   # the next typed letter starts a new undo step (a word ends at a space or a mark)

    @staticmethod
    def parse(stored):
        out, rest = [], stored
        while rest:
            m = re.match(r"\{\{(\w+)\}\}", rest)
            if m:
                out.append(("P", m.group(1)))
                rest = rest[m.end():]
            else:
                out.append(rest[0])
                rest = rest[1:]
        return out

    def stored(self, tokens=None):
        return "".join("{{%s}}" % t[1] if isinstance(t, tuple) else t for t in (self.tokens if tokens is None else tokens))

    def dirty(self):
        return self.stored() != self.saved

    def snapshot(self, pill=False):
        self.undo.append((list(self.tokens), self.cursor, pill))
        self.redo = []

    def insert(self, toks, pill=False, replace=0, typed=False):
        if not typed:
            self.snapshot(pill)
            self.step_open = True
        elif self.step_open and "typing-not-undoable" not in self.app.m:
            self.snapshot()
            self.step_open = False
        if typed and not (isinstance(toks[0], str) and toks[0].isalnum()):
            self.step_open = True
        start = self.cursor - replace
        self.tokens[start:self.cursor] = toks
        self.cursor = start + len(toks)
        if typed and "a124-pill-to-text" in self.app.m:   # mutant: the pills turn into plain words
            new, cur = [], self.cursor
            for i, t in enumerate(self.tokens):
                flat = list("@" + t[1]) if isinstance(t, tuple) else [t]
                new += flat
                cur += len(flat) - 1 if i < self.cursor else 0
            self.tokens, self.cursor = new, cur
        self.changed()

    def changed(self):
        self.confirm = False
        self.app.focused = self.key
        self.find_at()

    def find_at(self):
        i = self.cursor
        while i > 0 and isinstance(self.tokens[i - 1], str) and self.tokens[i - 1].isalnum():
            i -= 1
        ok = "a124-menu-not-opened" not in self.app.m and i > 0 and self.tokens[i - 1] == "@" and i - 1 != self.closed_at and self.key in ("prompt", "retrievalPrompt")
        if ok and i - 1 > 0 and isinstance(self.tokens[i - 2], str) and self.tokens[i - 2].isalnum():
            ok = False
        self.menu = (i - 1, "".join(self.tokens[i:self.cursor])) if ok else None
        if not ok:
            self.index = 0

    def items(self):
        if not self.menu or not self.menu[1] or "menu-no-filter" in self.app.m:
            return list(VARS)
        return [v for v in VARS if v.startswith(self.menu[1])]

    def key_press(self, k):
        app = self.app
        if self.menu is not None and k in ("Return", "Tab", "Escape", "Down", "Up"):
            if k in ("Return", "Tab"):
                its = self.items()
                if its:
                    at, q = self.menu
                    self.insert([("P", its[self.index]), " "], pill=True, replace=1 + len(q))
                return
            if k == "Escape":
                self.closed_at, self.menu = self.menu[0], None
                return
            self.index = (self.index + (1 if k == "Down" else -1)) % max(1, len(self.items()))
            return
        if k == "Return":
            return self.insert(["\n"])
        if k == "space":
            return self.insert([" "], typed=True)
        if k == "Delete":
            if self.cursor == 0:
                return
            t = self.tokens[self.cursor - 1]
            if isinstance(t, tuple) and "pill-backspace-char" in app.m:
                word = list("@" + t[1])[:-1]
                self.snapshot()
                self.tokens[self.cursor - 1:self.cursor] = word
                self.cursor += len(word) - 1
                return self.changed()
            self.snapshot(isinstance(t, tuple))
            del self.tokens[self.cursor - 1]
            self.cursor -= 1
            return self.changed()
        if k == "Escape":
            if not self.dirty():
                if "esc-clean-destroys" in app.m:
                    self.tokens = []
                self.confirm = False
                if self.confirm is False and app.focused == self.key:
                    app.focused = None      # nothing to lose: the editor lets go (RepliesEditorKeys, the lead's main92 round 0dff03e97)
                return
            if self.confirm or "esc-reverts-at-once" in app.m:
                return self.revert()
            self.confirm = True
            return
        if k == "⌘z":
            if "a124-undo-noop" in app.m:
                return
            if self.menu is not None and "undo-keeps-menu" not in self.app.m:
                self.closed_at, self.menu = self.menu[0], None
            self.step_open = True
            if self.undo and not ("undo-skips-pill" in app.m and self.undo[-1][2]):
                toks, cur, pill = self.undo.pop()
                self.redo.append((list(self.tokens), self.cursor, pill))
                self.tokens, self.cursor = toks, cur
                self.find_at()
            return
        if k == "⇧⌘z":
            if self.redo:
                toks, cur, pill = self.redo.pop()
                self.undo.append((list(self.tokens), self.cursor, pill))
                self.tokens, self.cursor = toks, cur
                self.find_at()
            return
        if k == "⌘s":
            return app.ed_save(self.key)
        for ch in k:   # words, typed one key at a time (keyevent has no « @ »)
            if ch != "@":
                self.insert([" " if k == "space" else ch], typed=True)

    def revert(self):
        self.tokens, self.cursor, self.confirm, self.menu = self.parse(self.saved), min(self.cursor, len(self.parse(self.saved))), False, None

    def revert_click(self):
        if self.confirm:
            self.revert()
        elif self.dirty():
            self.confirm = True

    def unknown(self):
        if "unknown-silent" in self.app.m or self.key not in ("prompt", "retrievalPrompt"):
            return []
        return sorted(set(re.findall(r"(?<![^\W_])@\w+", self.stored())))

    def row(self):
        d, unknown = self.dirty(), self.unknown()
        line = "Esc again" if self.confirm else ("Not a variable: " + ", ".join(unknown) if unknown else ("Not saved" if d else None))
        text = "".join("[@%s]" % t[1] if isinstance(t, tuple) else t for t in self.tokens)
        lines_of = self.saved if "note-late" in self.app.m else self.stored()
        kinds = ["heading" if l.startswith("## ") else ("note" if l.startswith("//") else "body") for l in lines_of.split("\n")]
        comp = "TextEditor" if "one-place-other-component" in self.app.m and self.key == "aiDescription" else "RepliesTextEditor"
        return {"ax": "%s.editor.%s" % (P, self.key), "key": self.key, "page": "prompt" if self.key.startswith("note.") else
                {"prompt": "prompt", "retrievalPrompt": "retrieval", "voiceText": "voice", "aiDescription": "filter"}[self.key],
                "component": comp, "live": True, "text": text, "stored": self.stored(), "lines": kinds, "cursor": self.cursor,
                "undo_depth": len(self.undo), "dirty": d, "save_shown": d or "save-shown-clean" in self.app.m, "line": line, "unknown": unknown,
                "confirm": self.confirm, "focused": self.app.focused == self.key, "menu_open": self.menu is not None, "menu_query": self.menu[1] if self.menu else None,
                "menu_items": self.items() if self.menu else [], "menu_index": self.index, "last_key_ms": 0.1,
                "versions": len(self.app.prompts) if self.key == "prompt" else None, "double_click": False,
                "mouse_downs_to_focus": self.app.mouse_focus if self.app.focused == self.key else None,
                "card_frame": {"x": 300, "y": 200, "w": 600, "h": 240},
                "text_frame": {"x": 311, "y": 500 if "text-outside-card" in self.app.m else 212, "w": 560, "h": 200}}


class FakeEditorsMixin:
    """The editor half of FakeApp: places, saves, seams, the preview."""

    def places(self):
        return ["prompt", "retrievalPrompt", "voiceText", "aiDescription", "note.new"] + ["note." + n["id"] for n in self.notes]

    def follow(self):
        """RepliesStore.followLanguage on the stand-in: an untouched shipped default takes the app language's catalogue text."""
        if "defaults-stay-english" in self.m:
            return
        lang = getattr(self, "lang", "en")

        def here(key, stored):
            texts, st = set(_SHIPPED.get(key, {}).values()) | {key}, (stored or "").strip()
            mine = _SHIPPED.get(key, {}).get(lang, key)
            if (st in texts or ("typed-text-replaced" in self.m and st)) and st != mine:
                return mine
            return None
        r = self.retrieval
        t = here(RETR_KEY, r["retrieval_prompt"])
        if t:
            r["retrieval_prompt"] = t
        t = here(FILTER_KEY, r["ai_filter_question"]) if r["ai_filter_question"] else None
        if t:
            r["ai_filter_question"] = t
        t = here(PROMPT_KEY, [p for p in self.prompts if p["current"]][0]["text"])
        if t:
            self._save(t)

    def saved_text(self, k):
        if k == "prompt":
            return [p for p in self.prompts if p["current"]][0]["text"]
        if k == "retrievalPrompt":
            return self.retrieval["retrieval_prompt"]
        if k == "voiceText":
            return self.voice["tone_text"]
        if k == "aiDescription":
            d = self.filter["ai"]["description"]
            lang = getattr(self, "lang", "en")
            if "typed-description-replaced" in self.m or (d.strip() in set(_DESC.values()) | {DESC_KEY} and "description-not-localized" not in self.m):
                return _DESC.get(lang, _DESC["en"])
            return d
        if k == "note.new":
            return ""
        return [n for n in self.notes if "note." + n["id"] == k][0]["text"]

    def ed(self, k):
        if k not in self.eds:
            self.eds[k] = FakeEditor(self, k, self.saved_text(k))
        return self.eds[k]

    def editor_rows(self):
        return [self.ed(k).row() for k in self.places()]

    def focus_on(self, k, at=None):
        if "click-without-window" in self.m:
            return
        if ("a124-two-clicks" in self.m or "a124-second-click-long" in self.m and k != "prompt") and k not in self.armed:
            self.armed.add(k)   # mutant: the first click only arms the editor
            return
        e = self.ed(k)
        self.focused = k
        self.fresh = True
        if "cursor-ignored" not in self.m or at is None:
            e.cursor = len(e.tokens) if at is None else at
        e.find_at()

    def go_page(self, p):
        self.ui["page"] = p
        if "draft-lost-on-leave" in self.m:
            for e in self.eds.values():
                e.revert()

    def keyevent(self, chord):
        """The verb's reply: the window it built the key for and the route. A key built for the main window never reaches an editor of the Settings window."""
        if "keyevent-main-window-text-arrives" in self.m:   # the old verb: the letters arrive (AppKit hands them to the key window), the window named is the main one
            self.key_to_editor(chord)
            return {"window": "Whoosh", "window_id": "mac-main", "focus": "RepliesTextView", "route": "sendEvent", "handled": True}
        if "keyevent-main-window" in self.m:
            return {"window": "Whoosh", "window_id": "mac-main", "focus": "none", "route": "sendEvent", "handled": False}
        had = self.focused is not None      # the verb names the focus the key was sent to (round 7: a clean Esc lets the editor go AFTER it arrived)
        self.key_to_editor(chord)
        return {"window": "Whoosh", "window_id": "mac-settings", "focus": "RepliesTextView" if had else "none",
                "route": "keyEquivalent" if chord.startswith(("⌘", "⇧⌘")) else "sendEvent", "handled": True}

    def key_to_editor(self, chord):
        # the shell's key monitor: these chords reach the editor only through the feature's own bindings (RepliesEditorKeys)
        if "keys-eaten-by-shell" in self.m and chord in ("Escape", "⌘s", "⌘Return"):
            return
        if "a124-letter-lost" in self.m and self.fresh and len(chord) == 1 and chord.isalpha():
            self.fresh = False   # mutant: the first letter typed after a click is lost
            return
        if self.focused is not None:
            self.ed(self.focused).key_press(chord)

    def ed_seam(self, k, what):
        e = self.ed(k)
        if what == "mouse-click":   # a real mouse click as the app serves it
            self.downs += 1
            if "a124-mouse-needs-two" not in self.m or self.downs >= 2:
                self.focused, e.cursor, self.mouse_focus = k, len(e.tokens), self.downs
            return
        if what.startswith("cursor."):
            n = len(e.tokens)
            return self.focus_on(k, {"cursor.start": 0, "cursor.middle": n // 2, "cursor.end": n}[what])
        self.focused = k
        if what == "at":
            return e.insert(["@"])
        if "paste-stays-text" in self.m:
            return e.insert(list("Écris comme @tone."))
        e.insert(list("Écris comme ") + [("P", "tone")] + ["."], pill=True)

    def ed_save(self, k):
        e = self.ed(k)
        if not e.dirty():
            return
        text = e.stored()
        if k == "prompt":
            if "save-no-version" in self.m or "a124-save-no-version" in self.m:
                [p for p in self.prompts if p["current"]][0]["text"] = text
            else:
                self._save(text)
        elif k == "retrievalPrompt":
            self.retrieval["retrieval_prompt"] = text
        elif k == "voiceText":
            self.voice["tone_text"] = text
        elif k == "aiDescription":
            self.filter["ai"]["description"] = text
        elif k == "note.new":
            self.notes.append({"id": "note-%d" % (len(self.notes) + 20), "title": text[:40], "text": text})
            e.tokens, e.cursor = [], 0
            return
        else:
            [n for n in self.notes if "note." + n["id"] == k][0]["text"] = text
        e.saved = text
        e.confirm = False

    def _preview(self):
        e = self.ed("prompt")
        draft = e.dirty() and "preview-reads-saved" not in self.m
        text = e.stored() if draft else self.saved_text("prompt")
        self.ui.update(popover="preview", preview=re.sub(r"\{\{(\w+)\}\}", "…", "\n".join(l for l in text.split("\n") if not l.startswith("//"))),
                       preview_of_draft=e.dirty())


class FakeApp(FakeEditorsMixin):
    """The app on the wire as the build should behave; `noop:<door>` makes one door do nothing, `extra-control` lists a door no case drives."""

    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.reset()

    def reset(self):
        self.use = {"auto": {"enabled": True, "tone_id": "voice", "match_thread": True}, "on_demand": {"enabled": True, "tone_id": "voice", "match_thread": True}}
        self.filter = {"scope": {"all": False, "label_ids": ["needs-reply", "lbl-clients"]}, "entry": {"enabled": True, "rule": _rule({"mailing_list": "no"})},
                       "ai": {"enabled": True, "subject": True, "body": True, "rest_of_thread": True, "attachments": False, "description": DESC_KEY},
                       "exit": {"enabled": True, "rule": _rule({"replied_by_me": "no"})}}
        self.on_demand = {"instruction_before": True, "change_after": True}
        self.labels = ["needs-reply", "lbl-clients", "lbl-updates"]
        self.accounts = {"acc-1": [_folder("acc-1", f, on, n, p) for f, on, n, p in [("INBOX", True, 9204, "up_to_date"), ("Sent", True, 4812, "up_to_date"), ("Clients", True, 2960, "syncing"),
                                                                                   ("Clients/Lorvan Hotels", True, 1120, "syncing"), ("Clients/Oranta", False, 1840, "not_started"), ("Newsletters", False, 6310, "not_started")]],
                         "acc-2": [_folder("acc-2", f, on, 10, "up_to_date") for f, on in [("INBOX", True), ("Sent", True), ("Family", False), ("Invoices", False), ("Archive", False)]]}
        self.local = [{"source": {"kind": "local_folder", "path": p}, "enabled": True, "item_count": n, "progress": {"state": "up_to_date"}} for p, n in [("~/Documents/Contrats", 214), ("~/Documents/Propositions", 86)]]
        self.calendars = [{"calendar_id": "cal-work", "name": "Travail", "enabled": True}, {"calendar_id": "cal-personal", "name": "Personnel", "enabled": False}]
        self.retrieval = {"retrieval_prompt": RETR_KEY, "excerpts_per_question": 5, "excerpt_size": "paragraph",
                          "attached_fields": {"date": True, "sender": True, "folder": True, "subject": True}, "keep_n": 6, "clash_rule": "keep_both_dated",
                          "ai_filter_question": FILTER_KEY, "scope": "this_account"}
        self.voice = {"tone_text": "Phrases courtes.", "sample_size": 100, "analysed_at_ms": 1793696400000, "same_person_examples": 3}
        self.tones = [{"id": "tone-formal", "name": "Formel"}, {"id": "tone-brief", "name": "Bref"}]
        self.prompts = [{"version": v, "text": t, "current": v == 3} for v, t in [(1, PROMPT_KEY), (2, "Réponds à {{message}} avec {{excerpts}}."),
                                                                                  (3, "## Ce que tu sais\nTu réponds à {{message}}.\n// une note")]]
        self.notes = [{"id": "note-1", "title": "Prix", "text": "Ne jamais annoncer un prix."}, {"id": "note-2", "title": "Disponibilités", "text": "Mardi et jeudi."}]
        self.ui = {"page": "auto", "popover": None, "editing": None, "edit_text": "", "open_accounts": ["acc-1"], "picker_open": False, "trial_kind": None,
                   "picked_sample": None, "preview": None}
        self.asks, self.counter = 0, 0
        self.armed, self.fresh, self.downs, self.mouse_focus = set(), False, 0, None
        self.eds, self.focused = {}, None   # the editors (A124): FakeEditor by key, the one holding the keys
        self.wishes = {}   # the pane's memory of what it asked for a gone folder (RepliesStore.goneWishes)
        self.handover_t = None   # --handover: the moment the stand-in hands the model over (set at the first read of mac-main.background)

    # ---- the core's reads
    def total(self):
        return sum(f["enabled"] or ("count-holds-gone" in self.m and f.get("gone", False)) for fs in self.accounts.values() for f in fs) + sum(x["enabled"] for x in self.local) + sum(c["enabled"] for c in self.calendars)

    def core(self, path):
        if path == "system-ui/save-panel":
            return self.asks
        if path.startswith("reply/use/"):
            return self.use[path.rsplit("/", 1)[1]]
        cur = [p for p in self.prompts if p["current"]][0]
        table = {"reply/filter": lambda: self.filter, "reply/on_demand": lambda: self.on_demand, "reply/retrieval": lambda: self.retrieval,
                 "reply/counts": lambda: {"days": 7, "received": 128, "after_entry": 31, "after_ai": 12, "after_exit": 9, "drafts": 9},
                 "reply/sources": lambda: {"accounts": [] if "no-account" in self.m else [self._account(a, fs) for a, fs in self.accounts.items()], "local_folders": self.local,
                                           "calendars": self.calendars, "scope": self.retrieval["scope"], "enabled_total": self.total()},
                 "reply/voice": lambda: {"voice": self.voice, "analysis": {"state": "idle"}, "tones": self.tones}, "reply/notes": lambda: self.notes,
                 "reply/prompt/current": lambda: cur, "reply/prompt/versions": lambda: sorted(self.prompts, key=lambda p: -p["version"]),
                 "reply/prompt/preview": lambda: "\n".join(l for l in re.sub(r"\{\{(\w+)\}\}", "…", cur["text"]).split("\n") if not l.startswith("//"))}
        if path not in table:
            raise KeyError("no core value at %s" % path)
        return copy.deepcopy(table[path]())

    def _row(self, f):
        """A folder as the core reads it (delta 24: gone = off, progress gone, last count kept)."""
        out = {k: v for k, v in f.items() if k not in ("gone", "owner")}
        out["owner_enabled"] = f["owner"] if f.get("gone") else None
        if "all-up-to-date" in self.m and not f.get("gone") and f["progress"]["state"] == "syncing":
            out["progress"] = {"state": "up_to_date"}
        if not f.get("gone") and f["progress"]["state"] == "syncing" and self.indexed():
            out["progress"] = {"state": "up_to_date"}   # the engine indexes the held-back folder once it has the model
        if f.get("gone"):
            out["progress"] = {"state": "gone"}
            out["enabled"] = "gone-stays-on" in self.m and f["owner"]
            out["item_count"] = 0 if "gone-loses-count" in self.m else f["item_count"]
        return out

    def indexed(self):
        return (self.handover_t is not None and time.time() >= self.handover_t + FAKE_INDEX_S and "progress-stuck-after-handover" not in self.m)

    def background(self):
        if self.handover_t is None:
            self.handover_t = time.time() + FAKE_HANDOVER_S
        return [{"embedder": "handed" if time.time() >= self.handover_t else "held"}]

    def _account(self, a, fs):
        rows = [self._row(f) for f in fs]
        on = sum(r["enabled"] for r in rows) + ("count-holds-gone" in self.m and sum(1 for f in fs if f.get("gone")))
        return {"account_id": a, "folders": rows, "enabled_folders": on, "total_folders": len(fs)}

    def gone_seam(self, account, i, gone):
        f = self.accounts[account][i]
        if gone and not f.get("gone"):
            f.update(gone=True, owner=f["enabled"], enabled=False)
        elif not gone and f.get("gone"):
            f.update(gone=False, enabled=True if "back-forgets-choice" in self.m else f["owner"])
            f.pop("owner", None)
            self.wishes.pop((account, f["source"]["folder"]), None)

    def tile(self, account, i):
        f = self.accounts[account][i]
        if not f.get("gone"):
            f["enabled"] = not f["enabled"]
            return
        key = (account, f["source"]["folder"])
        if "pane-keeps-own-memory" in self.m:
            wish = not self.wishes.get(key, False)
            self.wishes[key] = wish
        else:
            wish = not f["owner"]
        f["owner"] = wish
        if "gone-switch-enables" in self.m:
            f.update(gone=False, enabled=True)

    # ---- the rows
    def state(self):
        f, ids = self.filter, self.filter["scope"]["label_ids"]
        line = ", ".join(n for k, n in [("date", "Date"), ("sender", "Expéditeur"), ("folder", "Dossier"), ("subject", "Sujet")] if self.retrieval["attached_fields"][k])
        return {"auto_enabled": self.use["auto"]["enabled"], "auto_tone": self.use["auto"]["tone_id"], "auto_match_thread": self.use["auto"]["match_thread"],
                "on_demand_tone": self.use["on_demand"]["tone_id"], "on_demand_match_thread": self.use["on_demand"]["match_thread"],
                "instruction_before": self.on_demand["instruction_before"], "change_after": self.on_demand["change_after"],
                "scope_all": f["scope"]["all"], "scope_label_ids": list(ids), "entry_on": f["entry"]["enabled"], "ai_on": f["ai"]["enabled"], "exit_on": f["exit"]["enabled"],
                "entry_match": f["entry"]["rule"]["match"], "exit_match": f["exit"]["rule"]["match"], "entry_facts": dict(f["entry"]["rule"]["facts"]),
                "exit_facts": dict(f["exit"]["rule"]["facts"]), "sender": next((r["words"] for r in f["entry"]["rule"]["rows"] if r["kind"] == "from"), ""),
                "reads": {"subject": f["ai"]["subject"], "body": f["ai"]["body"], "rest": f["ai"]["rest_of_thread"], "attachments": f["ai"]["attachments"]},
                "description": f["ai"]["description"], "auto_lines": ["31 sur 128", "12 sur 31", "9 sur 12"], "search_scope": self.retrieval["scope"],
                "sources_line": "%d activées, réglées par compte" % self.total(),
                "retrieval": {"fields": line, "ai_filter": self.retrieval["ai_filter_question"]},
                "voice": {"picked_sample": self.ui["picked_sample"], "examples": self.voice["same_person_examples"], "tones": [t["name"] for t in self.tones]},
                "prompt": {"version": [p for p in self.prompts if p["current"]][0]["version"], "notes": [n["title"] for n in self.notes], "preview": self.ui["preview"],
                           "preview_of_draft": self.ui.get("preview_of_draft", False)},
                "ui": {k: copy.deepcopy(self.ui[k]) for k in ["page", "popover", "editing", "edit_text", "open_accounts", "picker_open", "trial_kind"]}}

    def sources_rows(self):
        out = []
        if "card-reopens-on-read" in self.m and not self.ui["open_accounts"]:
            self.ui["open_accounts"].append(next(iter(self.accounts)))
        for a, fs in self.accounts.items():
            out.append({"ax": "%s.sources.account.%s" % (P, a), "kind": "account", "open": a in self.ui["open_accounts"]})
            for i, f in enumerate(fs):
                row, gone = self._row(f), f.get("gone", False)
                drawn = (f if "tile-keeps-old-progress" in self.m else row)["progress"]["state"]
                out.append({"ax": "%s.sources.account.%s.folder.%d" % (P, a, i), "kind": "mail_folder", "enabled": row["enabled"], "gone": gone,
                            "progress": {"syncing": "62 %", "up_to_date": "à jour", "gone": "N’existe plus sur le serveur"}.get(drawn, ""),
                            "line": "" if not gone or "gone-no-line" in self.m else "N’existe plus sur le serveur",
                            "wish": (self.wishes.get((a, f["source"]["folder"])) if "pane-keeps-own-memory" in self.m else f["owner"]) if gone else None})
        out += [{"ax": "%s.sources.local.%d" % (P, i), "enabled": x["enabled"]} for i, x in enumerate(self.local)]
        out += [{"ax": "%s.sources.calendar.%s" % (P, c["calendar_id"]), "enabled": c["enabled"]} for c in self.calendars]
        return out

    # ---- the doors (the same list RepliesDoors.swift builds)
    def doors(self):
        d, u, f, r, ui = {}, self.use, self.filter, self.retrieval, self.ui

        def tog(obj, key):
            return lambda: obj.__setitem__(key, not obj[key])

        def setv(obj, key, v):
            return lambda: obj.__setitem__(key, v)
        for p in ["auto", "filter", "on-demand", "sources", "retrieval", "voice", "prompt"]:
            d["page." + p] = (lambda p=p: self.go_page(p))
        d["popover.close"] = setv(ui, "popover", None)
        d["auto.enabled"] = tog(u["auto"], "enabled")
        d["auto.try"] = lambda: ui.update(popover="trial", trial_kind="auto")
        d["retrieval.try"] = lambda: ui.update(popover="trial", trial_kind="on_demand")
        for k in ["entry", "ai", "exit"]:
            d["auto.step." + k] = setv(ui, "page", "filter")
            d["filter.%s.switch" % k] = tog(f[k], "enabled")
        for key, use in [("auto", "auto"), ("on-demand", "on_demand")]:
            d[key + ".sources"] = setv(ui, "page", "sources")
            d[key + ".prompt"] = setv(ui, "page", "prompt")
            d[key + ".match-thread"] = tog(u[use], "match_thread")
            for t in ["voice"] + [t["id"] for t in self.tones]:
                d["%s.tone.%s" % (key, t)] = setv(u[use], "tone_id", t)
        d["on-demand.instruction-before"] = tog(self.on_demand, "instruction_before")
        d["on-demand.change-after"] = tog(self.on_demand, "change_after")
        d["filter.scope.all"] = lambda: f.__setitem__("scope", {"all": True, "label_ids": []})

        def label(l):
            def go():
                ids = list(f["scope"]["label_ids"])
                ids.remove(l) if l in ids else ids.append(l)
                f["scope"] = {"all": False, "label_ids": ids}
            return go
        for l in self.labels:
            d["filter.scope.label." + l] = label(l)
        for k in ["entry", "exit"]:
            for m in ["all", "any"]:
                d["filter.%s.match.%s" % (k, m)] = setv(f[k]["rule"], "match", m)
            for fact in FACTS:
                for s in ["any", "yes", "no"]:
                    d["filter.%s.fact.%s.%s" % (k, fact, s)] = setv(f[k]["rule"]["facts"], fact, s)
        d["filter.entry.sender"] = lambda: f["entry"]["rule"].__setitem__("rows", [{"kind": "from", "words": SENDER}])
        d["filter.entry.sender.clear"] = lambda: f["entry"]["rule"].__setitem__("rows", [])
        for key, field in [("subject", "subject"), ("body", "body"), ("rest", "rest_of_thread"), ("attachments", "attachments")]:
            d["filter.ai.reads." + key] = tog(f["ai"], field)
        self._source_doors(d, setv, tog)
        self._text_doors(d, setv, tog)
        if "extra-control" in self.m:
            d["sources.new-thing"] = lambda: None
        for name in list(d):
            if "noop:" + name in self.m:
                d[name] = lambda: None
        return d

    def _source_doors(self, d, setv, tog):
        ui, r = self.ui, self.retrieval

        def ask():
            self.asks += 1
        d["sources.add"] = ask
        d["sources.add-folder"] = ask
        d["sources.picker.cancel"] = setv(ui, "picker_open", False)
        d["sources.add-folder.sample"] = lambda: self.local.append({"source": {"kind": "local_folder", "path": FOLDER}, "enabled": True, "item_count": 0, "progress": {"state": "syncing"}})
        for a, fs in self.accounts.items():
            d["sources.account." + a] = (lambda a=a: ui["open_accounts"].remove(a) if a in ui["open_accounts"] else ui["open_accounts"].append(a))
            for i, fo in enumerate(fs):
                d["sources.account.%s.folder.%d" % (a, i)] = (lambda a=a, i=i: self.tile(a, i))
        for i, x in enumerate(self.local):
            d["sources.local.%d" % i] = tog(x, "enabled")
            d["sources.local.%d.remove" % i] = (lambda x=x: self.local.remove(x))
        for c in self.calendars:
            d["sources.calendar." + c["calendar_id"]] = tog(c, "enabled")
        for sc in ["this_account", "all_accounts"]:
            d["sources.scope." + sc] = setv(r, "scope", sc)
        for n in [3, 5, 8, 10]:
            d["retrieval.per-question.%d" % n] = setv(r, "excerpts_per_question", n)
        for z in ["sentence", "paragraph", "section"]:
            d["retrieval.size." + z] = setv(r, "excerpt_size", z)
        d["retrieval.fields"] = setv(ui, "popover", "fields")
        for fld in ["date", "sender", "folder", "subject"]:
            d["retrieval.fields." + fld] = tog(r["attached_fields"], fld)
        for n in [3, 6, 10, 20]:
            d["retrieval.keep.%d" % n] = setv(r, "keep_n", n)
        for c in ["keep_both_dated", "keep_newest", "keep_oldest"]:
            d["retrieval.clash." + c] = setv(r, "clash_rule", c)
        d["retrieval.ai-filter"] = setv(ui, "popover", "aiFilter")
        d["retrieval.ai-filter.field"] = setv(r, "ai_filter_question", FILTER_Q)
        d["retrieval.ai-filter.save"] = setv(r, "ai_filter_question", FILTER_Q)
        d["retrieval.ai-filter.off"] = setv(r, "ai_filter_question", None)

    def _text_doors(self, d, setv, tog):
        ui, v = self.ui, self.voice
        for n in [50, 100, 200, 500]:
            d["voice.sample.%d" % n] = setv(ui, "picked_sample", n)
        d["voice.analyse"] = lambda: v.update(sample_size=ui["picked_sample"] or v["sample_size"], analysed_at_ms=v["analysed_at_ms"] + 1)
        for n in [0, 1, 3, 5]:
            d["voice.examples.%d" % n] = setv(v, "same_person_examples", n)
        d["voice.tones"] = setv(ui, "popover", "tones")
        for t in self.tones:
            d["voice.tone.%s.delete" % t["id"]] = (lambda t=t: self.tones.remove(t))
        d["voice.tone.add"] = lambda: self.tones.append({"id": "tone-%d" % len(self.tones), "name": "Chaleureux"})
        d["prompt.preview"] = self._preview
        d["prompt.variables"] = setv(ui, "popover", "variables")
        d["prompt.notes"] = setv(ui, "popover", "notes")
        d["prompt.versions"] = setv(ui, "popover", "versions")
        d["prompt.note.add"] = lambda: self.notes.append({"id": "note-%d" % (len(self.notes) + 9), "title": "Ton", "text": "Toujours vouvoyer."})
        for n in self.notes:
            d["prompt.note.%s.delete" % n["id"]] = (lambda n=n: self.notes.remove(n))
        for p in self.prompts:
            if not p["current"]:
                d["prompt.version.%d.restore" % p["version"]] = (lambda p=p: self._save(p["text"]))
        for vid in VARS:
            d["prompt.variable." + vid] = (lambda vid=vid: (self.ed("prompt").insert([("P", vid), " "], pill=True), ui.update(popover=None)))
        for k in self.places():
            d["editor." + k] = (lambda k=k: self.focus_on(k))
            d["editor.%s.save" % k] = (lambda k=k: self.ed_save(k))
            d["editor.%s.revert" % k] = (lambda k=k: self.ed(k).revert_click())
        if "old-double-click" in self.m:
            d["doc.prompt"] = lambda: None

    def _save(self, text):
        top = max(p["version"] for p in self.prompts)
        for p in self.prompts:
            p["current"] = False
        self.prompts.append({"version": top + 1, "text": text, "current": True})

    # ---- the wire
    def handle(self, verb, args):
        if verb == "fixture" and args[:1] == ["reset"]:
            self.reset()
            return {}
        if verb == "click":
            ax = args[0]
            if ax.startswith(LANG_DOOR % ""):      # the app's language (Settings > General); "system" is the stand-in's English
                code = ax[len(LANG_DOOR % ""):]
                self.lang = code if code in _DESC else "en"
                return {}
            if ax in (P + ".fixture.reset", P + ".fixture.relaunch"):
                self.wishes, self.eds, self.focused = {}, {}, None
                self.armed, self.fresh, self.downs, self.mouse_focus = set(), False, 0, None
                self.follow()
                return {}
            m = re.match(r"%s\.editor\.(.+)\.(cursor\.start|cursor\.middle|cursor\.end|at|paste-sample|mouse-click)$" % re.escape(P), ax)
            if m:
                self.ed_seam(m.group(1), m.group(2))
                return {}
            m = re.match(r"%s\.fixture\.(folder-gone|folder-back)\.(.+)\.(\d+)$" % re.escape(P), ax)
            if m:
                self.gone_seam(m.group(2), int(m.group(3)), m.group(1) == "folder-gone")
                return {}
            door = self.doors().get(ax[len(P) + 1:]) if ax.startswith(P + ".") else None
            if door is None:
                raise KeyError("nothing to click at %s" % ax)
            door()
            self.follow()     # the app's every write reads the core again (RepliesStore.run > load > followLanguage)
            return {}
        if verb == "rows":
            ax = args[0]
            if ax == P + ".controls":
                return {"rows": [{"ax": "%s.%s" % (P, k), "page": "", "kind": ""} for k in self.doors() if k not in SEAMS]}
            table = {P + ".editors": self.editor_rows, P + ".state": lambda: [self.state()], P + ".place": lambda: [{"page": self.ui["page"]}], P + ".sources": self.sources_rows,
                     BACKGROUND: self.background}
            if ax not in table:
                raise KeyError("no rows at %s" % ax)
            return {"rows": table[ax]()}
        if verb == "keyevent":
            return self.keyevent(args[-1])
        if verb == "uistate":
            return {"state": {"core": "rust" if "real-core" in self.m else "fixture"}}
        if verb == "core" and args[:1] == ["state"]:
            return {"value": self.core(args[1])}
        raise KeyError("unknown verb %s" % verb)


class _Handler(socketserver.StreamRequestHandler):
    def handle(self):
        for line in self.rfile:
            req = json.loads(line)
            try:
                reply = dict(ok=True, **self.server.app.handle(req["verb"], req.get("args", [])))
            except (KeyError, ValueError, IndexError) as e:
                reply = {"ok": False, "error": str(e)}
            self.wfile.write((json.dumps(reply) + "\n").encode())


class FakeServer:
    def __init__(self, mutants=()):
        self.dir = tempfile.mkdtemp(prefix="wsr", dir="/tmp")
        self.srv = socketserver.ThreadingUnixStreamServer(os.path.join(self.dir, "whoosh.sock"), _Handler)
        self.srv.app = FakeApp(mutants)
        threading.Thread(target=self.srv.serve_forever, daemon=True).start()

    def close(self):
        self.srv.shutdown()
        self.srv.server_close()
        shutil.rmtree(self.dir, ignore_errors=True)


# ---------------------------------------------------------------- the runner
def run(ctl, cases):
    out = []
    for label, _doors, fn in cases:
        reset(ctl)
        try:
            out.append((label, "pass" if fn(ctl) else "fail", ""))
        except Red as e:
            out.append((label, "fail", str(e)))
        except Excluded as e:
            out.append((label, "excluded", str(e)))
        except Unreadable as e:
            out.append((label, "unjudged", str(e)))
        except (KeyError, IndexError, TypeError, ValueError) as e:
            out.append((label, "fail", "reply shape: %s %s" % (type(e).__name__, e)))
    return out


def summary(res):
    n, f, u = len(res), sum(r[1] == "fail" for r in res), sum(r[1] == "unjudged" for r in res)
    for l, st, d in res:
        if st == "excluded":
            print("ok %s excluded, %s" % (l, d))   # never counted as not judged: judged on the engine its words name
        elif st != "pass":
            sys.stderr.write("%s %s: %s\n" % (st.upper(), l, d))
        else:
            print("PASS %s" % l)                                  # ONE line per passing case
    print("%d checks, %d failed, %d not judged" % (n, f, u))
    return f, u


def against(mutants, cases):
    srv = FakeServer(mutants)
    ctl = Ctl(srv.dir).connect()
    try:
        return run(ctl, cases)
    finally:
        ctl.close()
        srv.close()


def selftest():
    f, u = summary(against((), CASES))
    doors = sorted(k for k in FakeApp().doors() if k not in SEAMS)
    rows, bad = [], 0
    for door in sorted(LANG_MUTANTS):      # hid-ai round 6: each behaviour of the description's language has its broken copy
        res = against((door,), [c for c in CASES if c[0] == "ai-description-in-the-app-language"])
        red = [l for l, st, _d in res if st == "fail"]
        bad += not red
        rows.append("| %s | ai-description-in-the-app-language | %s |" % (door, "RED" if red else "NOT RED"))
    for door in sorted(DEFAULT_MUTANTS):   # hid-ai round 18: each behaviour of the shipped defaults has its broken copy
        res = against((door,), [c for c in CASES if c[0] == "shipped-defaults-follow-the-app-language"])
        red = [l for l, st, _d in res if st == "fail"]
        bad += not red
        rows.append("| %s | shipped-defaults-follow-the-app-language | %s |" % (door, "RED" if red else "NOT RED"))
    for door in doors + ["extra-control"] + sorted(GONE_MUTANTS) + sorted(EDITOR_MUTANTS) + sorted(SOURCE_MUTANTS) + sorted(A124_MUTANTS):
        if door in A124_MUTANTS:   # one mutant, every a124 case run against it: its own case must be red
            res = against((door,), [c for c in CASES if c[0].startswith(A124)])
            red = [l for l, st, _d in res if st == "fail"]
            own = A124_MUTANTS[door][0]
            bad += own not in red
            rows.append("| %s | %s | %s |" % (door, own, ("RED: " + own if own in red else "NOT RED") + " (all red: %s)" % ", ".join(red)))
            continue
        mutant = door if door == "extra-control" or door in GONE_MUTANTS or door in EDITOR_MUTANTS or door in SOURCE_MUTANTS else "noop:" + door
        cases = [c for c in CASES if door == "extra-control" and c[0] == "every-control-has-a-case"
                 or door in SOURCE_MUTANTS and c[0] == "sources-tiles-calendars-scope"
                 or door in GONE_MUTANTS and c[0].startswith("gone-")
                 or door in EDITOR_MUTANTS and c[0].startswith("editor-")
                 or any(fnmatch.fnmatchcase(door, p) for p in c[1])]
        res = against((mutant,), cases)
        red = [l for l, st, _d in res if st == "fail"]
        bad += not red
        rows.append("| %s | %s | %s |" % (mutant, ", ".join(c[0] for c in cases) or "NO CASE", "RED: " + red[0] if red else "NOT RED"))
    print("MUTANTS (one per control, %d controls)" % len(doors))
    print("| mutant | cases | observed |\n|---|---|---|\n" + "\n".join(rows))
    # the static case on a source copy: a typed sentence is red, a symbol or an interpolated figure is not
    typed = bare_text_literals({"X.swift": 'Text("Choose a folder")', "Y.swift": 'Text("@" + n)\nText("\\(p) %")'})
    red_ok = list(typed) == ["X.swift"] and not bare_text_literals(replies_sources())
    bad += not red_ok
    print("| bare-text-literal | no-text-literal-holds-a-bare-sentence | %s |" % ("RED on a typed sentence only: %s" % typed if red_ok else "NOT RED or wrong: %s" % typed))
    # round 15 (item 4f131cf1, F2): the hand-over case passes on the stand-in, is red on each broken copy, and is not judged on the test engine
    global HANDOVER_WAIT_S, INDEX_WAIT_S, READ_EVERY_S
    HANDOVER_WAIT_S, INDEX_WAIT_S, READ_EVERY_S = 5.0, 4.0, 0.25   # the stand-in hands over in 1.5 s and indexes in 1 s more; a broken copy is red after 4 s, not 60
    hf, hu = summary(against(("real-core",), HANDOVER_CASES))
    f, u = f + hf, u + hu
    for door in sorted(HANDOVER_MUTANTS):
        res = against((door, "real-core"), HANDOVER_CASES)
        red = [l for l, st, _d in res if st == "fail"]
        bad += not red
        print("| %s | %s | %s |" % (door, HANDOVER_CASES[0][0], "RED: " + red[0] if red else "NOT RED"))
    for mode, words in (("no-account", "no mail account"), ("all-up-to-date", "already read up to date")):   # round 22: the NOT JUDGED line names what is missing
        res = against(("real-core", mode), HANDOVER_CASES)
        ok = res[0][1] == "unjudged" and "precondition not met" in res[0][2] and words in res[0][2]
        bad += not ok
        print("| %s | %s | %s |" % (mode, HANDOVER_CASES[0][0], "NOT JUDGED naming %r" % words if ok else "WRONG: %s %s" % (res[0][1], res[0][2][:120])))
    res = against((), HANDOVER_CASES)      # round 16: the test engine excludes it (its own `ok ... excluded` line), never not judged
    bad += res[0][1] != "excluded"
    print("| test-engine | %s | %s |" % (HANDOVER_CASES[0][0], "excluded, as it must be: %s" % res[0][2][:60] if res[0][1] == "excluded" else "WRONG: %s" % res[0][1]))
    gone = [c for c in CASES if c[0].startswith("gone-")]
    res = against(("real-core",), gone)
    wrong = [(l, st) for l, st, _d in res if st != "excluded"]
    bad += bool(wrong) or not gone
    print("| real-core | %s | %s |" % (", ".join(c[0] for c in gone), "all excluded, as they must be" if not wrong and gone else "WRONG: %s" % wrong))
    # round 13: the hang sample reader names the innermost frames of the main thread's stuck chain, not the top of the report
    fake = ("Call graph:\n    1591 Thread_100   DispatchQueue_1: com.apple.main-thread\n    + 1591 start  (in dyld) + 6  [0x1]\n"
            "    +   1591 static KeyEventVerb.run(_:_:)  (in Whoosh) + 4888  [0x2]\n    +     1591 -[NSRunLoop runUntilDate:]  (in Foundation)  [0x3]\n"
            "    +       1591 RepliesTextView.keyDown(with:)  (in Whoosh)  [0x4]\n    +       12 other  (in x)  [0x5]\n    1591 Thread_200\n    + 1591 accept\n")
    frames = deepest_frames(fake, 2)
    hang_ok = frames == ["-[NSRunLoop runUntilDate:]  (in Foundation)", "RepliesTextView.keyDown(with:)  (in Whoosh)"]
    bad += not hang_ok
    print("| hang-sample-reader | editor-preview-says-which | %s |" % ("reads the innermost frames: %s" % frames if hang_ok else "WRONG: %s" % frames))
    ok = f == 0 and u == 0 and bad == 0
    print("selftest: %s (%d mutants not red)" % ("OK" if ok else "FAILED", bad))
    return 0 if ok else 1


def handover_launch(rest):
    """`--handover-launch <bundle>` (round 22): the local mail server in THIS process, then the real-engine copy launched by scripts/launch-one.sh with it, the model
    folder (WHOOSH_EMBED_WEIGHTS from this environment) and the hand-over held; inside, the hand-over case with its mail set-up. Prints the case's lines."""
    import subprocess
    label = HANDOVER_CASES[0][0]
    weights = os.environ.get("WHOOSH_EMBED_WEIGHTS", "")
    if not rest or not weights or not os.path.isdir(weights):
        print("UNJUDGED %s: precondition not met: %s" % (label, "no bundle named" if not rest else "WHOOSH_EMBED_WEIGHTS is not a folder (%r)" % weights))
        print("1 checks, 0 failed, 1 not judged")
        return 1
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake  # noqa: E402
    try:
        mail, imap, smtp = fake.start(0, 0, inbox=HANDOVER_MAIL)
    except OSError as e:
        print("UNJUDGED %s: precondition not met: the local mail server did not start: %s" % (label, e))
        print("1 checks, 0 failed, 1 not judged")
        return 1
    ports = "%d:%d" % (imap.server_address[1], smtp.server_address[1])
    extra = ["WHOOSH_RIG_HIDDEN=1", "WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=" + ports, "WHOOSH_EMBED_WEIGHTS=" + weights,
             "WHOOSH_RIG_EMBED_HANDOVER_DELAY_MS=%d" % HANDOVER_DELAY_MS]
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(extra))
    env.pop("WHOOSH_FIXTURE_CORE", None)
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), rest[0], "--", "--rig-hidden",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" WHOOSH_HANDOVER_SETUP=1 python3 "$0" --handover', os.path.abspath(__file__)]
    try:
        r = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True)
    finally:
        imap.shutdown(); smtp.shutdown()
    out = r.stdout + r.stderr
    print(out.rstrip())
    print("launcher exit %d; the local mail server saw: %s" % (r.returncode, sorted({l.split(" ")[0] for l in mail.log})))
    return 0 if ("1 checks, 0 failed, 0 not judged" in out) else 1


def main(argv):
    if "--selftest" in argv:
        return selftest()
    if "--handover-launch" in argv:
        return handover_launch(argv[argv.index("--handover-launch") + 1:])
    cases = HANDOVER_CASES if "--handover" in argv else CASES   # --handover: the hand-over case alone, on a real-engine copy (see HANDOVER_CASES)
    try:
        ctl = Ctl().connect()
    except CouldNotDrive as e:
        res = [(l, "unjudged", str(e)) for l, _d, _f in cases]
    else:
        try:
            res = run(ctl, cases)
        finally:
            ctl.close()
    f, u = summary(res)
    return 1 if f or u else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
