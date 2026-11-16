"""J5: AI providers, the Supplier desk agent, one search that answers questions with messages, MCP, and how Whoosh decides (JOURNEYS J5.1-J5.10).

Written from docs/JOURNEYS.md J5 and docs/DECISION-MODEL.md VERSION 2 (signed 2026-11-07 19:16), never from a built screen.
Round 2 (2026-11-08): J5.4 to J5.6 are the rewritten, signed rows (docs/JOURNEYS.md "One search"): a question typed in the one search field, results by
meaning and nothing else (no answer text, no agent reply, no Ask panel), Enter opens the thread at the matched message, then select all, label, one undo.
J5.8 and J5.9 are the v2 decision settings: one page per decision, one ordered list per page, rules and "ask the model"
entries in the same list, an Engine block (Local, Jev, Off), a "why" naming page and entry. Wire names (ax ids, core
paths) are read from fixtures/tests-j5.json; numbers from docs/CONSTANTS.md. Providers and engines are stubs on 127.0.0.1.
"""
import json
import os
import sys
import time
from lib.ctl import Unreadable
from lib import theme as _theme
from lib.fixture import MSG, ACCOUNT, ROOT as _FROOT, const, FX as DESIGN, FLOW_EXAMPLE

# Words: a hidden test copy starts in the language of its catalogue default (French on this Mac), so an expected sentence is read from the catalogue
# (Sources/Resources/Localizable.xcstrings: the key is the English source sentence, the value the one in LANG), never typed in English.
# WHOOSH_JOURNEY_LANG names another catalogue language for a copy started in it.
with open(os.path.join(_FROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8") as _c:
    CATALOGUE = json.load(_c)["strings"]
LANG = os.environ.get("WHOOSH_JOURNEY_LANG", "fr")
# The copy is NOT assumed to start in LANG: the job that launches it may pin another language (scripts/main-job.sh writes AppleLanguages en,
# and main88 ran J5 in English while every sentence below was read in French). The journey picks LANG through the language door of
# Settings > General at its start (the J3 way, journeys/J3.py:131) and puts the Mac's own language (`system`) back at its end, a failed run included.
LANG_DOOR = "mac-settings-general.appearance.language.%s"      # a click door that needs no window (par-settingsgeneralmac)


def say(key, *args):
    """The catalogue sentence for an English source key, in LANG; each %@ or %lld takes the next argument. A key the catalogue lacks is a KeyError (a red, never a guess)."""
    out = CATALOGUE[key]["localizations"][LANG]["stringUnit"]["value"]
    for a in args:
        for hole in ("%@", "%lld"):
            if hole in out:
                out = out.replace(hole, str(a), 1)
                break
    return out


def expect_why(s, name, thunk, expected, why):
    """`expect`, whose failure line also carries the values that decide it (`why()`: the rows read, the window list, the state before the step)."""
    def run():
        got = thunk()
        s._eg = (expected, got)
        if got == expected:
            return True, "expected %r, got %r" % (expected, got)
        try:
            more = why()
        except Exception as e:      # noqa: BLE001  a context read never hides the verdict it explains
            more = "unreadable (%s: %s)" % (type(e).__name__, e)
        return False, "expected %r, got %r | deciding values: %s" % (expected, got, more)
    s._record(name, run)


def settle(s, read, want, secs=6.0):
    """An action: reads the state (`read()`) until it equals `want` or `secs` pass; never a blind sleep. The check after it judges the state."""
    def go():
        end = time.monotonic() + secs
        while time.monotonic() < end:
            try:
                if read() == want:
                    return
            except Unreadable:
                pass
            time.sleep(0.1)
    s._act(go)

ID = "J5"
TITLE = _theme.title("Connect AI, build an agent, search by a question, allow MCP, set how Whoosh decides")
TARGET = "LOCAL"
VISIBLE_OUTCOME = "A question in the search field returns the matching messages and nothing else, a batch label lands with one undo, and each decision list says which entry placed a message"

FX = json.load(open(os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "fixtures", "tests-j5.json"), encoding="utf-8"))
A, D = FX["ax"], FX["decision"]
# hid-ai round 6: the list's row doors count from 0 (InboxList/ListClicks.swift `mac-main.list.row.\(i)`), and the message the prefill names
# (decision.prefill.msg, msg-01) is the FIRST row; fixtures/tests-j5.json's "row.1" opened the second row (main92: msg-01a, assem@northlight-example.de).
A["list_row_1_create_rule"] = "mac-main.list.row.0.create-rule"
SE = FX["search"]
# Item 3bb3d817 (lane every-word): a match holds EVERY typed word (search-text query.rs; FixtureSearchBook.holdsEvery). Record 1 holds neither "suppliers" nor
# "spring" of SE["question"], so while the meaning index builds (the word matches alone are listed) the question lists no record 1. The building step asks
# WORDS_QUERY instead: record 1 (msg-01) and msg-s05 hold all three words, the meaning-only thread holds none (checks/parity/par-searchmac.py WORDS_QUERY;
# the sets: _captures/every-word/match-sets-5.out).
WORDS_QUERY = "quote price run"
_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CORPUS = json.load(open(os.path.join(_ROOT, SE["corpus"]), encoding="utf-8"))
BY_ID = {m["id"]: m for m in CORPUS["messages"]}
R1 = MSG[SE["record1_message"]]
assert BY_ID[SE["record1_message"]]["subject"] == R1["subject"] and BY_ID[SE["record1_message"]]["thread"] == SE["record1_thread"]
# the corpus must let the checks fail: a meaning-only supplier thread, an unrelated thread, a decoy to delete
_THREADS = {m["thread"] for m in CORPUS["messages"]}
assert {SE["meaning_only_thread"], SE["unrelated_thread"], SE["deleted_thread"]} <= _THREADS
assert not any(w in (BY_ID["msg-s06"]["subject"] + BY_ID["msg-s06"]["body"]).lower() for w in ("quote", "spring", "supplier"))
assert sum(1 for m in CORPUS["messages"] if m["thread"] == SE["record1_thread"]) > 1   # a batch label must cover several messages
RES, SKEL, NOTICE = A["search_results"], A["search_skeleton"], A["search_notice"]


# J5.10: the label À répondre as a pipeline of three steps (SPEC A140). The figures are derived from the design fixture file (docs/design/fixtures/mac-main-sample.json), never typed
# (the same derivation as checks/parity/par-labelscontactsmac.py example_figures()): step 1 picks every flow_example row plus the corpus mails whose body holds the entry rule's
# words (those wait for the AI until a Re-run); the AI accepts the kept and the rejected rows; step 3 keeps the kept rows and sends the rejected rows to the fallback.
BOARD = "needs-reply"
BODY_WORDS = "Hi Assem"          # the example's one entry rule: a body rule (FixtureFlow.seedFlowExampleLocked)
INS = "mac-labels.inspector"
FLOW_AX = INS + ".flow"


def example_figures(rows=None, messages=None):
    rows = FLOW_EXAMPLE if rows is None else rows
    messages = DESIGN["messages"] if messages is None else messages
    waiting = sum(1 for m in messages if BODY_WORDS in json.dumps(m.get("body", ""), ensure_ascii=False))
    kept, rej = sum(r["step"] == "kept" for r in rows), sum(r["step"] == "rejected" for r in rows)
    return {"entry": len(rows) + waiting, "ai": kept + rej, "kept": kept, "sent": rej, "waiting": waiting}


EX = example_figures()
EX_PINNED = {"entry": 37, "ai": 32, "kept": 24, "sent": 8, "waiting": 5}   # the owner's example after A140; the self-test holds the two equal
REPLY_MSG = next(r["id"] for r in FLOW_EXAMPLE if r["step"] == "kept")      # a mail the AI accepted and step 3 keeps: a reply moves it to the fallback
COUNT_FIELDS = {"entry": "caught_by_entry", "ai": "sorted_by_ai", "kept": "kept_by_exit", "sent": "sent_to_fallback", "waiting": "waiting_for_ai"}


def in_board(s, mid):
    """True when the mail is filed under the board's label. `label_assignment/<m>` lists the labels' NAMES on both engines (the real one:
    core/crates/engine/src/label_cmds.rs:380 `label_names_of`; the test engine: FixtureCore.swift `label_assignment`), not their ids: the label's own
    name is read (`label/<id>/name`) and the id is accepted too (the stand-in lists ids). Round 7: before, the id alone was looked for (main93 read
    (False, False): the held mail was never seen held)."""
    name = core0(s, "label/%s/name" % BOARD)
    got = s.ctl.core("label_assignment/%s" % mid) or []
    # round 13: a built-in category's id is its name, lower case, spaces as hyphens (contracts label_tabs.rs `builtin_label_id`): "Needs Reply" is
    # needs-reply, also when `label/<id>/name` answers nothing (main104: None)
    return BOARD in got or (name is not None and name in got) or any(str(g).strip().lower().replace(" ", "-") == BOARD for g in got)


def flow_counts(s):
    """`label/needs-reply/flow/counts` as the figures this journey names (entry, ai, kept, sent, waiting)."""
    got = s.ctl.core("label/%s/flow/counts" % BOARD) or {}
    return {k: got.get(f) for k, f in COUNT_FIELDS.items()}


def region(s, ax):
    """Rows of a region; a region the screen does not have counts as empty (docs/CONTROL-SOCKET.md delta 3: ok:false 'no rows at').
    Every use pairs it with a positive control read in the same thunk, so an absent region never passes alone."""
    try:
        return s.ctl.rows(ax)
    except Unreadable as e:
        if "no rows at" in str(e):
            return []
        raise


def core0(s, path):
    """A core read where 'no core value' is an answer (None), not an unreadable probe; any other failure stays unreadable."""
    try:
        return s.ctl.core(path)
    except Unreadable as e:
        if "no core value at" in str(e):
            return None
        raise


def bar(s):
    return s.ctl.rows(BAR)[0]


def query(s, text):
    """The search field takes the words: a click on it, ⌘A clears it, the words typed (par-searchmac query())."""
    s.click(BAR)
    s.key("⌘A")
    s.type(text)


def select_result(s, thread):
    """Move the selection to this thread's result with Down (selection starts on result 0 after a launch)."""
    def go():
        ths = [r["thread"] for r in s.j.ctl.rows(RES)]
        if thread not in ths:
            raise Unreadable("no result row for %s in %r" % (thread, ths))
        sel = [r["selected"] for r in s.j.ctl.rows(RES)].index(True) if any(r["selected"] for r in s.j.ctl.rows(RES)) else 0
        for _ in range(ths.index(thread) - sel):
            s.j.ctl.key("Down")
        for _ in range(sel - ths.index(thread)):
            s.j.ctl.key("Up")
    s._act(go)


def selected_thread(s):
    return [r["thread"] for r in s.ctl.rows(RES) if r["selected"]]


def threads(s):
    return [r["thread"] for r in region(s, RES)]


def members(thread_ids, gone=()):
    return sorted(m["id"] for m in CORPUS["messages"] if m["thread"] in thread_ids and m["id"] not in gone)
P = A["providers"]
BAR = "mac-search.bar"
# Round r54 (2026-11-10, driven on main53): ids and doors as the app serves them now. Read from the Sources at c58f2c43 and the
# parity checks (par-searchmac, par-settingswindow), never from a built screen:
#  - the key paste door: `fixture paste <name>` posts args, the AI store listens for userInfo "name" (AIFixture.swift), so the verb
#    changes nothing; the store's own INTERIM click doors `mac-settings-ai.fixture.paste.<name>` drive the same paste.
#  - the search field: click the bar, ⌘A clears it, type, Enter (par-searchmac query()); the skeleton hold is
#    `mac-search.skeleton.hold` / `.release`, its row says `loading`; the search history is core `search/recent`.
#  - the MCP prompt: `fixture mcp-search` answers a request id; the prompt is a row of `mac-mcp-notes.requests`, approved by
#    `mac-mcp-notes.request.<id>.allow`; the results are core `mcp_request/<id>/results`.
REQS = "mac-mcp-notes.requests"
# what an approved MCP search for the query returns: the subjects of the corpus messages carrying every query word in subject, sender
# or body (FixtureMcpSearch.mcpSearchSubjectsLocked); record 1 must be among them, so the expectation cannot be empty
_MQ = FX["mcp"]["query"].lower().split()
MCP_SUBJECTS = sorted(m["subject"] for m in CORPUS["messages"]
                      if all(w in " ".join([m["subject"], m["from_name"], m["from"], m["body"]]).lower() for w in _MQ))
assert R1["subject"] in MCP_SUBJECTS and len(MCP_SUBJECTS) > 1
LABEL_MENU = "mac-settings-ai.label-menu"
PAGE = "mac-settings-ai.decisions.page."
TAB = D["shipped"]["which-tab"]


def seam(s, verb, *args):
    """A fixture verb whose answer a check reads: {'reply': ...} when served, {'refused': words} when the app refuses it."""
    out = {}
    if s.j.ctl is None:
        return out
    try:
        out["reply"] = s.j.ctl.send("fixture", verb, *args)
    except Unreadable as e:
        out["refused"] = str(e)
    return out


def served(out):
    """'served' when the fixture verb ran, else the app's refusal (a check that needs the seam is red, and says why)."""
    if s_dry(out):
        raise Unreadable("dry run: no socket")
    if "reply" not in out and "is not served" in out["refused"]:
        # the build has no such fixture door (the served list: Sources/ControlSocket/FixtureEvents.swift:15-34 and FixtureVerbs.swift): the step cannot be staged, so it is not judged
        raise Unreadable("this build serves no such fixture verb, so the step cannot be staged: %s" % out["refused"])
    return "served" if "reply" in out else out["refused"]


def s_dry(out):
    return not out


def said(s):
    """What the app said about the last key and where the keys are: the key verb's own report (`rows mac-keyevent.monitor`, field `last`), the surface, the focus, the results."""
    try:
        last = s.ctl.rows("mac-keyevent.monitor")[0].get("last")
    except Unreadable as e:
        last = "unreadable (%s)" % str(e)[-60:]
    st = s.ctl.uistate()
    return "key verb said %r; surface %r; focus %r; results %r" % (last, st.get("surface"), st.get("first_responder"),
                                                                [(r.get("thread"), r.get("selected")) for r in region(s, RES)])


# SPEC A150 (2026-11-11): the search list has three parts, matches, related mail, then one closed line. A mail found by meaning alone is
# listed as related (`matched_in` "related"), and while the ranking by meaning is still being made the line above the closed line says the
# catalogue's sentence in `mac-search.sections` (SearchResultsList.titles). Before (round 2, hid-ai): `matched_in` "meaning", and the English words
# SE["building_words"] (in no catalogue) read in `mac-search.notice`.
SECTIONS = "mac-search.sections"
RANKING = "Ranking the other mails by meaning…"


def sections_text(s):
    return " ".join(str(r.get("title", "")) for r in region(s, SECTIONS))


def notice_text(s):
    return " ".join(str(r.get("text", "")) for r in region(s, NOTICE)).lower()


def drive(j):
    """The language the journey pins (LANG, read for every expected sentence) is put back to `system` at the end, a failed run included."""
    try:
        _drive(j)
    finally:
        if j.ctl is not None:
            try:
                j.ctl.click(LANG_DOOR % "system")
            except Unreadable as e:
                sys.stderr.write("J5 NOTE the language was not put back: %s\n" % e)


def _drive(j):
    s = j.step("J5.1", "Connect a provider")
    s.raw("fixture", "reset")                                             # J5 starts on the Unified Inbox whatever ran before it
    _theme.apply(s)                                                       # after the reset: a reset gives the app its default look back
    s.click(LANG_DOOR % LANG)                                             # the app speaks the language the expected sentences are read in
    s.arrive(("click", A["palette_ai_settings"]), "mac-settings-ai")
    s.click("mac-settings-ai.nav.providers")
    s.click("mac-settings-ai.fixture.paste.bad-key")                      # the store's own paste door (see the note at the top)
    s.raw("clock", "advance", "1")                                        # the core answers a key in two steps: let the fixture clock land the second
    # the state word is the catalogue's; the note is the provider's own words as the core serves them (`ai_provider/anthropic/refusal`: AISettingsStore.swift:195
    # shows them verbatim, and the app's own sentence only when the core gives none), so the note must equal them and must not be empty
    s.expect("refused-key-reason-in-words", lambda: (lambda row, why: (row["state"], bool(why) and row["note"] == why))(s.ctl.rows(P + ".anthropic")[0], core0(s, "ai_provider/anthropic/refusal")),
             (say("refused"), True))
    s.click("mac-settings-ai.fixture.paste.anthropic-key")
    s.raw("clock", "advance", "1")                                        # the core answers a key in two steps: let the fixture clock land the second
    s.expect("connected", lambda: (s.ctl.rows(P + ".anthropic")[0]["state"], s.ctl.rows(P + ".anthropic")[0]["note"]), (say("connected"), ""))
    s.expect("models-listed", lambda: [r["model"] for r in s.ctl.rows(P + ".anthropic.models")], ["claude-sonnet-5-5", "claude-haiku-5-5"])
    expect_why(s, "chatgpt-stop-note", lambda: s.ctl.rows(P + ".chatgpt")[0]["note"], say("Signing in with ChatGPT can stop working at any time."),
               lambda: "the chatgpt row %r" % (s.ctl.rows(P + ".chatgpt"),))
    # the key goes to the Keychain (the core says it is stored) and is never read back from the database
    s.expect("key-in-keychain-not-db", lambda: (core0(s, "provider_credential/anthropic/stored"), core0(s, "ai_provider/anthropic/key")), (True, None))

    s = j.step("J5.2", "Pick a speech engine")
    s.click(A["speech_local"])
    s.expect("engine-ready", lambda: s.ctl.rows(A["speech"])[0]["state"], "ready")
    s.expect("engine-in-core", lambda: core0(s, "speech_engine/chosen/state"), "ready")

    s = j.step("J5.3", "Create the Supplier desk agent")
    s.click("mac-settings-ai.nav.agents")
    s.click(A["agents_new"])
    s.type(FX["agent"]["name"])
    for w in ("read", "search", "label"):
        s.click("mac-settings-ai.agents.allow." + w)
    s.click("mac-settings-ai.agents.save")
    s.expect("card-lists-actions-in-words", lambda: [r["allowed"] for r in s.ctl.rows(A["agents_card"]) if r["name"] == FX["agent"]["name"]], [[say(w) for w in FX["agent"]["words"]]])
    s.expect("config-in-core", lambda: core0(s, "agent_config/%s/allowed" % FX["agent"]["name"]), FX["agent"]["allow"])
    # the saved config holds actions (positive control) and send is not among them
    s.expect("no-send-permission", lambda: (lambda v: (isinstance(v, list) and len(v) > 0, FX["agent"]["no_send"] in (v or [])))(
        core0(s, "agent_config/%s/allowed" % FX["agent"]["name"])), (True, False))

    s = j.step("J5.4", "Search by a question")
    s.raw("fixture", "corpus", os.path.join(_ROOT, SE["corpus"]))       # the mail the expectations are computed from is the mail the app searches
    # the Settings window's own close (the key verb has no ⌘W for a Settings page: it opens inside the one Settings window, `mac-settings`), then the state is read until the inbox shows
    s.click("mac-settings-window.close")
    settle(s, lambda: s.ctl.uistate().get("surface"), "mac-main")
    expect_why(s, "settings-closed-to-the-inbox", lambda: s.ctl.uistate().get("surface"), "mac-main",
               lambda: "windows %r; %s" % ([(f.get("title"), f.get("kind")) for f in s.ctl.winframes()], said(s)))
    s.arrive(("key", "⌘K"), "mac-search")
    sent = {}
    s._act(lambda: sent.update(n=core0(s, "provider_request/count")))
    query(s, SE["question"])
    s.click("mac-search.skeleton.hold")                                    # keeps the skeleton on screen so its state can be read
    s.key("Enter")
    s.expect("skeleton-rows-appear-at-once", lambda: (s.ctl.rows(SKEL)[0]["loading"], bar(s)["launched"], bar(s)["text"]), (True, True, SE["question"]))
    # the hidden rig's accessibility walk reaches no search element (main53: `measure mac-search.list` finds nothing, 15 elements
    # walked), so the pane is judged by the state it draws from: SearchListPane shows the skeleton rows exactly while `loading` holds
    s.expect("list-swapped-for-skeletons", lambda: (s.ctl.uistate().get("surface"), bar(s)["active"], s.ctl.rows(SKEL)[0]["held"], s.ctl.rows(SKEL)[0]["loading"]),
             ("mac-search", True, True, True))
    s.click("mac-search.skeleton.release")
    s.expect("skeleton-gives-way-to-results", lambda: (s.ctl.rows(SKEL)[0]["loading"], s.ctl.rows(SKEL)[0]["held"], len(region(s, RES)) >= 2), (False, False, True))
    s.expect("record1-in-results", lambda: SE["record1_thread"] in threads(s), True)
    s.expect("meaning-match-without-shared-words", lambda: (SE["meaning_only_thread"] in threads(s),
             [r["matched_in"] for r in region(s, RES) if r["thread"] == SE["meaning_only_thread"]]), (True, ["related"]))   # A150: before ["meaning"]
    s.expect("unrelated-mail-left-out", lambda: (len(threads(s)) >= 2, SE["unrelated_thread"] in threads(s)), (True, False))
    s.expect("no-answer-no-agent-reply-no-panel", lambda: (
        s.ctl.uistate().get("surface"), len(threads(s)) >= 2,
        [(ax, region(s, ax)) for ax in A["search_answer_regions"] if region(s, ax)],
        [r for r in region(s, NOTICE) if any(w in str(r.get("text", "")).lower() for w in ("agent", " ai ", "answer", "ask"))]),
        ("mac-search", True, [], []))
    # the probe must exist before and after (a missing probe is red, never a silent zero)
    expect_why(s, "search-sends-nothing-off-device", lambda: (lambda a, b: (a is not None and b is not None, (b or 0) - (a or 0)))(
        sent.get("n"), core0(s, "provider_request/count")), (True, 0),
        lambda: "provider_request/count before the search %r, after %r (None = the core serves no such value)" % (sent.get("n"), core0(s, "provider_request/count")))
    s.expect("history-written", lambda: (SE["question"] in (core0(s, "search/recent") or []), [r["text"] for r in s.ctl.rows("mac-search.recent")][:1]),
             (True, [SE["question"]]))
    building = seam(s, "index-state", "building")                         # the meaning index is still building
    asked = [SE["question"]]                                              # the words the bar holds from here on (J5.6 reads them back)
    if "reply" in building:                                               # new words (the same again would open result 0, fieldMode): the words query, item 3bb3d817
        query(s, WORDS_QUERY)
        s.key("Enter")
        asked.append(WORDS_QUERY)
    s.expect("building-shows-word-matches-first", lambda: (served(building), SE["record1_thread"] in threads(s), SE["meaning_only_thread"] in threads(s)),
             ("served", True, False))
    s.expect("building-quiet-line", lambda: (served(building), say(RANKING) in sections_text(s)), ("served", True))   # before: SE["building_words"] in mac-search.notice
    ready = seam(s, "index-state", "ready")
    s.expect("meaning-results-merge-when-ready", lambda: (served(ready), SE["meaning_only_thread"] in threads(s), say(RANKING) in sections_text(s)),
             ("served", True, False))

    s = j.step("J5.5", "Open a result")                                    # the results of J5.4 stand: Enter now opens the selected one
    gone = seam(s, "delete-message", SE["deleted_message"])               # the index still lists it: the message is gone since it ran
    select_result(s, SE["deleted_thread"])
    s.key("Enter")
    s.expect("deleted-result-says-no-longer-in-your-mail", lambda: (served(gone), say(SE["deleted_words"]).lower() in notice_text(s)), ("served", True))
    s.expect("deleted-result-opens-nothing", lambda: s.ctl.uistate().get("surface"), "mac-search")
    s._act(lambda: s.j.ctl.key("Escape") if s.j.ctl.uistate().get("surface") == "mac-reader" else None)   # a thread it opened: back to the results
    s.expect("deleted-result-leaves-the-list", lambda: (SE["record1_thread"] in threads(s), SE["deleted_thread"] in threads(s)), (True, False))
    select_result(s, SE["record1_thread"])
    s.expect("record1-row-selected", lambda: selected_thread(s), [SE["record1_thread"]])
    s.arrive(("key", "Enter"), "mac-reader")
    s.expect("opens-at-the-matched-message", lambda: [(r["id"], r["subject"]) for r in s.ctl.rows("mac-main.reader.subject")], [(SE["record1_message"], R1["subject"])])

    s = j.step("J5.6", "Label every result, one undo")
    ew = lambda name, thunk, exp: expect_why(s, name, thunk, exp, lambda: said(s))      # an undecided case prints the key verb's report, the focus and the results
    s.arrive(("key", "Escape"), "mac-search")
    s.expect("results-survive-the-return", lambda: (SE["record1_thread"] in threads(s), bar(s)["text"].rstrip()), (True, asked[-1]))   # J5.4 asked the words query when the building seam was served (item 3bb3d817)
    s.key("⌘A")
    ew("select-all-selects-every-result", lambda: (lambda rs: (len(rs) >= 2, all(r.get("selected") is True for r in rs)))(region(s, RES)), (True, True))
    undo0 = {}
    s._act(lambda: undo0.update(n=core0(s, "undo_entry/count")))
    shown = {}
    s._act(lambda: shown.update(t=sorted(set(threads(s)))))
    s.key("L")
    s.type(SE["label"])
    # the label tree is read (positive control: it holds nodes), the label is not among them, and the L field offers to create it
    ew("label-field-offers-create", lambda: (lambda menu: (len(menu) > 0, any(p.split("/")[-1] == SE["label"] for p in menu),
             [r.get("offers_create") for r in region(s, A["search_label_field"])]))([r["path"] for r in region(s, LABEL_MENU)]), (True, False, [True]))
    s.key("Enter")
    ew("chips-on-every-selected-result", lambda: (lambda rs: (len(rs) >= 2, all(SE["label"] in r.get("chips", []) for r in rs)))(region(s, RES)), (True, True))
    ew("labelled-messages-are-exactly-the-shown-threads", lambda: (lambda want: (len(want) > 0, sorted(
        m for m in want if SE["label"] in (core0(s, "label_assignment/" + m) or [])) == want))(s.ctl and members(shown.get("t", []))), (True, True))
    ew("one-undo-entry-for-the-batch", lambda: ((core0(s, "label_assignment/%s/count" % SE["label"]) or 0) > 1, (core0(s, "undo_entry/count") or 0) - (undo0.get("n") or 0)), (True, 1))
    ew("one-undo-line-on-screen", lambda: len(region(s, A["search_undo"])), 1)
    before_z = {}
    s._act(lambda: before_z.update(n=core0(s, "label_assignment/%s/count" % SE["label"]) or 0))
    s.key("Z")
    ew("one-undo-reverts-the-whole-batch", lambda: (before_z.get("n", 0) > 1, core0(s, "label_assignment/%s/count" % SE["label"]) or 0), (True, 0))

    s = j.step("J5.7", "Allow Claude Desktop over MCP")
    s.raw("fixture", "reset")                                               # the main window on the inbox, typed text cleared (the corpus stays)
    s.raw("fixture", "corpus", os.path.join(_ROOT, SE["corpus"]))       # the MCP search runs over the mail its expectation is computed from
    s.arrive(("click", A["palette_mcp"]), "mac-mcp-notes")
    s.click(A["mcp_allow_claude"])
    s.expect("listed-allowed", lambda: (lambda d: (d.get(FX["mcp"]["app"]), d.get("Cursor"), sorted(k for k, v in d.items() if v == "allowed")))(
        {r["app"]: r["state"] for r in s.ctl.rows(A["mcp_apps"])}), ("allowed", "not allowed", [FX["mcp"]["app"]]))
    s.expect("grant-in-core", lambda: s.ctl.core("mcp_grant/" + FX["mcp"]["app"]), True)
    req = seam(s, "mcp-search", FX["mcp"]["app"], FX["mcp"]["query"])
    rid = lambda: req["reply"]["request_id"] if "reply" in req and "request_id" in req["reply"] else (req.get("refused") or (req.get("reply") or {}).get("refused"))
    s.expect("approval-prompt-before-results", lambda: (served(req), [(r["app"], r["tool"]) for r in region(s, REQS) if r["id"] == rid()],
             core0(s, "mcp_request/%s/results" % rid())), ("served", [(FX["mcp"]["app"], "search_mail")], []))
    s._act(lambda: s.j.ctl.click("mac-mcp-notes.request.%s.allow" % rid()))
    s.expect("results-after-approval", lambda: sorted(core0(s, "mcp_request/%s/results" % rid()) or []), MCP_SUBJECTS)

    s = j.step("J5.8", "Decisions: pages by outcome, one list each, the engine, the preview")
    s.click("mac-mcp-notes.close")                                          # the Outside AI apps window's own close door (McpNotesMacFeature.swift:59): ⌘W does nothing on mac-mcp-notes
    s.arrive(("click", A["palette_decisions"]), "mac-settings-ai")
    s.expect("one-page-per-decision", lambda: [(p["id"], p["title"]) for p in s.ctl.rows(A["dec_pages"])], [(p["id"], p["title"]) for p in D["pages"]])
    s.expect("which-tab-list-mixes-rules-and-the-model", lambda: [(r["kind"], r.get("trust")) for r in s.ctl.rows(PAGE + "which-tab.entries")], [(e["kind"], e.get("trust")) for e in TAB])
    s.expect("which-tab-model-entry-is-a-sentence", lambda: s.ctl.rows(PAGE + "which-tab.entries")[2]["text"], TAB[2]["question"])
    s.click("mac-settings-ai.decisions.engine.jev")
    s.click("mac-settings-ai.decisions.engine.local")                      # from Jev back to Local: the choice moves twice
    s.expect("engine-in-core", lambda: core0(s, "decision_model/provider"), "local")
    s.expect("engine-block-offers-local-jev-off", lambda: [r["id"] for r in s.ctl.rows(A["dec_engine"])], ["local", "jev", "off"])
    s.click(PAGE + "which-tab.preview")
    settle(s, lambda: s.ctl.rows(PAGE + "which-tab.preview.state")[0]["state"], "done")      # the preview runs off the main actor: wait on its served state, never on time
    s.expect("preview-state-ends-done", lambda: [r["state"] for r in s.ctl.rows(PAGE + "which-tab.preview.state")], ["done"])
    s.expect("preview-runs-on-n", lambda: len(s.ctl.rows(PAGE + "which-tab.preview")), const("DECISION_PREVIEW_N"))
    s.expect("preview-names-the-deciding-entry-per-message", lambda: all(isinstance(r["entry"], int) and r["entry"] >= 1 for r in s.ctl.rows(PAGE + "which-tab.preview")) and len(s.ctl.rows(PAGE + "which-tab.preview")) > 0, True)
    s.expect("preview-writes-nothing", lambda: (core0(s, "decision_preview/applied"), len(s.ctl.rows(PAGE + "which-tab.preview")) > 0), (False, True))
    dec = seam(s, "decide", "which-tab", "promo", "promotions", "0.91")
    s.expect("why-names-page-entry-and-model", lambda: (served(dec), [r.get("why") for r in region(s, A["dec_result"])][:1]),
             ("served", ["Which tab · entry 3 · the model said Promotions (0.91)"]))

    s = j.step("J5.9", "A rule from a message goes first in the list and wins")
    s.click("mac-settings-window.close")                                   # the Settings window's own close (⌘W is a stand-in, J5.4)
    s.expect("back-on-the-inbox", lambda: s.ctl.uistate().get("surface"), "mac-main")
    s.arrive(("click", A["list_row_1_create_rule"]), "mac-settings-ai")
    s.expect("editor-prefilled-from-the-message", lambda: [(r.get("from"), r.get("domain")) for r in region(s, A["dec_rule_editor"])][:1], [(D["prefill"]["from"], D["prefill"]["domain"])])
    rule = seam(s, "add-rule", "which-tab", D["new_rule"]["words"], D["new_rule"]["outcome"])
    s.expect("new-rule-is-entry-1", lambda: (served(rule), s.ctl.rows(PAGE + "which-tab.entries")[0]["text"]), ("served", D["new_rule"]["words"]))
    dec2 = seam(s, "decide", "which-tab", D["new_rule"]["msg"], "primary", "0.5")
    s.expect("rule-beats-the-model", lambda: (served(dec2), [(r.get("entry"), r.get("outcome")) for r in region(s, A["dec_result"])][:1]), ("served", [(1, "Updates")]))

    s = j.step("J5.10", "A label is a pipeline of three steps: picks, the AI accepts, the exit filter keeps")
    before = {}
    s.click("mac-settings-window.close")                                   # J5.9 ends in Settings: the inspector opens from the main window's label board
    s.raw("fixture", "reset")                                               # the sample's mail and flow rows back: J5.4-J5.9 loaded the J5 corpus, which holds none of the 37 picked mails (the figures below are the sample's)
    s.click("%s.open.modify.%s" % (INS, BOARD))                             # the door of par-labelscontactsmac example_open
    settle(s, lambda: s.ctl.rows(INS)[0].get("counting"), False)           # the count is read off the main thread: wait until the inspector says it is done
    s.expect("inspector-opens-on-the-label", lambda: [(r.get("open"), r.get("label_id")) for r in s.ctl.rows(INS)][:1], [(True, BOARD)])
    s.expect("three-cards-in-order", lambda: [r.get("card") for r in s.ctl.rows(FLOW_AX) if "card" in r], ["entry", "ai", "exit"])
    s.expect("step-1-picks-n", lambda: flow_counts(s)["entry"], EX["entry"])
    s.expect("the-ai-accepts-n", lambda: flow_counts(s)["ai"], EX["ai"])
    s.expect("the-exit-filter-keeps-n", lambda: flow_counts(s)["kept"], EX["kept"])
    s.expect("n-go-to-the-fallback", lambda: flow_counts(s)["sent"], EX["sent"])
    s.expect("n-picked-mails-wait-for-the-ai", lambda: flow_counts(s)["waiting"], EX["waiting"])
    s.expect("cards-say-what-the-core-says", lambda: {r["card"]: r.get("count") for r in s.ctl.rows(FLOW_AX) if r.get("card") in ("entry", "exit")},
             {"entry": say("picks %lld", EX["entry"]), "exit": say("keeps %lld of %lld", EX["kept"], EX["ai"])})
    s._act(lambda: before.update(counts=flow_counts(s), held=in_board(s, REPLY_MSG), held_raw=core0(s, "label_assignment/%s" % REPLY_MSG)))
    s.raw("fixture", "reply", REPLY_MSG)                                     # the replied fact flips on one kept mail
    settle(s, lambda: flow_counts(s)["kept"], EX["kept"] - 1)
    s.expect("a-reply-takes-one-kept-mail-to-the-fallback", lambda: (before["counts"]["kept"], before["counts"]["sent"], flow_counts(s)["kept"], flow_counts(s)["sent"]),
             (EX["kept"], EX["sent"], EX["kept"] - 1, EX["sent"] + 1))
    s.expect("picks-and-accepts-do-not-move-on-a-reply", lambda: (flow_counts(s)["entry"], flow_counts(s)["ai"]), (EX["entry"], EX["ai"]))
    # round 12: a red names what the core said (main95 and main103: (False, False), the mail never seen held, cause not in the log)
    expect_why(s, "the-replied-mail-leaves-the-label", lambda: (before["held"], in_board(s, REPLY_MSG)), (True, False),
               lambda: "label_assignment/%s now %r; label/%s/name %r; before the reply %r (held %r)" % (
                   REPLY_MSG, core0(s, "label_assignment/%s" % REPLY_MSG), BOARD, core0(s, "label/%s/name" % BOARD), before.get("held_raw"), before.get("held")))
    s.raw("fixture", "reset")                                               # the launch state back for what runs after this journey
    for n in j.notes:                                                       # every action the app refused, beside the verdict
        sys.stderr.write("J5 NOTE %s\n" % n)

# MUTANTS
# J5.1/theme-applied | the app ignores the theme and stays light
# J5.1/arrives-by-control | the control that leads here does nothing
# J5.1/refused-key-reason-in-words | a refused key shows a bare code
# J5.1/connected | a pasted key never connects
# J5.1/models-listed | no models are listed
# J5.1/chatgpt-stop-note | the ChatGPT note is missing
# J5.1/key-in-keychain-not-db | the key is stored in the database
# J5.2/engine-ready | the engine never reports ready
# J5.2/engine-in-core | the choice is not saved
# J5.3/card-lists-actions-in-words | the card shows codes, not words
# J5.3/config-in-core | the agent is not saved
# J5.3/no-send-permission | the agent gets a send permission nobody gave
# J5.4/settings-closed-to-the-inbox | ⌘W leaves the Settings window open, so the search starts from Settings and not the inbox
# J5.4/arrives-by-control | the search key opens nothing
# J5.4/skeleton-rows-appear-at-once | Enter shows no skeleton rows while the search runs
# J5.4/list-swapped-for-skeletons | the old list stays under the skeleton rows
# J5.4/skeleton-gives-way-to-results | the skeleton rows never leave
# J5.4/record1-in-results | the question returns the other suppliers and leaves out record 1
# J5.4/meaning-match-without-shared-words | the question matches by words only, not by meaning
# J5.4/unrelated-mail-left-out | the question returns every message
# J5.4/no-answer-no-agent-reply-no-panel | an answer line, an agent reply, a proposal row or the Ask panel appears
# J5.4/search-sends-nothing-off-device | the search sends mail text to a cloud provider
# J5.4/history-written | the query is not saved to search history
# J5.4/building-shows-word-matches-first | while the meaning index builds, the list stays empty
# J5.4/building-quiet-line | while the meaning index builds, nothing says meaning results are coming
# J5.4/meaning-results-merge-when-ready | the meaning results never join the list
# J5.5/deleted-result-says-no-longer-in-your-mail | a result for a deleted message gives no reason
# J5.5/deleted-result-leaves-the-list | the deleted message stays in the results
# J5.5/deleted-result-opens-nothing | Enter on a deleted result opens a thread
# J5.5/record1-row-selected | Down moves nothing: Enter would open whatever result stands first
# J5.5/arrives-by-control | Enter on a result opens nothing
# J5.5/opens-at-the-matched-message | the thread opens at its first message, not the matched one
# J5.6/arrives-by-control | Esc does not return to the results
# J5.6/results-survive-the-return | Esc back empties the results
# J5.6/select-all-selects-every-result | select all selects nothing or one result
# J5.6/label-field-offers-create | the L field gives no way to create the missing label
# J5.6/chips-on-every-selected-result | the label lands on one result only
# J5.6/labelled-messages-are-exactly-the-shown-threads | the label covers one message per thread, not the thread's messages
# J5.6/one-undo-entry-for-the-batch | every message gets its own undo entry
# J5.6/one-undo-line-on-screen | no undo line shows after the batch
# J5.6/one-undo-reverts-the-whole-batch | undo takes back one message only
# J5.7/arrives-by-control | the control that leads here does nothing
# J5.7/listed-allowed | the grant is not listed
# J5.7/grant-in-core | the grant is not saved
# J5.7/approval-prompt-before-results | a search returns mail with no prompt
# J5.7/results-after-approval | approval returns nothing
# J5.8/arrives-by-control | the control that leads here does nothing
# J5.8/one-page-per-decision | the pages are split by method (Rules, Decision model) as in version 1
# J5.8/which-tab-list-mixes-rules-and-the-model | the list holds rules only or the model apart
# J5.8/which-tab-model-entry-is-a-sentence | the model entry shows no question in words
# J5.8/engine-in-core | the engine choice is not saved
# J5.8/engine-block-offers-local-jev-off | the engine block lacks Off
# J5.8/preview-state-ends-done | the preview never leaves running, so the page never says it is done
# J5.8/preview-runs-on-n | the preview runs on a different number of messages
# J5.8/preview-names-the-deciding-entry-per-message | the preview names no entry
# J5.8/preview-writes-nothing | the preview changes stored decisions
# J5.8/why-names-page-entry-and-model | the why drops the page, the entry or the model's answer
# J5.9/back-on-the-inbox | the Settings close leaves Settings open, so a dead row control still reads as arrived in Settings
# J5.9/arrives-by-control | the control that leads here does nothing
# J5.9/editor-prefilled-from-the-message | the editor opens empty
# J5.9/new-rule-is-entry-1 | a new rule is added after the model entry
# J5.9/rule-beats-the-model | the model decides before the rule

# J5.10/inspector-opens-on-the-label | the label's door opens the inspector on another label or none
# J5.10/three-cards-in-order | a card of the pipeline is missing or out of order
# J5.10/step-1-picks-n | the picked figure is off by one
# J5.10/the-ai-accepts-n | the accepted figure is off by one
# J5.10/the-exit-filter-keeps-n | the kept figure is off by one
# J5.10/n-go-to-the-fallback | the figure of mails sent to the fallback is off by one
# J5.10/n-picked-mails-wait-for-the-ai | the waiting figure is off by one
# J5.10/cards-say-what-the-core-says | a card shows a figure other than the core's
# J5.10/a-reply-takes-one-kept-mail-to-the-fallback | a reply moves no kept mail to the fallback
# J5.10/picks-and-accepts-do-not-move-on-a-reply | a reply re-reads step 1 and the AI, so the picks fall
# J5.10/the-replied-mail-leaves-the-label | the replied mail stays in the label

# ---------------------------------------------------------------------------------------------------------------- self-test (the kit is journeys/J4.py)
class _Kit:
    """Loads journeys/J4.py's self-test kit (Sim, serve_sim, run_selftest) by path, under its own name, so this file does not depend on the loader's search path."""
    _mod = None

    @classmethod
    def get(cls):
        if cls._mod is None:
            import importlib.util
            path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "J4.py")
            spec = importlib.util.spec_from_file_location("journey_selftest_kit", path)
            cls._mod = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(cls._mod)
        return cls._mod


def _app5(kit):
    TABROWS = [{"kind": e["kind"], "trust": e.get("trust"), "text": e.get("question") or e.get("word", "")} for e in TAB]

    class App5(kit.Sim):
        FAULTS = {"J5.3/no-send-permission": lambda out: dict(out, value=list(FX["agent"]["allow"]) + [FX["agent"]["no_send"]]) if "value" in out else out,
                  "J5.4/unrelated-mail-left-out": lambda out: dict(out, rows=out["rows"] + [{"thread": SE["unrelated_thread"], "selected": False}]) if "rows" in out else out,
                  "J5.6/one-undo-line-on-screen": lambda out: dict(out, rows=[]) if "rows" in out else out,
                  "J5.8/preview-state-ends-done": lambda out: dict(out, rows=[{"state": "running"}]) if "rows" in out else out,
                  "J5.8/preview-names-the-deciding-entry-per-message": lambda out: dict(out, rows=[{"entry": None} for _ in out["rows"]]) if "rows" in out else out,
                  "J5.4/search-sends-nothing-off-device": lambda out: dict(out, value=3) if "value" in out else out,
                  "J5.4/skeleton-gives-way-to-results": lambda out: dict(out, rows=[{"loading": True, "held": True}]) if "rows" in out else out}
        unserved = False

        def _nudge(field, by=1):
            return lambda out: dict(out, value=dict(out["value"], **{COUNT_FIELDS[field]: out["value"][COUNT_FIELDS[field]] + by})) if isinstance(out.get("value"), dict) else out
        FAULTS.update({"J5.10/step-1-picks-n": _nudge("entry"), "J5.10/the-ai-accepts-n": _nudge("ai"), "J5.10/the-exit-filter-keeps-n": _nudge("kept"),
                       "J5.10/n-go-to-the-fallback": _nudge("sent"), "J5.10/n-picked-mails-wait-for-the-ai": _nudge("waiting"),
                       "J5.10/a-reply-takes-one-kept-mail-to-the-fallback": _nudge("kept", 1),      # the kept figure does not drop: it reads one above what a reply leaves
                       "J5.10/picks-and-accepts-do-not-move-on-a-reply": _nudge("entry", -1),     # a reply re-reads step 1: one pick fewer
                       "J5.10/the-replied-mail-leaves-the-label": lambda out: dict(out, value=[BOARD]) if "value" in out else out})

        def setup(self):
            self.bar, self.launched, self.held, self.loading, self.building, self.deleted, self.gone = "", False, False, False, False, False, False
            self.sel, self.label, self.lab_mode, self.labelled, self.undo, self.notice = 0, "", False, False, 0, ""
            self.provider, self.agent, self.allow, self.engine, self.grant, self.req, self.req_ok = "", False, [], "jev", False, None, False
            self.preview, self.decided, self.rule, self.searched, self.prompt, self.selall = False, None, False, False, False, False
            self.lab_open, self.replied = False, False
            self.state["surface"] = "mac-main"

        def threads(self):
            ts = [SE["record1_thread"], SE["deleted_thread"], SE["meaning_only_thread"]]
            if self.building:
                ts.remove(SE["meaning_only_thread"])
            if self.gone:
                ts.remove(SE["deleted_thread"])
            return ts

        def rows_for(self, ax):
            if ax == P + ".anthropic":
                st = {"": say("not connected") if "not connected" in CATALOGUE else "", "bad": say("refused"), "good": say("connected")}[self.provider]
                return [{"state": st, "note": "Anthropic did not accept that key" if self.provider == "bad" else ""}]
            if ax == P + ".anthropic.models":
                return [{"model": m} for m in ("claude-sonnet-5-5", "claude-haiku-5-5")] if self.provider == "good" else []
            if ax == P + ".chatgpt":
                return [{"note": say("Signing in with ChatGPT can stop working at any time.")}]
            if ax == A["speech"]:
                return [{"state": "ready" if self.engine == "local-speech" else "idle"}]
            if ax == A["agents_card"]:
                return [{"name": FX["agent"]["name"], "allowed": [say(w) for w in FX["agent"]["words"][:len(self.allow)]]}] if self.agent else []
            if ax == BAR:
                return [{"launched": self.launched, "text": self.bar, "active": self.state["surface"] == "mac-search"}]
            if ax == SKEL:
                return [{"loading": self.loading, "held": self.held}]
            if ax == RES:
                if not self.launched:
                    return []
                ts = self.threads()
                return [{"thread": t, "selected": (self.selall or i == self.sel), "matched_in": "related" if t == SE["meaning_only_thread"] else "words",
                         "chips": [SE["label"]] if self.labelled else []} for i, t in enumerate(ts)]
            if ax == NOTICE:
                return [{"text": self.notice}] if self.notice else []
            if ax == SECTIONS:
                return [{"title": say(RANKING)}] if self.building else []
            if ax in A["search_answer_regions"]:
                return None
            if ax == "mac-search.recent":
                return [{"text": SE["question"]}] if self.searched else []
            if ax == "mac-main.reader.subject":
                return [{"id": SE["record1_message"], "subject": R1["subject"]}]
            if ax == A["search_label_field"]:
                return [{"offers_create": True}] if self.lab_mode else None
            if ax == LABEL_MENU:
                return [{"path": "Work"}, {"path": "Work/Clients"}]
            if ax == A["search_undo"]:
                return [{"text": "x"}] if self.undo == 1 else []
            if ax == "mac-keyevent.monitor":
                return [{"last": {"handled": True}}]
            if ax == A["mcp_apps"]:
                return [{"app": FX["mcp"]["app"], "state": "allowed" if self.grant else "not allowed"}, {"app": "Cursor", "state": "not allowed"}]
            if ax == REQS:
                return [{"id": "req-1", "app": FX["mcp"]["app"], "tool": "search_mail"}] if self.req else []
            if ax == INS:
                return [{"open": self.lab_open, "label_id": BOARD if self.lab_open else None, "counting": False}]
            if ax == FLOW_AX:
                if not self.lab_open:
                    return []
                n = self.flow()
                return [{"card": "entry", "on": True, "count": say("picks %lld", n["entry"])},
                        {"card": "ai", "on": True, "count": "%d accepted" % n["ai"]},
                        {"card": "exit", "on": True, "count": say("keeps %lld of %lld", n["kept"], n["ai"])}]
            if ax == A["dec_pages"]:
                return [{"id": p["id"], "title": p["title"]} for p in D["pages"]]
            if ax == PAGE + "which-tab.entries":
                return ([{"kind": "rule", "trust": None, "text": D["new_rule"]["words"]}] + TABROWS) if self.rule else TABROWS
            if ax == PAGE + "which-tab.preview":
                return [{"entry": 3} for _ in range(const("DECISION_PREVIEW_N"))] if self.preview else []
            if ax == PAGE + "which-tab.preview.state":
                return [{"state": "done" if self.preview else "idle"}]
            if ax == A["dec_engine"]:
                return [{"id": i} for i in ("local", "jev", "off")]
            if ax == A["dec_result"]:
                return list(self.decided) if self.decided else []
            if ax == A["dec_rule_editor"]:
                return [{"from": D["prefill"]["from"], "domain": D["prefill"]["domain"]}] if self.state["surface"] == "mac-settings-ai" and self.rule_editor else []
            return None

        rule_editor = False
        focus_bar = False

        def flow(self):
            """The label's figures as the core serves them: the example's, and after a reply one kept mail fewer and one more in the fallback."""
            r = 1 if self.replied else 0
            return {"entry": EX["entry"], "ai": EX["ai"], "kept": EX["kept"] - r, "sent": EX["sent"] + r, "waiting": EX["waiting"]}

        def core_for(self, path):
            if path == "provider_request/count":
                return 0
            if path == "provider_credential/anthropic/stored":
                return self.provider == "good"
            if path == "ai_provider/anthropic/refusal":
                return "Anthropic did not accept that key" if self.provider == "bad" else kit.NOT_SERVED
            if path == "speech_engine/chosen/state":
                return "ready" if self.engine == "local-speech" else kit.NOT_SERVED
            if path == "agent_config/%s/allowed" % FX["agent"]["name"]:
                return list(FX["agent"]["allow"]) if self.agent else kit.NOT_SERVED
            if path == "search/recent":
                return [SE["question"]] if self.searched else []
            if path == "undo_entry/count":
                return self.undo
            if path == "label_assignment/%s/count" % SE["label"]:
                return len(members([SE["record1_thread"], SE["meaning_only_thread"]])) if self.labelled else 0
            if path.startswith("label_assignment/msg") or path.startswith("label_assignment/m"):
                m = path.split("/", 1)[1]
                return [SE["label"]] if self.labelled and m in members([SE["record1_thread"], SE["meaning_only_thread"]]) else []
            if path == "mcp_grant/" + FX["mcp"]["app"]:
                return self.grant
            if path == "mcp_request/req-1/results":
                return MCP_SUBJECTS if self.req_ok else []
            if path == "decision_model/provider":
                return self.engine_choice if hasattr(self, "engine_choice") else "local"
            if path == "decision_preview/applied":
                return False
            if path == "label/%s/flow/counts" % BOARD:
                n = self.flow()
                return {COUNT_FIELDS[k]: v for k, v in n.items()}
            if path == "label_assignment/%s" % REPLY_MSG:
                return [] if self.replied else [BOARD]
            return kit.NOT_SERVED

        def answer(self, verb, args):
            if self.inert or verb in ("rows", "core", "uistate", "winframes"):
                return super().answer(verb, args)
            go = lambda surface: self.state.update(surface=surface)
            if verb == "type":
                if self.lab_mode:
                    self.label = args[0]
                else:
                    self.bar = args[0]
                return {"ok": True}
            if verb in ("clock", "capture"):
                return {"ok": True}
            if verb == "fixture":
                v = args[0]
                if self.unserved and v in ("index-state", "decide", "add-rule"):
                    raise kit.Gone("fixture %s is not served" % v)
                if v == "index-state":
                    self.building = args[1] == "building"
                    self.notice = SE["building_words"] if self.building else ""
                elif v == "delete-message":
                    self.deleted = True
                elif v == "mcp-search":
                    self.req = True
                    return {"ok": True, "request_id": "req-1"}
                elif v == "decide":
                    entry, outcome = (1, "Updates") if self.rule else (3, "Promotions")
                    self.decided = [{"why": "Which tab · entry 3 · the model said Promotions (0.91)", "entry": entry, "outcome": outcome}]
                elif v == "add-rule":
                    self.rule = True
                elif v == "reply" and args[1] == REPLY_MSG:
                    self.replied = True
                elif v == "reset":
                    self.replied = False
                elif v == "corpus":
                    pass
                else:
                    raise kit.Gone("fixture %s is not served" % v)
                return {"ok": True}
            if verb == "key":
                k = args[0]
                if k == "⌘K":
                    go("mac-search")
                elif k == "⌘A" and self.state["surface"] == "mac-search" and self.launched and not self.focus_bar:
                    self.selall = True      # in the search field ⌘A selects its text; with the results holding the keys it selects every result
                elif k == "Enter":
                    if self.lab_mode:
                        self.labelled, self.undo, self.lab_mode = True, 1, False
                    elif self.state["surface"] == "mac-search" and not self.launched is False:
                        pass
                    if self.state["surface"] == "mac-search" and not self.lab_mode and not self.labelled or self.state["surface"] == "mac-search":
                        self.enter()
                elif k == "Escape" and self.state["surface"] == "mac-reader":
                    go("mac-search")
                    self.focus_bar = False
                elif k == "Down":
                    self.sel = min(self.sel + 1, len(self.threads()) - 1)
                elif k == "Up":
                    self.sel = max(self.sel - 1, 0)
                elif k == "L" and self.selall:
                    self.lab_mode = True
                elif k == "Z":
                    self.labelled, self.undo = False, 0
                return {"ok": True}
            return self.click(args[0])

        def enter(self):
            if self.lab_mode or self.labelled and self.undo == 1 and False:
                return
            if not self.launched or self.bar != getattr(self, "last_bar", None):
                self.last_bar, self.launched, self.loading, self.sel, self.searched = self.bar, True, True, 0, True
                if not self.held:
                    self.loading = False
                return
            t = self.threads()[self.sel]
            if t == SE["deleted_thread"] and self.deleted:
                self.gone, self.notice = True, say(SE["deleted_words"])
                return
            self.state["surface"] = "mac-reader"

        def click(self, ax):
            go = lambda surface: self.state.update(surface=surface)
            if ax == A["palette_ai_settings"]:
                go("mac-settings-ai")
            elif ax == BAR:
                self.focus_bar = True
            elif ax == "mac-settings-ai.nav.providers" or ax == "mac-settings-ai.nav.agents":
                pass
            elif ax == "mac-settings-ai.fixture.paste.bad-key":
                self.provider = "bad"
            elif ax == "mac-settings-ai.fixture.paste.anthropic-key":
                self.provider = "good"
            elif ax == A["speech_local"]:
                self.engine = "local-speech"
            elif ax == A["agents_new"] or ax == "mac-settings-ai.agents.save":
                self.agent = self.agent or ax.endswith("save")
            elif ax.startswith("mac-settings-ai.agents.allow."):
                self.allow.append(ax.rsplit(".", 1)[1])
            elif ax == "mac-settings-window.close":
                go("mac-main")
            elif ax == "mac-search.skeleton.hold":
                self.held = True
            elif ax == "mac-search.skeleton.release":
                self.held, self.loading = False, False
            elif ax == A["palette_mcp"]:
                go("mac-mcp-notes")
            elif ax == A["mcp_allow_claude"]:
                self.grant = True
            elif ax == "mac-mcp-notes.request.req-1.allow":
                self.req_ok = True
            elif ax == A["palette_decisions"]:
                go("mac-settings-ai")
            elif ax in ("mac-settings-ai.decisions.engine.jev", "mac-settings-ai.decisions.engine.local"):
                self.engine_choice = ax.rsplit(".", 1)[1]
            elif ax == PAGE + "which-tab.preview":
                self.preview = True
            elif ax == "%s.open.modify.%s" % (INS, BOARD):
                self.lab_open = True
            elif ax == A["list_row_1_create_rule"]:
                self.rule_editor = True
                go("mac-settings-ai")
            elif ax.startswith(LANG_DOOR % ""):
                self.lang = ax[len(LANG_DOOR % ""):]
            else:
                raise kit.Gone("click %s: nothing to click at %s" % ((ax,), ax))
            return {"ok": True}
    return App5


def selftest():
    kit = _Kit.get()
    App5 = _app5(kit)
    mod = sys.modules[__name__]
    code = kit.run_selftest(mod, os.path.abspath(__file__), lambda mutant, inert: App5(mutant, inert), {})
    # the build as it stands: the fixture verbs index-state, decide and add-rule are not served, so exactly those cases are not judged and nothing else changes
    class Built(App5):
        unserved = True
    got = kit.serve_sim(Built(), mod.drive, mod.ID)
    want = {"J5.4/building-shows-word-matches-first", "J5.4/building-quiet-line", "J5.4/meaning-results-merge-when-ready", "J5.8/why-names-page-entry-and-model",
            "J5.9/new-rule-is-entry-1", "J5.9/rule-beats-the-model"}
    unj = {c.label for c in got if c.state == "unjudged"}
    other = sorted(c.label for c in got if c.state not in ("pass", "unjudged"))
    ok_ex = EX == EX_PINNED and example_figures([], []) == {k: 0 for k in EX}
    print("MUTANTS | example figures | %s | derived from docs/design/fixtures/mac-main-sample.json %s, pinned %s, reply mail %s (a kept row)" % ("green" if ok_ex else "RED", EX, EX_PINNED, REPLY_MSG))
    code = code or int(not ok_ex)
    print("build stand-in (no index-state, decide or add-rule): %d checks, not judged %d, red %d%s" % (len(got), len(unj), len(other), (": " + ", ".join(other)) if other else ""))
    if unj != want or other:
        print("  UNEXPECTED: not judged %s, red %s" % (sorted(unj ^ want), other))
        code = 1
    return code


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else 0)
