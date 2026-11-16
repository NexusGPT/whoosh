"""J10: Get a reply drafted in your own voice, alone or on request (docs/JOURNEYS.md J10.1-J10.10).
Written from the J10 step table, docs/lanes/replies-contract.md, docs/lanes/reply-thread.md and fixtures/tests-j10.json, never from a built screen.
Every expected value comes from that fixture file (each names its source there). Rows are read by id, never by a translated label.

    python3 journeys/run.py J10            drive the running rig copy ($WHOOSH_SOCKET_DIR, --fixture-core)
    python3 journeys/run.py J10 --dry      same code, no socket: every check not judged
    python3 journeys/J10.py --selftest     the whole journey against a stand-in app: clean (two kinds), then one mutant per check, and an app that does nothing

Nothing is ever sent: the journey says "nothing is sent alone", and the driver never presses Send (no cmd-return). It reads the outbox count
(`core outbox_entry/count`) before J10.8 and asserts it did not move after the arrival, the draft, the take-over, the on-demand draft and the change.

Doors used (all registered in Sources, read in the lane notes): the first start `mac-first-start.*` and the step's rows `mac-welcome-accounts.replies.*`;
the Settings sidebar `mac-settings-window.pane-row.replies.<part>` and the pane's doors `mac-settings-replies.*` (settingsreplies-check.py); the
quick reply `mac-main.quickreply.*` and its AI doors (par-quickreply.py, reply-thread.md). The Settings text editors are typed through the `keyevent` verb.

The arrival (J10.8): `fixture deliver` -> FixtureFiling.deliverArrival -> FixtureCoreArrivalDraft.swift writeArrivalDraftLocked runs the three steps of the auto use
on the arrival and, when all pass, stores a live draft (made_by auto, state proposed, instruction None) served at `reply/draft/<arrival thread>`, as the real core does
(core/crates/engine/src/replies_work/usecases/arrival_pass.rs). The check `J10.8/arrival-writes-a-draft-alone` reads it; a core that serves none leaves the check
NOT JUDGED (an absent value is an Unreadable, never a fail and never a pass); it is green on a core that drafts on arrival, and nothing is sent."""
import json
import os
import sys
import time
from lib.ctl import Unreadable
from lib.fixture import ROOT, FX

ID = "J10"
TITLE = "Get a reply drafted in your own voice, alone or on request"
TARGET = "LOCAL"
VISIBLE_OUTCOME = ("The first start's Replies step writes the owner's three choices; the Settings panes show and write the sources, the voice, the automatic draft, "
                   "the three label cards, the retrieval and the prompt; a mail that passes the three steps arrives with a draft marked as the AI's and nothing sent; "
                   "cmd-J writes a draft from one instruction and the change field rewrites it in place")

FX_PATH = os.path.join(ROOT, "fixtures", "tests-j10.json")
with open(FX_PATH, encoding="utf-8") as _f:
    F10 = json.load(_f)
FS_, SRC_, CNT, VOI, AUT, FIL, RET, PRM, RPL = (F10[k] for k in ("first_start", "sources", "counts", "voice", "auto", "filter", "retrieval", "prompt", "reply"))
ARR = F10["arrivals"]["needs-reply"]
ACC1 = SRC_["first_account"]

FS = "mac-first-start"
WR = "mac-welcome-accounts.replies"
SW = "mac-settings-window"
R = "mac-settings-replies"
QR = "mac-main.quickreply"
FIELD, AI = QR + ".field", QR + ".ai"
AI_INSTR, AI_CHANGE, AI_DISCARD = AI + ".instruction", AI + ".change", AI + ".discard"
OUTBOX = "outbox_entry/count"

SETTLE = 1.0          # the waits of the drive body are multiplied by this (the self-test shortens them)
GAP_WAIT_S = 4.0      # how long the arrival check waits for a draft before it says the core drafts nothing on arrival
GAP_ARRIVAL = ("the core served no draft for the arrival within the wait: the fixture core writes one in FixtureCoreArrivalDraft.swift writeArrivalDraftLocked when the auto use is on "
               "and the arrival passes the three steps; the draft seeded on %s is not the arrival's" % RPL["seeded_thread"])


# ---------------------------------------------------------------------------------------------------------------- reads (inside thunks, through s.ctl)
def rows(s, ax):
    return s.ctl.rows(ax)


def core(s, path):
    return s.ctl.core(path)


def absent(s, path):
    """True when the core serves no value at the path (the verb answers ok:false "no core value at ..."); any other failure stays Unreadable."""
    ctl = s.ctl
    try:
        ctl.core(path)
        return False
    except Unreadable as e:
        if "no core value" in str(e):
            return True
        raise


def state(s):
    return rows(s, R + ".state")[0]


def place(s):
    p = rows(s, SW + ".place")[0]
    return (p["pane"], p["reply"])


def outbox(s):
    return int(core(s, OUTBOX))


def role(name, roles=FS_["folder_roles"]):
    leaf = name.rsplit("/", 1)[-1].lower()
    for r, names in roles.items():
        if leaf in names:
            return r
    return None


def folders(s):
    """Every mail folder as the core serves it: [(account, folder, enabled)]."""
    return [(f["source"]["account_id"], f["source"]["folder"], f["enabled"]) for a in core(s, "reply/sources")["accounts"] for f in a["folders"]]


def role_states(fs):
    out = {"inbox": set(), "sent": set()}
    for _a, name, on in fs:
        r = role(name)
        if r:
            out[r].add(on)
    return {k: sorted(v) for k, v in out.items()}


def others(fs):
    return sorted((a, n, on) for a, n, on in fs if not role(n))


def editors(s):
    return {r["key"]: r for r in rows(s, R + ".editors")}


def ai(s):
    return rows(s, AI)[0]


def qfield(s, key):
    return rows(s, FIELD)[0][key]


MONITOR = "mac-keyevent.monitor"


def what_the_app_said(snap, row):
    """The words a failed typing case carries: the key verb's last report (`rows mac-keyevent.monitor` field `last`, read right after the keyevent) and the editor's row.
    Fields an older build does not serve read "absent"; nothing here raises."""
    last = snap[0].get("last") if isinstance(snap, list) and snap and isinstance(snap[0], dict) else None
    last = last if isinstance(last, dict) else {}
    keys = ("window_id", "focus", "handled", "windows", "text_before", "text_after")
    said = {k: last.get(k, "absent") for k in keys}
    if not last:
        said["last"] = "absent"
    tail = row.get("text")
    return "app said: keyevent %r; editor focused=%r dirty=%r last3=%r" % (said, row.get("focused", "absent"), row.get("dirty", "absent"), tail[-3:] if isinstance(tail, str) else "absent")


def typing_check(s, name, key, typed, snap):
    """The pass condition is the same as before: the editor is dirty and its text ends with the letter. A failure also says what the key verb reported."""
    def run():
        row = editors(s)[key]
        got = (row["dirty"], row["text"].endswith(typed))
        return got == (True, True), "expected %r, got %r; %s" % ((True, True), got, what_the_app_said(snap, row))
    s._record(name, run)


def live_draft(s, thread):
    return core(s, "reply/draft/%s" % thread)


# ---------------------------------------------------------------------------------------------------------------- the journey
def drive(j):
    live = j.ctl is not None

    def ctl_try(fn):
        """A read in the drive body (never in a check): None when there is no app or the read fails."""
        try:
            return fn() if live else None
        except (Unreadable, KeyError, IndexError, TypeError, ValueError):
            return None

    def wait(sec):
        if live:
            time.sleep(sec * SETTLE)

    def settings_step(sid, title, part, arrives=True):
        s = j.step(sid, title)
        s.raw("fixture", "reset")
        s.click(R + ".fixture.reset")
        s.click(SW + ".open")
        s.click(SW + ".pane-row.replies." + part)
        if arrives:
            s.expect("arrives-by-control", lambda: place(s), ("replies", part))
        return s

    def focus_editor(s, key):
        """A click in the editor, retried until the editor holds the focus (a key typed before that would run as a command of the app)."""
        for _ in range(15):
            s.click("%s.editor.%s" % (R, key))
            if ctl_try(lambda: editors(s)[key]["focused"] is True):
                return
            wait(0.2)

    # --- J10.1 Quick start, step Replies: three choices, written when Continue runs
    s = j.step("J10.1", "Quick start, step Replies")
    s.raw("fixture", "reset")
    s.raw("fixture", "first-start", "on")
    for _ in range(10):
        if ctl_try(lambda: len(j.ctl.rows(FS + ".replies"))):
            break
        s.click(FS + ".continue")
    s.expect("arrives-by-continue", lambda: [r["id"] for r in rows(s, FS + ".replies")], [c[0] for c in FS_["choices_default"]])
    s.expect("three-choices-with-defaults", lambda: [(r["id"], r["chosen"], [o["id"] for o in r["options"]]) for r in rows(s, FS + ".replies")],
             [tuple(c) for c in FS_["choices_default"]])
    before = ctl_try(lambda: (folders(j), j.ctl.core("reply/use/auto")["enabled"], j.ctl.core("reply/voice")["voice"]["sample_size"]))
    pick = FS_["pick"]
    for row, opt in pick.items():
        s.click("%s.replies.%s.%s" % (FS, row, opt))
    s.expect("nothing-written-before-continue",
             lambda: ([r["chosen"] for r in rows(s, FS + ".replies")], core(s, "reply/use/auto")["enabled"], core(s, "reply/voice")["voice"]["sample_size"],
                      rows(s, WR + ".applied")[0]["applied"]),
             ([pick["draft"], pick["search"], pick["voice"]], FS_["auto_enabled_before"], FS_["sample_before"], False))
    s.click(FS + ".continue")
    s.expect("continue-writes-the-three-choices",
             lambda: (core(s, "reply/use/auto")["enabled"], core(s, "reply/voice")["voice"]["sample_size"], rows(s, WR + ".applied")[0]["writes"][0],
                      rows(s, WR + ".applied")[0]["writes"][-1]),
             (FS_["auto_enabled_after"], FS_["sample_after"], FS_["writes_first"], FS_["writes_last"]))
    s.expect("continue-writes-inbox-on-sent-off",
             lambda: (role_states(folders(s)), others(folders(s))),
             (FS_["folders_after_inbox_only"], others(before[0]) if before else None))
    s.expect("continue-leaves-the-step", lambda: (rows(s, FS + ".replies"), core(s, "reply/use/auto")["enabled"]), ([], FS_["auto_enabled_after"]))
    s.raw("fixture", "first-start", "off")

    # --- J10.2 Settings > Replies > Sources
    s = settings_step("J10.2", "Settings > Replies > Sources", "sources")
    s.raw("fixture", "calendar-access", "allowed")
    s.expect("one-card-per-account-with-tiles",
             lambda: ([(a["enabled_folders"], a["total_folders"]) for a in core(s, "reply/sources")["accounts"]],
                      (len([r for r in rows(s, R + ".sources") if r["kind"] == "account"]),
                       len([r for r in rows(s, R + ".sources") if r["kind"] == "mail_folder"]))),
             ([tuple(x) for x in SRC_["accounts"]], (len(SRC_["accounts"]), sum(x[1] for x in SRC_["accounts"]))))
    s.expect("header-counts-messages",
             lambda: sum(f["item_count"] for a in core(s, "reply/sources")["accounts"] if a["account_id"] == ACC1 for f in a["folders"] if f["enabled"]),
             SRC_["first_account_enabled_messages"])
    s.expect("folders-of-this-mac-listed",
             lambda: ([f["source"]["path"] for f in core(s, "reply/sources")["local_folders"]], [r["path"] for r in rows(s, R + ".sources") if r["kind"] == "local_folder"]),
             (SRC_["local_folders"], SRC_["local_folders"]))
    s.expect("calendar-switches-listed",
             lambda: ([(c["calendar_id"], c["name"], c["enabled"]) for c in core(s, "reply/sources")["calendars"]],
                      [(r["name"], r["enabled"]) for r in rows(s, R + ".sources") if r["kind"] == "calendar"]),
             ([tuple(c) for c in SRC_["calendars"]], [(c[1], c[2]) for c in SRC_["calendars"]]))
    sy = SRC_["syncing_folder"]
    s.expect("sync-progress-shown",
             lambda: [(f["progress"]["state"], f["progress"].get("percent")) for a in core(s, "reply/sources")["accounts"] for f in a["folders"]
                      if f["source"]["account_id"] == ACC1 and f["source"]["folder"] == sy["name"]],
             [("syncing", sy["percent"])])
    s.expect("sources-line-counts", lambda: core(s, "reply/sources")["enabled_total"], SRC_["enabled_total"])
    idx = ctl_try(lambda: [f["source"]["folder"] for a in j.ctl.core("reply/sources")["accounts"] if a["account_id"] == ACC1 for f in a["folders"]].index(SRC_["tick_folder"]))
    s.click("%s.sources.account.%s.folder.%s" % (R, ACC1, 0 if idx is None else idx))
    s.expect("tick-a-folder",
             lambda: ([f["enabled"] for a in core(s, "reply/sources")["accounts"] if a["account_id"] == ACC1 for f in a["folders"]
                       if f["source"]["folder"] == SRC_["tick_folder"]], core(s, "reply/sources")["enabled_total"]),
             ([True], SRC_["enabled_total"] + 1))
    s.click(R + ".sources.add-folder.sample")
    s.expect("add-a-folder-of-this-mac", lambda: [f["source"]["path"] for f in core(s, "reply/sources")["local_folders"]],
             SRC_["local_folders"] + [SRC_["added_folder"]])
    s.click("%s.sources.calendar.%s" % (R, SRC_["calendar_to_switch"]))
    s.expect("switch-a-calendar",
             lambda: ([(c["calendar_id"], c["enabled"]) for c in core(s, "reply/sources")["calendars"]],
                      [(r["name"], r["enabled"]) for r in rows(s, R + ".sources") if r["kind"] == "calendar"]),
             ([(c[0], True) for c in SRC_["calendars"]], [(c[1], True) for c in SRC_["calendars"]]))

    # --- J10.3 Settings > Replies > Voice
    s = settings_step("J10.3", "Settings > Replies > Voice", "voice")
    s.expect("analysis-line-names-the-sample",
             lambda: (core(s, "reply/voice")["voice"]["sample_size"], core(s, "reply/voice")["voice"]["analysed_at_ms"] is not None, state(s)["voice"]["sample"]),
             (VOI["sample_before"], True, VOI["sample_before"]))
    at0 = ctl_try(lambda: j.ctl.core("reply/voice")["voice"]["analysed_at_ms"])
    s.click("%s.voice.sample.%d" % (R, VOI["sample_pick"]))
    s.click(R + ".voice.analyse")
    s.expect("analyse-runs-on-the-chosen-sample",
             lambda: (core(s, "reply/voice")["voice"]["sample_size"], core(s, "reply/voice")["voice"]["analysed_at_ms"] != at0, state(s)["voice"]["sample"]),
             (VOI["sample_pick"], True, VOI["sample_pick"]))
    text0 = ctl_try(lambda: j.ctl.core("reply/voice")["voice"]["tone_text"])
    focus_editor(s, "voiceText")
    s.expect("text-field-takes-the-focus", lambda: editors(s)["voiceText"]["focused"], True)
    s.raw("keyevent", VOI["typed"])
    s.raw("keyevent", "⌘s")
    s.expect("text-edit-is-saved", lambda: (core(s, "reply/voice")["voice"]["tone_text"], state(s)["voice"]["text"]),
             ((text0 or "") + VOI["typed"], (text0 or "") + VOI["typed"]) if text0 is not None else None)

    # --- J10.4 Settings > Replies > Automatic draft
    s = settings_step("J10.4", "Settings > Replies > Automatic draft", "auto")
    s.expect("three-lines-with-counts",
             lambda: ({k: core(s, "reply/counts")[k] for k in ("received", "after_entry", "after_ai", "after_exit", "drafts")},
                      {k: state(s)["counts"][k] for k in ("received", "after_entry", "after_ai", "drafts")}, len(state(s)["auto_lines"])),
             ({k: CNT[k] for k in ("received", "after_entry", "after_ai", "after_exit", "drafts")},
              {k: CNT[k] for k in ("received", "after_entry", "after_ai", "drafts")}, CNT["line_count"]))
    s.expect("use-lines-sources-tone-prompt",
             lambda: (core(s, "reply/use/auto")["tone_id"], core(s, "reply/use/auto")["prompt_id"] == core(s, "reply/prompt/current")["prompt_id"],
                      AUT["sources_line_digit"] in state(s)["sources_line"]),
             (AUT["tone"], True, True))
    s.expect("switch-shows-the-sample-state", lambda: (core(s, "reply/use/auto")["enabled"], state(s)["auto_enabled"]), (AUT["enabled_before"], AUT["enabled_before"]))
    s.click(R + ".auto.enabled")
    s.expect("turning-it-off-writes-it", lambda: (core(s, "reply/use/auto")["enabled"], state(s)["auto_enabled"]), (not AUT["enabled_before"], not AUT["enabled_before"]))
    s.click(R + ".auto.enabled")
    s.expect("turning-it-on-writes-it", lambda: (core(s, "reply/use/auto")["enabled"], state(s)["auto_enabled"]), (True, True))

    # --- J10.5 Which messages: the three label cards
    s = settings_step("J10.5", "Auto draft > Which messages", "auto", arrives=False)      # the arrival here is the line of J10.4, checked below
    s.click(R + ".auto.step.entry")
    s.expect("arrives-by-the-line", lambda: state(s)["ui"]["page"], "filter")
    s.expect("sample-cards-and-labels",
             lambda: ([core(s, "reply/filter")[k]["enabled"] for k in ("entry", "ai", "exit")], core(s, "reply/filter")["scope"]["all"], len(core(s, "reply/filter")["scope"]["label_ids"])),
             (FIL["cards"], FIL["scope_all_in_sample"], FIL["labels_ticked_in_sample"]))
    lab = ctl_try(lambda: [r["ax"] for r in j.ctl.rows(R + ".controls") if r["ax"].startswith(R + ".filter.scope.label.")][0].rsplit(".", 1)[1])
    ticked0 = ctl_try(lambda: set(j.ctl.core("reply/filter")["scope"]["label_ids"]))
    s.click("%s.filter.scope.label.%s" % (R, lab))
    s.expect("ticking-a-label-writes-the-scope",
             lambda: (core(s, "reply/filter")["scope"]["all"], sorted(core(s, "reply/filter")["scope"]["label_ids"]), state(s)["scope_all"]),
             (False, sorted(ticked0 ^ {lab}), False) if ticked0 is not None else None)
    s.click(R + ".filter.scope.all")
    s.expect("all-labels-again", lambda: (core(s, "reply/filter")["scope"]["all"], state(s)["scope_all"]), (True, True))
    fact, val = FIL["fact"]
    s.click("%s.filter.entry.fact.%s.%s" % (R, fact, val))
    s.expect("a-fact-is-set", lambda: (core(s, "reply/filter")["entry"]["rule"]["facts"][fact], state(s)["entry_facts"][fact]), (val, val))
    d0 = ctl_try(lambda: j.ctl.core("reply/filter")["ai"]["description"])
    focus_editor(s, "aiDescription")
    s.expect("description-field-takes-the-focus", lambda: editors(s)["aiDescription"]["focused"], True)
    s.raw("keyevent", FIL["typed"])
    snap = ctl_try(lambda: j.ctl.rows(MONITOR))        # read at once, before anything else is sent
    typing_check(s, "description-typing-reaches-the-field", "aiDescription", FIL["typed"], snap)
    s.raw("keyevent", "⌘s")
    s.expect("description-edit-is-saved", lambda: (core(s, "reply/filter")["ai"]["description"], state(s)["description"]),
             ((d0 or "") + FIL["typed"], (d0 or "") + FIL["typed"]) if d0 is not None else None)
    s.click(R + ".filter.exit.switch")
    s.expect("a-card-switch-is-written", lambda: (core(s, "reply/filter")["exit"]["enabled"], state(s)["exit_on"]), (FIL["exit_card_off"], FIL["exit_card_off"]))

    # --- J10.6 Settings > Replies > Retrieval
    s = settings_step("J10.6", "Settings > Replies > Retrieval", "retrieval")
    s.expect("keep-default", lambda: (core(s, "reply/retrieval")["keep_n"], state(s)["retrieval"]["keep"]), (RET["keep_default"], RET["keep_default"]))
    other = ctl_try(lambda: [int(r["ax"].rsplit(".", 1)[1]) for r in j.ctl.rows(R + ".controls") if r["ax"].startswith(R + ".retrieval.keep.")
                             and int(r["ax"].rsplit(".", 1)[1]) != RET["keep_default"]][0])
    s.click("%s.retrieval.keep.%s" % (R, other))
    s.expect("editing-keep-is-written", lambda: (core(s, "reply/retrieval")["keep_n"], state(s)["retrieval"]["keep"]), (other, other))
    c0 = ctl_try(lambda: (j.ctl.core("reply/counts"), int(j.ctl.core(OUTBOX))))
    s.click(R + ".retrieval.try")
    s.expect("try-opens-the-trial", lambda: (state(s)["ui"]["popover"], state(s)["ui"]["trial_kind"]), ("trial", "on_demand"))
    s.expect("try-changes-nothing", lambda: (core(s, "reply/counts"), outbox(s)), c0)

    # --- J10.7 Settings > Replies > The prompt
    s = settings_step("J10.7", "Settings > Replies > The prompt", "prompt")
    s.expect("variables-are-seven", lambda: state(s)["prompt"]["variables"], PRM["variables"])
    s.expect("fixed-notes-are-two", lambda: (len(core(s, "reply/notes")), len(state(s)["prompt"]["notes"])), (PRM["notes"], PRM["notes"]))
    v0 = ctl_try(lambda: (len(j.ctl.core("reply/prompt/versions")), j.ctl.core("reply/prompt/current")["version"]))
    focus_editor(s, "prompt")
    s.expect("prompt-field-takes-the-focus", lambda: editors(s)["prompt"]["focused"], True)
    s.raw("keyevent", PRM["typed"])
    snap = ctl_try(lambda: j.ctl.rows(MONITOR))
    typing_check(s, "prompt-typing-reaches-the-field", "prompt", PRM["typed"], snap)
    s.raw("keyevent", "⌘s")
    s.expect("save-adds-a-version",
             lambda: (len(core(s, "reply/prompt/versions")) - v0[0], core(s, "reply/prompt/current")["version"] - v0[1], core(s, "reply/prompt/current")["text"].endswith(PRM["typed"]),
                      state(s)["prompt"]["version"] == core(s, "reply/prompt/current")["version"], v0[1]),
             (1, 1, True, True, PRM["versions_current"]))
    s.raw("keyevent", "space")
    s.click(R + ".editor.prompt.at")
    s.expect("at-opens-the-variable-list", lambda: (editors(s)["prompt"]["menu_open"], len(editors(s)["prompt"]["menu_items"])), (True, PRM["variables"]))

    # --- J10.8 Main window: a mail that passed the three steps arrives with a draft; nothing was sent
    s = j.step("J10.8", "A draft already in the reply")
    s.click(SW + ".close")                           # back to the main window (the Settings window's own close door: surface mac-main)
    s.expect("main-window-after-settings", lambda: s.ctl.uistate().get("surface"), "mac-main")
    s.raw("fixture", "reset")
    s.click(R + ".fixture.reset")
    if ctl_try(lambda: j.ctl.core("reply/use/auto")["enabled"]) is False:
        s.click(R + ".auto.enabled")                 # the automatic draft on (J10.4's switch): the arrival below needs it; the launch state has it on already
    ob0 = ctl_try(lambda: j.ctl.core(OUTBOX))
    s.raw("fixture", "deliver", "needs-reply", FX_PATH)
    wait(0.5)

    def arrival_draft():
        ctl = s.ctl
        end = time.time() + GAP_WAIT_S
        while True:
            try:
                d = ctl.core("reply/draft/%s" % ARR["thread"])
                return (d["made_by"], d["state"], d["instruction"], d["thread_id"] == ARR["thread"])
            except Unreadable as e:
                if "no core value" not in str(e):
                    raise
                if time.time() > end:
                    raise Unreadable(GAP_ARRIVAL)
                time.sleep(0.2)
    s.expect("arrival-writes-a-draft-alone", arrival_draft, (RPL["arrival_made_by"], RPL["arrival_state"], None, True))
    th = RPL["seeded_thread"]
    s.click("%s.open.%s.stay" % (QR, th))

    def shown():
        d = live_draft(s, th)
        a = ai(s)
        return (qfield(s, "thread"), a["mark"], a["made_by"], a["draft_id"] == d["id"], qfield(s, "text") == d["text"], d["state"])
    s.expect("open-shows-the-draft-marked-ai", shown, (th, "1", RPL["arrival_made_by"], True, True, RPL["arrival_state"]))
    s.type(RPL["owner_words"])
    s.expect("owner-words-take-it-over",
             lambda: (ai(s)["mark"], live_draft(s, th)["state"], qfield(s, "text").endswith(RPL["owner_words"])), ("0", RPL["taken_over_state"], True))
    s.expect("nothing-was-sent", lambda: outbox(s) - ob0, 0)           # the arrival, the open draft and the take-over: nothing in the outbox (a reset below empties it)
    s.key("Escape")
    s.raw("fixture", "reset")                         # the seeded draft comes back (reset re-seeds the reply book): the discard is read on it
    s.click("%s.open.%s.stay" % (QR, th))
    there = ctl_try(lambda: j.ctl.core("reply/draft/%s" % th) is not None)     # the positive control: the same read finds the draft before the discard
    ob0b = ctl_try(lambda: j.ctl.core(OUTBOX))
    s.click(AI_DISCARD)
    s.expect("discard-removes-the-draft", lambda: (there, absent(s, "reply/draft/%s" % th), ai(s)["mark"], qfield(s, "text")), (True, True, "0", ""))
    s.expect("nothing-was-sent-by-the-discard", lambda: outbox(s) - ob0b, 0)

    # --- J10.9 Main window, any thread: cmd-J, one instruction, the draft
    s = j.step("J10.9", "cmd-J: the instruction field, then the draft")
    s.key("Escape")
    s.key("R")                                        # the focused row's own thread, an empty reply field
    s.click(QR + ".ai.option.instruction-before.on")  # the fixture-only door of the quick reply (reply-thread.md): the Sur demande switch, set not flipped
    s.click(QR + ".ai.option.change-after.on")
    ob1 = ctl_try(lambda: j.ctl.core(OUTBOX))
    s.key("⌘J")
    s.expect("cmd-j-asks-for-the-instruction", lambda: (ai(s)["phase"], ai(s)["instruction_field"], ai(s)["focus"], qfield(s, "text")), ("asking", "1", "instruction", ""))
    s.type(RPL["instruction"])
    s.expect("instruction-field-takes-the-words", lambda: (rows(s, AI_INSTR)[0]["text"], qfield(s, "text")), (RPL["instruction"], ""))
    w0 = ctl_try(lambda: j.ctl.rows(AI)[0]["writes"])
    s.key("Enter")

    def written():
        th9 = qfield(s, "thread")
        d = live_draft(s, th9)
        a = ai(s)
        return (a["writes"] - w0, a["phase"], a["mark"], a["made_by"], d["instruction"], qfield(s, "text") == d["text"], a["instruction_field"])
    s.expect("return-writes-the-draft", written, (1, "ready", "1", RPL["draft_made_by_asked"], RPL["instruction"], True, "0"))
    s.expect("draft-names-its-sources", lambda: len(live_draft(s, qfield(s, "thread"))["excerpt_ids"]) > 0, True)
    s.expect("nothing-sent-alone", lambda: outbox(s) - ob1, 0)

    # --- J10.10 The same reply: the change field
    s = j.step("J10.10", "The change field under the draft")
    s.expect("change-field-under-the-draft", lambda: (ai(s)["change_field"], len(rows(s, AI_CHANGE))), ("1", 1))
    k0 = ctl_try(lambda: (j.ctl.rows(AI)[0]["draft_id"], j.ctl.rows(AI)[0]["revision"], j.ctl.rows(FIELD)[0]["text"]))
    s.click(AI_CHANGE)
    s.type(RPL["change"])
    s.key("Enter")

    def changed():
        d = live_draft(s, qfield(s, "thread"))
        a = ai(s)
        return (a["draft_id"] == k0[0] == d["id"], a["revision"] - k0[1], qfield(s, "text") == d["text"], d["text"] != k0[2], a["mark"], rows(s, AI_CHANGE)[0]["text"])
    s.expect("change-rewrites-in-place", changed, (True, 1, True, True, "1", ""))
    s.expect("nothing-sent-after-the-change", lambda: outbox(s) - ob1, 0)
    s.click(AI_DISCARD)                               # leaves no draft behind
    s.raw("fixture", "reset")


# MUTANTS
# J10.1/arrives-by-continue | Continue never reaches the Replies step
# J10.1/three-choices-with-defaults | the draft choice starts on Automatically
# J10.1/nothing-written-before-continue | picking a choice writes it at once
# J10.1/continue-writes-the-three-choices | Continue sends no SetReplyUseEnabled, or no analysis
# J10.1/continue-writes-inbox-on-sent-off | Inbox only leaves every Sent folder on
# J10.1/continue-leaves-the-step | Continue at the Replies step stays on it
# J10.2/arrives-by-control | the sidebar line Sources opens another pane
# J10.2/one-card-per-account-with-tiles | the second account has no card, or a folder has no tile
# J10.2/header-counts-messages | the header counts every folder's messages, ticked or not
# J10.2/folders-of-this-mac-listed | the folders of this Mac are missing
# J10.2/calendar-switches-listed | the calendars are not listed
# J10.2/sync-progress-shown | the syncing folder shows up to date
# J10.2/sources-line-counts | the Sources line counts mail folders only
# J10.2/tick-a-folder | ticking a folder writes nothing
# J10.2/add-a-folder-of-this-mac | adding a folder of this Mac adds nothing
# J10.2/switch-a-calendar | a calendar switch writes nothing
# J10.3/arrives-by-control | the sidebar line Voice opens another pane
# J10.3/analysis-line-names-the-sample | the analysis line shows no sample, or no date
# J10.3/analyse-runs-on-the-chosen-sample | Analyse ignores the chosen sample
# J10.3/text-field-takes-the-focus | a click in the tone text gives it no focus
# J10.3/text-edit-is-saved | the tone text is not saved
# J10.4/arrives-by-control | the sidebar line Automatic draft opens another pane
# J10.4/three-lines-with-counts | the counts cover another number of days, or a line is missing
# J10.4/use-lines-sources-tone-prompt | the tone or prompt line names another tone, or the Sources line misses its count
# J10.4/switch-shows-the-sample-state | the switch does not show the state the core holds
# J10.4/turning-it-off-writes-it | the switch turned off writes nothing
# J10.4/turning-it-on-writes-it | the switch turned on writes nothing
# J10.5/arrives-by-the-line | a step line of the automatic draft opens nothing
# J10.5/sample-cards-and-labels | a card is off, or the scope is not the two ticked labels
# J10.5/ticking-a-label-writes-the-scope | ticking a label leaves every label on
# J10.5/all-labels-again | the All labels tick does nothing
# J10.5/a-fact-is-set | a fact choice is not written
# J10.5/description-field-takes-the-focus | a click in the description gives it no focus
# J10.5/description-typing-reaches-the-field | the typed letter does not reach the description
# J10.5/description-edit-is-saved | the description edit is not saved
# J10.5/a-card-switch-is-written | a card switch writes nothing
# J10.6/arrives-by-control | the sidebar line Retrieval opens another pane
# J10.6/keep-default | the retrieval keeps another number by default
# J10.6/editing-keep-is-written | a Keep choice is not written
# J10.6/try-opens-the-trial | Try on a message opens nothing
# J10.6/try-changes-nothing | a trial stores a draft or queues a mail
# J10.7/arrives-by-control | the sidebar line The prompt opens another pane
# J10.7/variables-are-seven | the pane lists another number of variables
# J10.7/fixed-notes-are-two | the fixed notes are not served
# J10.7/prompt-field-takes-the-focus | a click in the prompt gives it no focus
# J10.7/prompt-typing-reaches-the-field | the typed letter does not reach the prompt
# J10.7/save-adds-a-version | a save of the prompt adds no version
# J10.7/at-opens-the-variable-list | typing @ opens no list
# J10.8/main-window-after-settings | closing Settings leaves the Settings surface in front
# J10.8/arrival-writes-a-draft-alone | the arrival's draft says it was made by the owner, not the AI (no draft at all is the gap: not judged)
# J10.8/open-shows-the-draft-marked-ai | the open reply shows no mark, or another text than the core's draft
# J10.8/owner-words-take-it-over | typing over the AI's words keeps the mark
# J10.8/discard-removes-the-draft | the discard control leaves the draft
# J10.8/nothing-was-sent | the arrival or its draft goes to the outbox by itself
# J10.8/nothing-was-sent-by-the-discard | the discard sends the draft
# J10.9/cmd-j-asks-for-the-instruction | cmd-J writes at once with no instruction field
# J10.9/instruction-field-takes-the-words | the typing goes to the reply field
# J10.9/return-writes-the-draft | Return writes a draft without the instruction, or unmarked
# J10.9/draft-names-its-sources | the draft carries no excerpt
# J10.9/nothing-sent-alone | the on-demand draft goes to the outbox
# J10.10/change-field-under-the-draft | no change field under the draft
# J10.10/change-rewrites-in-place | a change request makes a new draft, or leaves the text as it was
# J10.10/nothing-sent-after-the-change | the rewritten draft goes to the outbox



# ---------------------------------------------------------------------------------------------------------------- self-test: a stand-in app on the wire
# App answers the control-socket verbs the journey uses, the way docs/lanes/replies-contract.md (the reads), docs/lanes/reply-thread.md (the quick reply) and the J10
# table say the app behaves: it is written from those, never from a built screen. One mutant (a check label, "J10.2/tick-a-folder") is switched on at a time and must turn
# THAT check red. `arrival_drafts=False` is a core that writes no draft on an arrival: the arrival check is then not judged.
# `Inert` answers every verb ok with nothing in it: every check must then be red or not judged, none green.
SETTLE_SELFTEST, GAP_WAIT_SELFTEST = 0.02, 0.3


def _rule():
    return {"match": "all", "facts": {k: "any" for k in ("unread", "unanswered", "has_attachment", "starred", "vip_sender", "cc_me", "mailing_list", "replied_by_me")}, "rows": []}


class App:
    def __init__(self, mutants=(), arrival_drafts=True):
        self.m, self.arrival_drafts = set(mutants), arrival_drafts
        self.surface = "mac-main"
        self.reset()

    def on(self, label):
        return label in self.m

    def reset(self):
        self.fs_on, self.fs_step = False, 1
        self.rp = {"draft": "auto" if self.on("J10.1/three-choices-with-defaults") else "on-demand", "search": "sent-inbox", "voice": 100}
        self.applied, self.writes = False, []
        self.use = {"auto": {"kind": "auto", "enabled": not self.on("J10.4/switch-shows-the-sample-state"), "tone_id": "formal" if self.on("J10.4/use-lines-sources-tone-prompt") else "voice",
                             "match_thread": True, "prompt_id": "replies"},
                    "on_demand": {"kind": "on_demand", "enabled": True, "tone_id": "voice", "match_thread": True, "prompt_id": "replies"}}
        self.filter = {"scope": {"all": self.on("J10.5/sample-cards-and-labels"), "label_ids": [] if self.on("J10.5/sample-cards-and-labels") else ["needs-reply", "lbl-clients"]},
                       "entry": {"enabled": True, "rule": _rule()},
                       "ai": {"enabled": True, "subject": True, "body": True, "rest_of_thread": True, "attachments": False, "description": "Les messages qui attendent une reponse."},
                       "exit": {"enabled": True, "rule": _rule()}}
        self.opts = {"instruction_before": True, "change_after": True}
        self.labels = ["needs-reply", "lbl-clients"]

        def folder(acc, name, on, n, prog="up_to_date", pct=None):
            p = {"state": prog}
            if pct is not None:
                p["percent"] = pct
            return {"source": {"kind": "mail_folder", "account_id": acc, "folder": name}, "enabled": on, "item_count": n, "progress": p}
        syncing = ("up_to_date", None) if self.on("J10.2/sync-progress-shown") else ("syncing", 62)
        self.accounts = {ACC1: [folder(ACC1, "INBOX", True, 9204), folder(ACC1, "Sent", True, 4812), folder(ACC1, "Clients", True, 2960 + (1 if self.on("J10.2/header-counts-messages") else 0), *syncing),
                                folder(ACC1, "Clients/Lorvan Hotels", True, 1120, "syncing", 62), folder(ACC1, "Clients/Oranta", False, 1840, "not_started"), folder(ACC1, "Newsletters", False, 6310, "not_started")],
                         "acc-02": [folder("acc-02", "INBOX", True, 1204), folder("acc-02", "Sent Items", True, 640), folder("acc-02", "Family", False, 388, "not_started"),
                                    folder("acc-02", "Invoices", False, 512, "not_started"), folder("acc-02", "Archive", False, 2906, "not_started")]}
        if self.on("J10.2/one-card-per-account-with-tiles"):
            del self.accounts["acc-02"]
        self.local = [] if self.on("J10.2/folders-of-this-mac-listed") else [{"source": {"kind": "local_folder", "path": p}, "enabled": True, "item_count": 10, "progress": {"state": "up_to_date"}}
                                                                              for p in SRC_["local_folders"]]
        self.calendars = [] if self.on("J10.2/calendar-switches-listed") else [{"calendar_id": c[0], "name": c[1], "enabled": c[2]} for c in SRC_["calendars"]]
        self.retrieval = {"keep_n": 5 if self.on("J10.6/keep-default") else RET["keep_default"], "excerpts_per_question": 5}
        self.voice = {"tone_text": "Phrases courtes.", "sample_size": VOI["sample_before"], "analysed_at_ms": None if self.on("J10.3/analysis-line-names-the-sample") else 1793696400000}
        self.picked = None
        self.prompts = [{"prompt_id": "replies", "version": v, "text": "Reponds a {{message}} (v%d)." % v, "current": v == PRM["versions_current"]} for v in (1, 2, 3)]
        self.notes = [] if self.on("J10.7/fixed-notes-are-two") else [{"id": "note-%d" % i, "title": t, "text": t} for i, t in ((1, "Prix"), (2, "Disponibilites"))]
        self.ui = {"page": "auto", "popover": None, "trial_kind": None}
        self.place = ("general", None)
        self.cal_access = "not-asked"
        self.drafts = [{"id": "reply-draft-seed", "thread_id": RPL["seeded_thread"], "text": "Bonjour, voici la proposition avant jeudi.", "made_by": "auto", "instruction": None,
                        "state": "proposed", "revision": 0, "excerpt_ids": ["chunk-1", "chunk-2"]}]
        self.counter, self.outbox, self.dstate = 0, 0, {}
        self.ed_saved = {"voiceText": self.voice["tone_text"], "aiDescription": self.filter["ai"]["description"], "prompt": self.prompts[2]["text"]}
        self.ed_text, self.ed_menu, self.focused = dict(self.ed_saved), False, None
        self.q = {"open": False, "thread": None, "text": "", "phase": "idle", "instr": "", "change": "", "focus": "reply", "draft": None, "writes": 0}

    # -- the core's reads
    def total(self):
        mail = sum(f["enabled"] for fs in self.accounts.values() for f in fs)
        if self.on("J10.2/sources-line-counts"):
            return mail
        return mail + sum(x["enabled"] for x in self.local) + sum(c["enabled"] for c in self.calendars)

    def live(self, thread):
        d = [x for x in self.drafts if x["thread_id"] == thread and x["state"] in ("proposed", "edited")]
        return d[-1] if d else None

    def core(self, path):
        if path == OUTBOX:
            return self.outbox
        if path.startswith("reply/use/"):
            return dict(self.use[path.rsplit("/", 1)[1]])
        if path.startswith("reply/draft/"):
            d = self.live(path[len("reply/draft/"):])
            if d is None:
                raise KeyError(path)
            return dict(d)
        cur = [p for p in self.prompts if p["current"]][0]
        counts = {"days": 7, "received": CNT["received"] - (1 if self.on("J10.4/three-lines-with-counts") else 0), "after_entry": CNT["after_entry"], "after_ai": CNT["after_ai"],
                  "after_exit": CNT["after_exit"], "drafts": CNT["drafts"] + (1 if self.on("J10.6/try-changes-nothing") and self.ui["trial_kind"] else 0)}
        table = {"reply/filter": lambda: self.filter, "reply/on_demand": lambda: self.opts, "reply/retrieval": lambda: self.retrieval, "reply/counts": lambda: counts,
                 "reply/sources": lambda: {"accounts": [{"account_id": a, "folders": fs, "enabled_folders": sum(f["enabled"] for f in fs), "total_folders": len(fs)} for a, fs in self.accounts.items()],
                                           "local_folders": self.local, "calendars": self.calendars, "scope": "this_account", "enabled_total": self.total()},
                 "reply/voice": lambda: {"voice": self.voice, "analysis": {"state": "idle"}, "tones": []}, "reply/notes": lambda: self.notes,
                 "reply/prompt/current": lambda: cur, "reply/prompt/versions": lambda: sorted(self.prompts, key=lambda p: -p["version"])}
        if path not in table:
            raise KeyError(path)
        return json.loads(json.dumps(table[path]()))

    # -- first start (J10.1)
    def fs_continue(self):
        if not self.fs_on:
            return False
        if self.fs_step < 8:
            if self.on("J10.1/arrives-by-continue") and self.fs_step >= 7:
                return True
            self.fs_step += 1
        elif self.fs_step == 8:
            self.apply()
            if not self.on("J10.1/continue-leaves-the-step"):
                self.fs_step = 9
        return True

    def apply(self):
        self.applied, self.writes = True, []
        if not self.on("J10.1/continue-writes-the-three-choices"):
            self.writes.append("SetReplyUseEnabled")
            self.use["auto"]["enabled"] = self.rp["draft"] == "auto"
        for fs in self.accounts.values():
            for f in fs:
                r = role(f["source"]["folder"])
                if r == "inbox":
                    f["enabled"] = self.rp["search"] != "sent"
                elif r == "sent":
                    f["enabled"] = f["enabled"] if self.on("J10.1/continue-writes-inbox-on-sent-off") else self.rp["search"] != "inbox"
                if r:
                    self.writes.append("SetReplySourceEnabled")
        self.voice["sample_size"] = self.rp["voice"]
        if not self.on("J10.1/continue-writes-the-three-choices"):
            self.voice["analysed_at_ms"] = (self.voice["analysed_at_ms"] or 0) + 1000
            self.writes.append("RunVoiceAnalysis")
        else:
            self.writes.append("SetReplySourceEnabled")

    def choices(self):
        return [{"id": "draft", "chosen": self.rp["draft"], "options": [{"id": "on-demand"}, {"id": "auto"}]},
                {"id": "search", "chosen": self.rp["search"], "options": [{"id": "sent-inbox"}, {"id": "inbox"}, {"id": "sent"}]},
                {"id": "voice", "chosen": str(self.rp["voice"]), "options": [{"id": "100"}, {"id": "300"}, {"id": "1000"}]}]

    # -- Settings (J10.2 to J10.7)
    def settings_click(self, k):
        if k == "fixture.reset":
            self.ui, self.picked, self.focused, self.ed_menu = {"page": self.ui["page"], "popover": None, "trial_kind": None}, None, None, False
            return True
        if k.startswith("page."):
            self.ui["page"] = k[5:]
            return True
        if k == "auto.enabled":
            was = self.use["auto"]["enabled"]
            if not self.on("J10.4/turning-it-on-writes-it" if not was else "J10.4/turning-it-off-writes-it"):
                self.use["auto"]["enabled"] = not was
            return True
        if k == "auto.step.entry":
            if not self.on("J10.5/arrives-by-the-line"):
                self.ui["page"] = "filter"
            return True
        if k.startswith("sources.account.") and ".folder." in k:
            acc, i = k[len("sources.account."):].split(".folder.")
            if not self.on("J10.2/tick-a-folder"):
                f = self.accounts[acc][int(i)]
                f["enabled"] = not f["enabled"]
            return True
        if k == "sources.add-folder.sample":
            if not self.on("J10.2/add-a-folder-of-this-mac"):
                self.local.append({"source": {"kind": "local_folder", "path": SRC_["added_folder"]}, "enabled": True, "item_count": 1, "progress": {"state": "up_to_date"}})
            return True
        if k.startswith("sources.calendar."):
            if not self.on("J10.2/switch-a-calendar"):
                for c in self.calendars:
                    if c["calendar_id"] == k[len("sources.calendar."):]:
                        c["enabled"] = not c["enabled"]
            return True
        if k.startswith("voice.sample."):
            self.picked = int(k.rsplit(".", 1)[1])
            return True
        if k == "voice.analyse":
            if not self.on("J10.3/analyse-runs-on-the-chosen-sample") and self.picked:
                self.voice["sample_size"] = self.picked
            self.voice["analysed_at_ms"] = (self.voice["analysed_at_ms"] or 0) + 1000
            return True
        if k.startswith("filter.scope.label."):
            lid = k.rsplit(".", 1)[1]
            ids = self.filter["scope"]["label_ids"]
            self.filter["scope"]["all"] = self.on("J10.5/ticking-a-label-writes-the-scope")
            self.filter["scope"]["label_ids"] = [x for x in ids if x != lid] if lid in ids else ids + [lid]
            return True
        if k == "filter.scope.all":
            if not self.on("J10.5/all-labels-again"):
                self.filter["scope"] = {"all": True, "label_ids": []}
            return True
        if k.startswith("filter.entry.fact."):
            f, v = k[len("filter.entry.fact."):].rsplit(".", 1)
            if not self.on("J10.5/a-fact-is-set"):
                self.filter["entry"]["rule"]["facts"][f] = v
            return True
        if k == "filter.exit.switch":
            if not self.on("J10.5/a-card-switch-is-written"):
                self.filter["exit"]["enabled"] = not self.filter["exit"]["enabled"]
            return True
        if k.startswith("retrieval.keep."):
            if not self.on("J10.6/editing-keep-is-written"):
                self.retrieval["keep_n"] = int(k.rsplit(".", 1)[1])
            return True
        if k == "retrieval.try":
            if not self.on("J10.6/try-opens-the-trial"):
                self.ui.update(popover="trial", trial_kind="on_demand")
            return True
        if k.startswith("editor.") and k.endswith(".at"):
            key = k[len("editor."):-3]
            self.focused = key
            self.ed_text[key] += "@"
            self.ed_menu = not self.on("J10.7/at-opens-the-variable-list")
            return True
        if k.startswith("editor."):
            if not any(self.on(l) and k == "editor." + key for l, key in (("J10.3/text-field-takes-the-focus", "voiceText"), ("J10.5/description-field-takes-the-focus", "aiDescription"),
                                                                           ("J10.7/prompt-field-takes-the-focus", "prompt"))):
                self.focused = k[len("editor."):]
            return True
        if k == "fixture.reset":
            return True
        return False

    def keyevent(self, chord):
        if self.focused is None:
            return {"window_id": "mac-settings", "focus": "none"}
        key = self.focused
        if chord == "⌘s":
            blocked = {"voiceText": "J10.3/text-edit-is-saved", "aiDescription": "J10.5/description-edit-is-saved"}.get(key)
            if blocked and self.on(blocked):
                return {"window_id": "mac-settings", "focus": "RepliesTextView"}
            text = self.ed_text[key]
            if key == "voiceText":
                self.voice["tone_text"] = text
            elif key == "aiDescription":
                self.filter["ai"]["description"] = text
            elif key == "prompt":
                if self.on("J10.7/save-adds-a-version"):
                    [p for p in self.prompts if p["current"]][0]["text"] = text
                else:
                    for p in self.prompts:
                        p["current"] = False
                    self.prompts.append({"prompt_id": "replies", "version": max(p["version"] for p in self.prompts) + 1, "text": text, "current": True})
            self.ed_saved[key] = text
        elif chord != "space":
            if not ((key == "aiDescription" and self.on("J10.5/description-typing-reaches-the-field")) or (key == "prompt" and self.on("J10.7/prompt-typing-reaches-the-field"))):
                self.ed_text[key] += chord
        return {"window_id": "mac-settings", "focus": "RepliesTextView"}

    def state_row(self):
        c = self.core("reply/counts")
        return {"auto_enabled": self.use["auto"]["enabled"], "sources_line": "%d sources" % self.total(), "counts": c, "auto_lines": ["a", "b"] if self.on("J10.4/three-lines-with-counts") else ["a", "b", "c"],
                "scope_all": self.filter["scope"]["all"], "scope_label_ids": self.filter["scope"]["label_ids"], "entry_facts": self.filter["entry"]["rule"]["facts"],
                "exit_on": self.filter["exit"]["enabled"], "description": self.filter["ai"]["description"],
                "retrieval": {"keep": self.retrieval["keep_n"]}, "voice": {"sample": self.voice["sample_size"], "text": self.voice["tone_text"]},
                "prompt": {"variables": 6 if self.on("J10.7/variables-are-seven") else 7, "notes": [n["title"] for n in self.notes], "version": [p for p in self.prompts if p["current"]][0]["version"]},
                "ui": dict(self.ui)}

    def sources_rows(self):
        out = []
        for a, fs in self.accounts.items():
            out.append({"ax": "%s.sources.account.%s" % (R, a), "kind": "account"})
            out += [{"ax": "%s.sources.account.%s.folder.%d" % (R, a, i), "kind": "mail_folder", "name": f["source"]["folder"], "enabled": f["enabled"]} for i, f in enumerate(fs)]
        out += [{"ax": "%s.sources.local.%d" % (R, i), "kind": "local_folder", "path": f["source"]["path"], "enabled": f["enabled"]} for i, f in enumerate(self.local)]
        if self.cal_access == "allowed":
            out += [{"ax": "%s.sources.calendar.%s" % (R, c["calendar_id"]), "kind": "calendar", "name": c["name"], "enabled": c["enabled"]} for c in self.calendars]
        return out

    def editors_rows(self):
        return [{"key": k, "focused": self.focused == k, "dirty": self.ed_text[k] != self.ed_saved[k], "text": self.ed_text[k], "menu_open": self.ed_menu and self.focused == k,
                 "menu_items": ["message", "thread", "instruction", "excerpts", "calendar", "tone", "notes"] if self.ed_menu and self.focused == k else []} for k in self.ed_text]

    # -- the quick reply (J10.8 to J10.10; docs/lanes/reply-thread.md control table)
    def open_reply(self, thread):
        d = self.live(thread)
        self.q.update(open=True, thread=thread, text="", phase="idle", instr="", change="", focus="reply", draft=None)
        if d and d["state"] == "proposed":
            self.q.update(text=d["text"], draft=d["id"], phase="ready")

    def qdraft(self):
        return [d for d in self.drafts if d["id"] == self.q["draft"]][0] if self.q["draft"] else None

    def write_draft(self, instruction):
        q = self.q
        old = self.live(q["thread"])
        if old:
            old["state"] = "discarded"
        self.counter += 1
        by = "owner" if self.on("J10.8/arrival-writes-a-draft-alone") else "on_demand"
        d = {"id": "reply-draft-%d" % self.counter, "thread_id": q["thread"], "text": "Bonjour, merci pour votre message.", "made_by": "on_demand",
             "instruction": None if self.on("J10.9/return-writes-the-draft") else instruction, "state": "proposed", "revision": 0,
             "excerpt_ids": [] if self.on("J10.9/draft-names-its-sources") else ["chunk-1"]}
        self.drafts.append(d)
        q.update(draft=d["id"], text=d["text"], phase="ready", writes=q["writes"] + 1, instr="", focus="reply")
        if self.on("J10.9/nothing-sent-alone"):
            self.outbox += 1

    def taken_over(self):
        d = self.qdraft()
        if d and self.q["text"] != d["text"] and not self.on("J10.8/owner-words-take-it-over"):
            d["state"], self.q["draft"] = "edited", None

    def qkey(self, chord):
        q = self.q
        if chord == "R":
            self.open_reply("msg-01")
            return True
        if not q["open"]:
            return False
        if chord == "⌘J":
            if self.opts["instruction_before"] and not self.on("J10.9/cmd-j-asks-for-the-instruction"):
                q.update(phase="asking", focus="instruction", instr="")
            else:
                self.write_draft(None)
            return True
        if chord == "Enter":
            if q["phase"] == "asking":
                self.write_draft(q["instr"])
            elif q["focus"] == "change" and q["draft"]:
                d = self.qdraft()
                if self.on("J10.10/change-rewrites-in-place"):
                    self.write_draft(None)
                else:
                    d["text"], d["revision"] = d["text"] + " (" + q["change"] + ")", d["revision"] + 1
                    q.update(text=d["text"], change="", focus="reply")
                if self.on("J10.10/nothing-sent-after-the-change"):
                    self.outbox += 1
            return True
        if chord == "Escape":
            q["open"] = False
            return True
        return False

    def qtype(self, text):
        q = self.q
        if not q["open"]:
            return
        if q["focus"] == "instruction" and q["phase"] == "asking":
            if self.on("J10.9/instruction-field-takes-the-words"):
                q["text"] += text
            else:
                q["instr"] += text
        elif q["focus"] == "change" and q["draft"]:
            q["change"] += text
        else:
            q["text"] += text
            self.taken_over()

    def qclick(self, ax):
        q = self.q
        if ax.startswith(QR + ".open.") and ax.endswith(".stay"):
            self.open_reply(ax[len(QR + ".open."):-len(".stay")])
            return True
        if ax.startswith(AI + ".option."):
            k = ax[len(AI + ".option."):]
            name, val = k.rsplit(".", 1)
            self.opts[name.replace("-", "_")] = val == "on"
            return True
        if not q["open"]:
            return False
        if ax == AI_CHANGE:
            if q["draft"] and self.opts["change_after"]:
                q["focus"] = "change"
                return True
            return False
        if ax == AI_DISCARD:
            d = self.qdraft()
            if d is None:
                return False
            if not self.on("J10.8/discard-removes-the-draft"):
                d["state"] = "discarded"
                q.update(draft=None, text="", phase="idle")
            if self.on("J10.8/nothing-was-sent-by-the-discard"):
                self.outbox += 1
            return True
        if ax == FIELD:
            q["focus"] = "reply"
            return True
        return False

    def ai_row(self):
        q, d = self.q, self.qdraft()
        change = bool(d) and self.opts["change_after"] and q["phase"] != "asking" and not self.on("J10.10/change-field-under-the-draft")
        mark = "1" if d and not self.on("J10.8/open-shows-the-draft-marked-ai") else "0"
        return {"ax": AI, "phase": q["phase"], "mark": mark, "made_by": d["made_by"] if d else "", "draft_id": d["id"] if d else "", "revision": d["revision"] if d else 0,
                "focus": q["focus"], "writes": q["writes"], "instruction_field": "1" if q["phase"] == "asking" else "0", "change_field": "1" if change else "0"}

    # -- the wire
    def rows(self, ax):
        if ax == FS + ".replies":
            return [dict(c, ax="%s.replies.%s" % (FS, c["id"])) for c in self.choices()] if self.fs_on and self.fs_step == 8 else []
        if ax == WR + ".applied":
            return [{"applied": self.applied, "writes": self.writes, "refusal": ""}]
        if ax == SW + ".place":
            return [{"pane": self.place[0], "reply": self.place[1], "title": "t"}]
        if ax == R + ".state":
            return [self.state_row()]
        if ax == R + ".sources":
            return self.sources_rows()
        if ax == MONITOR:
            return [{"ax": MONITOR, "seen": 1, "sent": 1, "arm": "a", "last": {"window_id": "mac-settings", "focus": "RepliesTextView", "handled": True, "windows": [], "text_before": "", "text_after": "x"}}]
        if ax == R + ".editors":
            return self.editors_rows()
        if ax == R + ".controls":
            return [{"ax": "%s.filter.scope.label.%s" % (R, l), "page": "filter", "kind": "check"} for l in self.labels] + \
                   [{"ax": "%s.retrieval.keep.%d" % (R, n), "page": "retrieval", "kind": "choice"} for n in (3, 6, 10)]
        if ax in (AI, FIELD, AI_INSTR, AI_CHANGE) and not self.q["open"]:
            return []
        if ax == AI:
            return [self.ai_row()]
        if ax == FIELD:
            return [{"ax": ax, "thread": self.q["thread"], "text": self.q["text"]}]
        if ax == AI_INSTR:
            return [{"ax": ax, "text": self.q["instr"]}] if self.q["phase"] == "asking" else []
        if ax == AI_CHANGE:
            return [{"ax": ax, "text": self.q["change"]}] if self.ai_row()["change_field"] == "1" else []
        return []

    def click(self, ax):
        if ax == FS + ".continue":
            return self.fs_continue()
        if ax.startswith(FS + ".replies.") and self.fs_on and self.fs_step == 8:
            row, opt = ax[len(FS + ".replies."):].split(".", 1)
            self.rp[row] = int(opt) if row == "voice" else opt
            if self.on("J10.1/nothing-written-before-continue"):
                self.apply()
            return True
        if ax == SW + ".open":
            return True
        if ax == SW + ".close":
            self.surface = "mac-settings-replies" if self.on("J10.8/main-window-after-settings") else "mac-main"
            return True
        if ax.startswith(SW + ".pane-row.replies."):
            part = ax.rsplit(".", 1)[1]
            self.ui["page"] = part
            self.surface = "mac-settings-replies"
            other = {"auto": "voice"}.get(part, "auto")
            self.place = ("replies", other if self.on("J10.%d/arrives-by-control" % {"sources": 2, "voice": 3, "auto": 4, "retrieval": 6, "prompt": 7}.get(part, 0)) else part)
            return True
        if ax.startswith(R + "."):
            return self.settings_click(ax[len(R) + 1:])
        if ax.startswith(QR + "."):
            return self.qclick(ax)
        return False

    def deliver(self, name, path):
        a = json.load(open(path, encoding="utf-8"))["arrivals"][name]
        if self.on("J10.8/nothing-was-sent"):
            self.outbox += 1
        if self.arrival_drafts and self.use["auto"]["enabled"]:
            self.counter += 1
            self.drafts.append({"id": "reply-draft-%d" % self.counter, "thread_id": a["thread"], "text": "Bonjour, oui jeudi.", "instruction": None, "state": "proposed", "revision": 0,
                                "made_by": "owner" if self.on("J10.8/arrival-writes-a-draft-alone") else "auto", "excerpt_ids": ["chunk-1"]})

    def handle(self, verb, args):
        try:
            if verb == "rows":
                return {"ok": True, "rows": self.rows(args[0])}
            if verb == "core":
                return {"ok": True, "value": self.core(args[1])}
            if verb == "click":
                return {"ok": True} if self.click(args[0]) else {"ok": False, "error": "did nothing"}
            if verb == "key":
                return {"ok": True} if self.qkey(args[0]) else {"ok": False, "error": "key does nothing"}
            if verb == "type":
                self.qtype(args[0])
                return {"ok": True}
            if verb == "uistate":
                return {"ok": True, "state": {"surface": self.surface}}
            if verb == "keyevent":
                return dict(self.keyevent(args[0]), ok=True)
            if verb == "fixture":
                if args[0] == "reset":
                    self.reset()
                elif args[0] == "first-start":
                    self.fs_on, self.fs_step = args[1] == "on", 1
                elif args[0] == "calendar-access":
                    self.cal_access = args[1]
                elif args[0] == "deliver":
                    self.deliver(args[1], args[2])
                else:
                    return {"ok": False, "error": "fixture %s is not served" % args[0]}
                return {"ok": True}
        except KeyError as e:
            return {"ok": False, "error": "no core value at %s" % e}
        return {"ok": False, "error": "nothing"}


class Inert:
    """An app that does nothing: every verb answers ok with an empty reply."""
    def handle(self, verb, args):
        return {"ok": True, "rows": [], "value": None}


def serve(model, fn):
    """Runs fn(j) over a socket served by `model`; returns the journey's checks. The socket, the server and the folder are closed on every path."""
    import shutil
    import socketserver
    import tempfile
    import threading
    from lib.ctl import Ctl
    from lib.harness import Journey

    class H(socketserver.StreamRequestHandler):
        def handle(self):
            for line in self.rfile:
                q = json.loads(line.decode())
                self.wfile.write((json.dumps(model.handle(q["verb"], q["args"])) + "\n").encode())
    d = tempfile.mkdtemp(prefix="j10-standin-")
    srv = socketserver.ThreadingUnixStreamServer(os.path.join(d, "whoosh.sock"), H)
    srv.daemon_threads = True
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    ctl = None
    try:
        ctl = Ctl(d).connect()
        j = Journey(ID, ctl)
        fn(j)
        return j.checks()
    finally:
        if ctl is not None:
            ctl.close()
        srv.shutdown()
        srv.server_close()
        shutil.rmtree(d, ignore_errors=True)


def run_app(model):
    global SETTLE, GAP_WAIT_S
    old = (SETTLE, GAP_WAIT_S)
    SETTLE, GAP_WAIT_S = SETTLE_SELFTEST, GAP_WAIT_SELFTEST
    try:
        return serve(model, drive)
    finally:
        SETTLE, GAP_WAIT_S = old


def via_runner():
    """The same journey through journeys/run.py's own path (lib.harness.run_journey over $WHOOSH_SOCKET_DIR) on the ideal stand-in: verdict, mutant rows both ways."""
    global SETTLE, GAP_WAIT_S
    import shutil
    import socketserver
    import tempfile
    import threading
    from lib.harness import run_journey
    model = App()

    class H(socketserver.StreamRequestHandler):
        def handle(self):
            for line in self.rfile:
                q = json.loads(line.decode())
                self.wfile.write((json.dumps(model.handle(q["verb"], q["args"])) + "\n").encode())
    d = tempfile.mkdtemp(prefix="j10-runner-")
    srv = socketserver.ThreadingUnixStreamServer(os.path.join(d, "whoosh.sock"), H)
    srv.daemon_threads = True
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    old, env = (SETTLE, GAP_WAIT_S), os.environ.get("WHOOSH_SOCKET_DIR")
    SETTLE, GAP_WAIT_S = SETTLE_SELFTEST, GAP_WAIT_SELFTEST
    os.environ["WHOOSH_SOCKET_DIR"] = d
    try:
        return run_journey(sys.modules[__name__], os.path.abspath(__file__))
    finally:
        SETTLE, GAP_WAIT_S = old
        if env is None:
            os.environ.pop("WHOOSH_SOCKET_DIR", None)
        else:
            os.environ["WHOOSH_SOCKET_DIR"] = env
        srv.shutdown()
        srv.server_close()
        shutil.rmtree(d, ignore_errors=True)


def selftest():
    declared = {}
    for line in open(os.path.abspath(__file__), encoding="utf-8"):
        if line.startswith("# J10.") and " | " in line:
            k, v = line[2:].rstrip("\n").split(" | ", 1)
            declared[k.strip()] = v
    bad = 0
    ideal = run_app(App())
    labels = [c.label for c in ideal]
    print("ideal stand-in (the contract, drafts on arrival): %d checks, %d green, %d red, %d not judged"
          % (len(ideal), sum(c.state == "pass" for c in ideal), sum(c.state == "fail" for c in ideal), sum(c.state == "unjudged" for c in ideal)))
    for c in ideal:
        if c.state != "pass":
            print("  NOT GREEN on the ideal stand-in: %s %s: %s" % (c.label, c.state, c.detail))
            bad += 1
    fixture = run_app(App(arrival_drafts=False))
    off = [c for c in fixture if c.state != "pass"]
    print("no-draft stand-in (a core that writes no draft on arrival): %d checks, %d green, %d red, %d not judged"
          % (len(fixture), sum(c.state == "pass" for c in fixture), sum(c.state == "fail" for c in fixture), sum(c.state == "unjudged" for c in fixture)))
    for c in off:
        print("  %s %s: %s" % (c.state.upper(), c.label, c.detail))
    if [c.label for c in off] != ["J10.8/arrival-writes-a-draft-alone"] or off[0].state != "unjudged":
        print("  UNEXPECTED: only the arrival check may be not judged on the no-draft stand-in")
        bad += 1
    inert = run_app(Inert())
    green = [c.label for c in inert if c.state == "pass"]
    print("inert stand-in (does nothing): %d checks, %d red, %d not judged, %d green%s"
          % (len(inert), sum(c.state == "fail" for c in inert), sum(c.state == "unjudged" for c in inert), len(green), (": " + ", ".join(green)) if green else ""))
    bad += len(green)
    for lab in labels:
        if lab not in declared:
            print("NO MUTANT %s" % lab)
            bad += 1
    for lab in sorted(set(declared) - set(labels)):
        print("STRAY MUTANT %s" % lab)
        bad += 1
    r = via_runner()
    print("through run_journey on the ideal stand-in: %s, %d steps, %d checks, %d failed, %d not judged, no mutant row for %s, stray rows %s"
          % (r["verdict"], r["steps"], r["checks"], r["failed"], r["unjudged"], r["no_mutant"], r["stray_mutant"]))
    bad += r["verdict"] != "GREEN" or bool(r["no_mutant"]) or bool(r["stray_mutant"])
    steps_red, exact = {}, {}
    for lab in labels:
        if lab not in declared:
            continue
        got = {c.label: c for c in run_app(App({lab}))}
        state = got[lab].state
        also = sorted(k for k, c in got.items() if c.state != "pass" and k != lab)
        print("mutant %-52s -> %s%s" % (lab, "RED" if state == "fail" else "NOT RED (%s)" % state, ("  also: " + ", ".join(also)) if also else ""))
        bad += state != "fail"
        steps_red.setdefault(lab.split("/")[0], []).append(state == "fail")
        if state == "fail" and not also:
            exact.setdefault(lab.split("/")[0], []).append(lab)
        elif state == "fail" and {k.split("/")[0] for k in also} <= {lab.split("/")[0]}:
            exact.setdefault(lab.split("/")[0], []).append(lab)
    for step in sorted(steps_red, key=lambda x: int(x[3:].split(".")[1])):
        print("step %-6s %d mutants, %d red, %d turn only this step red: %s" % (step, len(steps_red[step]), sum(steps_red[step]), len(exact.get(step, [])), ", ".join(x.split("/")[1] for x in exact.get(step, []))))
        bad += not exact.get(step)
    print("selftest: %d checks, %d mutants, %d not red or missing" % (len(labels), len(declared), bad))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else 0)
