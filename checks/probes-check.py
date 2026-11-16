#!/usr/bin/env python3
"""Probes check: `fixture reset` brings the fixture core back to exactly its state at launch (probes-r1, round 3).

Cases
  reset-covers-every-stored-property   STATIC, runs without an app: every stored property of FixtureCore (Sources/CoreBridge/
                                       FixtureCore.swift) is assigned in `restoreSeedLocked` (FixtureReset.swift), reset by
                                       `resetMessages`, or named in `FixtureDigest.skipped`.
  steps-change-the-digest              DRIVEN: archive, delete, junk, star, a reply, a corpus each change `fixture digest`.
  reset-restores-the-launch-state      DRIVEN: digest at launch == digest after those steps and `fixture reset`; on a miss the
                                       parts that differ are named (one part per stored property).
Run (driven): sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/probes-check.py'
Run (no app): python3 checks/probes-check.py --selftest     (the model of the reset, one mutant per class of state; the static
                                       case on the real source and on a mutant of it)
Prints 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
"""
import os
import time
import types
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable      # noqa: E402
sys.path.insert(0, ROOT)
from checks.checklib import titlebar                         # noqa: E402

CORE = os.path.join(ROOT, "Sources", "CoreBridge", "FixtureCore.swift")
RESET = os.path.join(ROOT, "Sources", "CoreBridge", "FixtureReset.swift")
# A worktree is a sparse checkout without `_captures` (main88: hshard-0 held no corpus): the file is the one of the main checkout two folders up.
CORPUS = next((p for p in (os.path.join(ROOT, "_captures", "reader", "r5", "links-corpus.json"),
                           os.path.join(ROOT, "..", "..", "_captures", "reader", "r5", "links-corpus.json")) if os.path.exists(p)),
              os.path.join(ROOT, "_captures", "reader", "r5", "links-corpus.json"))

# MUTANTS (the selftest's model of the reset; each skips one class of state, and the digest case goes red naming the part)
# probes-r1/reset-restores-the-launch-state | mail-flags    | archived, deleted, junk and starred stay as the steps left them
# probes-r1/reset-restores-the-launch-state | corpus        | the served list stays the corpus's
# probes-r1/reset-restores-the-launch-state | drafts-outbox | drafts, the outbox and the undo stack stay
# probes-r1/reset-restores-the-launch-state | labels        | label edits and their assignments stay
# probes-r1/reset-restores-the-launch-state | accounts      | account edits stay
# probes-r1/reset-restores-the-launch-state | ai-settings   | AI providers, keys and agents stay
# probes-r1/reset-restores-the-launch-state | view          | tab, filter, selection and focus stay
# probes-r1/no-control-under-the-title-bar | ignore-window    | the helper skips a window: its offender goes unreported
# probes-r1/no-control-under-the-title-bar | band-zero        | the band is read as 0: nothing is under it
# probes-r1/no-control-under-the-title-bar | skip-under-band   | a control under the band is skipped
# probes-r1/keyevent-is-a-real-event | press-instead   | `keyevent` calls AppModel.press: the monitor counter stays 0
# probes-r1/permission-seams-follow-the-rig-flag | core-kind | a seam keyed on the core kind (CorePlace.onRig removed from CalendarsModel / DictationRig)
# probes-r1/install-state-new-is-a-new-installation | keeps-sample | `fixture install-state new` leaves the sample (auto-draft on, calendars listed)
# probes-r1/relaunch-keeps-the-run-folder | wipes-folder | `--relaunch` removes the run folder
# probes-r1/firstrun-data-equals-golden | labels-rule    | Promotions loses its "mailing list = yes" entry rule in the embedded data
# probes-r1/firstrun-data-equals-golden | labels-text    | a built-in label's description is changed
# probes-r1/firstrun-data-equals-golden | providers      | gemini is dropped from the providers
# probes-r1/firstrun-data-equals-golden | filter         | the reply filter's AI description is emptied
# probes-r1/firstrun-data-equals-golden | sources        | a folder of the Sources pane is switched on
# probes-r1/firstrun-data-equals-golden | auto           | auto-draft is on
# probes-r1/firstrun-data-equals-golden | refusal        | the label AI's refusal words are changed
# probes-r1/firstrun-data-equals-golden | apply-dropped  | FixtureFirstRun.swift stops applying the providers class
# probes-r1/shot-names-its-window | ignores-window-arg   | `shot <name> window:<id>` photographs the front window
# probes-r1/shot-refuses-what-is-no-window | serves-front  | `shot mac-settings-window` returns the front window's picture
# probes-r1/shot-opens-no-blank-window | opens-slot-window | `shot mac-settings-replies` opens a window titled mac-settings-replies
# probes-r1/reply-folder-gone-and-back | verb-does-nothing | `fixture reply-folder-gone` changes nothing
# probes-r1/reply-folder-gone-and-back | reset-keeps-gone   | `fixture reset` leaves the folder gone
# probes-r1/reset-covers-every-stored-property | source-drops-a-restore | one assignment of the restore is removed from the source


# ---- the static case -------------------------------------------------------------------------------------------

def stored_properties(core_src):
    """Names of the stored properties declared at class level in the head of FixtureCore.swift (before `init`)."""
    head = core_src[:core_src.index("    public init(url:")]
    names = re.findall(r"^    (?:public (?:private|internal)\(set\) |public |private |lazy |nonisolated\(unsafe\) |static )*(?:var|let) (\w+)", head, re.M)
    static = set(re.findall(r"^    (?:public |private )*(?:nonisolated\(unsafe\) )?static (?:var|let) (\w+)", head, re.M))
    return [n for n in names if n not in static]


def uncovered(core_src, reset_src):
    """Stored properties neither reset by `restoreSeedLocked` / `resetMessages` nor named as skipped."""
    body = reset_src[reset_src.index("func restoreSeedLocked"):reset_src.index("/// One value over")]
    rm = core_src[core_src.index("func resetMessages"):]
    rm = rm[:rm.index("buildContactCards()")]
    skipped = set(re.findall(r'"(\w+)"', reset_src[reset_src.index("static let skipped"):reset_src.index("/// FNV-1a")]))
    out = []
    for n in stored_properties(core_src):
        if n in skipped or re.search(r"\b%s\b" % n, body) or re.search(r"\b%s\b" % n, rm) or n == "order":
            continue
        out.append(n)
    return out


def static_case(core_src, reset_src):
    miss = uncovered(core_src, reset_src)
    return ("ok", "all %d stored properties are reset or named as skipped" % len(stored_properties(core_src))) if not miss \
        else ("FAIL", "not reset and not skipped: %s" % ", ".join(miss))


# ---- the driven cases -------------------------------------------------------------------------------------------

def digest(ctl):
    r = ctl.send("fixture", "digest")
    return r.get("digest"), r.get("parts", {}), r.get("view")


def differing(a, b):
    return sorted(k for k in set(a) | set(b) if a.get(k) != b.get(k))


def list_ids(ctl):
    """The message ids the main list shows right now (what a verb that names a message can still reach)."""
    return [r["id"] for r in (ctl.send("rows", "mac-main.list").get("rows") or []) if r.get("id")]


STAR_ROWS_TRIED = 6


def star_on_a_healthy_row(ctl):
    """S on the focused row; a row of the failing account is refused by the core on purpose (FixtureCore.refuseFailing: every change to its mail is refused, the
    sync line says it needs a sign-in), so the key does nothing there (main81: msg-03, a Provanta RFP row, `last_failure` Provanta RFP). Moves down one row at a
    time and presses S again, for STAR_ROWS_TRIED rows; the last refusal is raised when none of them takes it, so a star that does nothing anywhere stays red."""
    for i in range(STAR_ROWS_TRIED):
        try:
            return ctl.send("key", "S")
        except Unreadable:
            if i == STAR_ROWS_TRIED - 1:
                raise
            ctl.send("key", "J")


def drive(ctl, corpus=CORPUS):
    """-> list of (name, state, text); state ok | FAIL | NOT JUDGED."""
    out = []
    ctl.send("fixture", "reset")
    d0, p0, v0 = digest(ctl)
    if not d0:
        return [("steps-change-the-digest", "NOT JUDGED", "fixture digest answers no value"),
                ("reset-restores-the-launch-state", "NOT JUDGED", "fixture digest answers no value")]
    changed, same, unjudged = [], [], []
    steps = [("archive", ("key", "E")), ("delete", ("key", "#")), ("junk", ("key", "!")), ("star", star_on_a_healthy_row),
             ("reply", None), ("corpus", ("fixture", "corpus", corpus))]
    prev = d0
    for name, call in steps:
        if name == "reply":
            # the reply goes to a message the copy still holds at this step (the three steps before it took rows out of the list), read from the list
            ids = list_ids(ctl)
            if not ids:
                unjudged.append("reply: the main list holds no message to reply to")
                continue
            call = ("fixture", "reply", ids[0])
        try:
            call(ctl) if callable(call) else ctl.send(*call)
        except Unreadable as e:
            where = ""
            try:
                where = " [uistate %s; list %s]" % (ctl.uistate(), list_ids(ctl))
            except Exception as e2:                     # the read that would explain the refusal failed too: say so, never hide it
                where = " [uistate unread: %s]" % type(e2).__name__
            same.append("%s refused (%s)%s" % (name, e, where))
            continue
        d, _, _ = digest(ctl)
        (changed if d != prev else same).append(name if d != prev else "%s changed nothing" % name)
        prev = d
    if unjudged and not same:
        out.append(("steps-change-the-digest", "NOT JUDGED", "changed the digest: %s; %s" % (", ".join(changed), "; ".join(unjudged))))
    else:
        out.append(("steps-change-the-digest", "ok" if not same else "FAIL",
                    "changed the digest: %s%s" % (", ".join(changed), "; " + "; ".join(same + unjudged) if same or unjudged else "")))
    ctl.send("fixture", "reset")
    d1, p1, v1 = digest(ctl)
    bad = differing(p0, p1)
    out.append(("reset-restores-the-launch-state", "ok" if d1 == d0 and v0 == v1 else "FAIL",
                "digest %s -> %s; parts that differ: %s; view %s" % (d0, d1, ", ".join(bad) or "none", "same" if v0 == v1 else "%r -> %r" % (v0, v1))))
    return out


# ---- round 5: keyevent, permission seams, install-state, relaunch ------------------------------------------------

def read(path):
    with open(os.path.join(ROOT, path), encoding="utf-8") as f:
        return f.read()


def func_body(src, name):
    i = src.index(name)
    return src[i:i + 6000]


def keyevent_static(src):
    """The verb builds NSEvents and hands them to NSApp.sendEvent (arm a) / window.sendEvent (arm b); it never calls AppModel.press."""
    run = func_body(src, "public static func run")
    ok = "NSApp.sendEvent(e)" in run and "w.sendEvent(e)" in run and "NSEvent.keyEvent(with: type" in run and ".press(" not in run
    return ("ok", "run builds keyDown and keyUp NSEvents and sends them by NSApp.sendEvent or window.sendEvent") if ok \
        else ("FAIL", "KeyEventVerb.run does not send real events by NSApp.sendEvent / window.sendEvent, or calls press")


def seams_static(calendars, dictation, hidden):
    bad = []
    if "CorePlace.onRig" not in calendars: bad.append("CalendarsModel.swift")
    if "CorePlace.onRig" not in dictation: bad.append("DictationRig.swift")
    for kind in ('"calendar-access"', '"microphone"'):
        if kind not in hidden: bad.append("HiddenSystemUI.swift (%s)" % kind)
    return ("ok", "both seams ask CorePlace.onRig; the hidden rig refuses and counts calendar-access and microphone") if not bad \
        else ("FAIL", "not keyed on the rig flag or not hooked: " + ", ".join(bad))


def relaunch_static(script):
    body = script[script.index("relaunch_main() {"):]
    body = body[:body.index("\n}\n")]
    removals = re.findall(r"\brm\b[^\n;]*", body)
    bad = [r for r in removals if "-r" in r or not re.search(r'"\$D/(died|\$SOCKET_NAME|relaunching)"', r)]
    return ("ok", "relaunch_main removes only died, the socket and its own flag") if not bad else ("FAIL", "relaunch_main removes: " + "; ".join(bad))


class Rig5:
    """Stand-in for the driven reads of round 5. `mut`: press-instead | keeps-sample."""
    def __init__(self, mut=None):
        self.mut, self.seen, self.new = mut, 0, False

    def send(self, verb, *a):
        if verb == "keyevent":
            if a != ("reset",) and self.mut != "press-instead": self.seen += 1
            return {"ok": True}
        if verb == "fixture" and a[0] == "install-state":
            self.new = self.mut != "keeps-sample"; return {"ok": True}
        if verb == "rows" and a[0] == "mac-keyevent.monitor": return {"rows": [{"seen": self.seen}]}
        if verb == "core" and a[1] == "reply/use/auto": return {"value": {"enabled": not self.new, "kind": "auto", "match_thread": True, "prompt_id": "replies", "tone_id": "voice"}}
        if verb == "core" and a[1] == "reply/sources": return {"value": {"calendars": [] if self.new else [{"name": "Travail"}]}}
        return {"ok": True}


def keyevent_driven(ctl):
    """One real key, one event seen by the monitor. Arm d makes the window key first (KeyEventVerb.swift:174): arm a sends to whatever window is key, and after the
    cases before this one (`fixture reset`) none is (main92, 93, 95: key_before false, key_after false, 0 events for one key). The assertion is unchanged."""
    # The case runs after `fixture reset`, when no window is key (main92 and main93: key_before false, the monitor saw 0; green on main88, where a window was key).
    # A key built for a window while the app has no key window is not a key the way the owner types one, so the case asks for the main window first, through
    # the door the dock tile uses (WindowBridge.bringMainToFront, BaseFeature `dock.whoosh`); a refusal is kept in the reply words, never hidden. The verb itself
    # (KeyEventVerb.swift) stays as it is: a real key on the owner's copy is untouched.
    try:
        ctl.send("click", "dock.whoosh")
    except Unreadable:
        pass
    ctl.send("keyevent", "reset"); reply = ctl.send("keyevent", "arm:d", "q")   # a key no preset binds: `h` is Snooze, and the shell's own monitor (installed first) takes a bound key, so this counter, installed on the first keyevent, never sees it (main95: handled True, monitor_seen 0, key_before False, the window being key made no difference)
    seen = ctl.send("rows", "mac-keyevent.monitor")["rows"][0]["seen"]
    said = {f: reply.get(f, "absent") for f in ("seen", "sent", "monitor_seen", "window", "window_id", "focus", "route", "handled", "key_before", "key_after")} if isinstance(reply, dict) else reply
    return ("ok", "one real key, one event seen by the monitor") if seen == 1 else ("FAIL", "the monitor saw %s events for one key (want 1); the verb's own reply: %s" % (seen, said))


def scroll_frame_driven(ctl):
    """DRIVEN: a click on the frame `measure` serves for a list row lands on THAT row, after the list has scrolled. What `drag` and `measure` need is not that a row
    keeps its y across a scroll (a lazy list shows another row at that place), but that the id asked is served where it is drawn now. So the list is scrolled by its own
    key: J moves the focus down to a row that was below the fold, the list scrolls to show it (its own scroll view, not the largest one the `scroll` verb picks), and the
    served frame of THAT focused row (`mac-main.list.row.<id>.text`) must lie inside the window. A frame from before the scroll lies below it.
    Main104 (the earlier form, `scroll page-down` then the first row's y): source probe/probe, y 152.0 -> 152.0, scroll_y None, the offset 52.5: the verb scrolled the
    largest scroll view, which is not the list the first row sits in, so a first row that does not move is no evidence. -> (state, text)."""
    try:
        rows = ctl.send("rows", "mac-main.list")["rows"]
        win = next(f for f in ctl.send("winframes").get("frames", []) if str(f.get("id", "")).startswith("mac-main"))
        win_h = win["h"]
        n = min(len(rows) - 1, 25)
        if n < 8:
            return ("NOT JUDGED", "the list holds %d rows: too few to scroll it with the focus" % len(rows))
        ctl.send("click", "mac-main.list.row.0")
        for _ in range(n):
            ctl.send("key", "J")
        time.sleep(0.4)
        focus = ctl.uistate().get("focus_message")
        fr = ctl.send("measure", "mac-main.list.row.%s.text" % focus)["frame"]
    except (Unreadable, KeyError, IndexError, TypeError, StopIteration) as e:
        return ("NOT JUDGED", "the list could not be scrolled by the focus and measured: %s" % e)
    ctl.send("key", "Escape")
    said = "row %s (the %dth) served at y %.1f h %.1f in a window %.0f high, source %s, scroll_y %s" % (focus, n, fr["y"], fr["h"], win_h, fr.get("source"), fr.get("scroll_y"))
    if fr["y"] >= 0 and fr["y"] + fr["h"] <= win_h + 1:
        return ("ok", "the focused row below the fold is served inside the window: " + said)
    return ("FAIL", "the focused row was scrolled into view but its served frame is outside the window (the place before the scroll): " + said)


def install_driven(ctl):
    ctl.send("fixture", "install-state", "new")
    auto = ctl.send("core", "state", "reply/use/auto").get("value")
    cals = ctl.send("core", "state", "reply/sources").get("value")
    cals = cals.get("calendars") if isinstance(cals, dict) else None
    bad = []
    # the app serves reply/use/auto as an object (RepliesStore.swift:59): its `enabled` says on or off; an older core answered the bare flag
    on = auto.get("enabled") if isinstance(auto, dict) else auto
    if on not in (False, "false"): bad.append("auto-draft is %r (want off)" % (auto,))
    if cals: bad.append("calendars %r (want none)" % (cals,))
    return ("ok", "auto-draft off, no calendar") if not bad else ("FAIL", "; ".join(bad))


def relaunch_driven(ctl):
    import subprocess
    d = ctl.dir
    marker = os.path.join(d, "relaunch-marker")
    open(marker, "w").write("keep")
    before = open(os.path.join(d, "rig.pid")).read().strip()
    p = subprocess.run(["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), "--relaunch"], env=dict(os.environ, D=d, WHOOSH_RIG_HIDDEN="1"),
                       stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    after = open(os.path.join(d, "rig.pid")).read().strip()
    ok = p.returncode == 0 and before != after and os.path.exists(marker)
    return ("ok", "pid %s -> %s, the run folder kept" % (before, after)) if ok else ("FAIL", "rc %d, pid %s -> %s, marker %s: %s" % (p.returncode, before, after, os.path.exists(marker), p.stdout[-200:]))


# ---- round 6: the fixture's new installation equals the golden -------------------------------------------------

GOLDEN_PATHS = ("docs/FIRST-RUN.json", "../replies-settings/docs/FIRST-RUN.json")
DATA_SWIFT = "Sources/CoreBridge/FixtureFirstRunData.swift"
APPLY_SWIFT = "Sources/CoreBridge/FixtureFirstRun.swift"


def load_golden():
    import json
    for rel in GOLDEN_PATHS:
        path = os.path.join(ROOT, rel)
        if os.path.exists(path):
            with open(path, encoding="utf-8") as f:
                return json.load(f), rel
    return None, None


def golden_static(golden, data_src, apply_src):
    """ONE file both cores are compared with: the Swift data is regenerated from docs/FIRST-RUN.json and must equal the committed file; every
    class of it is applied by FixtureFirstRun.swift. -> (state, text)."""
    sys.path.insert(0, os.path.join(ROOT, "checks", "checklib"))
    import firstrun_data
    import json
    want = firstrun_data.subset(golden)
    m = re.search(r'static let json = #"""\n(.*?)\n"""#', data_src, re.S)
    if not m:
        return "FAIL", "FixtureFirstRunData.swift holds no embedded json"
    try:
        have = json.loads(m.group(1))
    except ValueError as e:
        return "FAIL", "the embedded json does not parse (%s)" % e
    bad = [k for k in want if have.get(k) != want[k]]
    if bad:
        return "FAIL", "differs from the golden in: " + ", ".join(bad)
    if data_src != firstrun_data.render(golden):
        return "FAIL", "equal values but the file is not what checks/checklib/firstrun_data.py writes (regenerate it)"
    unapplied = [k for k, needle in (("labels", "applyLabels("), ("providers", "applyProviders("), ("filter", 'g["filter"]'), ("auto", 'g["auto"]'),
                                      ("sources", 'g["sources"]'), ("refusal", 'FirstRunGolden.data["refusal"]')) if needle not in apply_src]
    if unapplied:
        return "FAIL", "FixtureFirstRun.swift does not apply: " + ", ".join(unapplied)
    return "ok", "the embedded data equals docs/FIRST-RUN.json (%s) and every class is applied" % ", ".join(sorted(want))


SERVED_PREFIXES = ("label/", "ai_provider/", "reply/use/", "reply/filter", "reply/sources", "reply/calendars/", "reply/notes")


def compare_paths(golden, read, ids):
    """read(path) -> JSON value or None. Account masks become the fixture's first two account ids. -> (differing, unserved)."""
    import json
    def unmask(v):
        t = json.dumps(v, ensure_ascii=False, sort_keys=True)
        for n, a in enumerate(ids, 1):
            t = t.replace("$ACCOUNT_%d" % n, a)
        return json.loads(t)
    differ, unserved = [], []
    for path, want in sorted(golden["defaults"]["state"].items()):
        if not path.startswith(SERVED_PREFIXES) or want is None:
            continue
        got = read(path)
        if got is None:
            unserved.append(path)
        elif json.dumps(got, sort_keys=True, ensure_ascii=False) != json.dumps(unmask(want), sort_keys=True, ensure_ascii=False):
            differ.append(path)
    return differ, unserved


def json_unmask(v, ids):
    import json
    t = json.dumps(v, ensure_ascii=False)
    for n, a in enumerate(ids, 1):
        t = t.replace("$ACCOUNT_%d" % n, a)
    return json.loads(t)


def golden_driven(ctl, golden):
    ctl.send("fixture", "install-state", "new")
    ids = [r["id"] for r in ctl.send("rows", "mac-main.account-book").get("rows", [])][:2]
    def read(path):
        try:
            return ctl.send("core", "state", path).get("value")
        except Unreadable:
            return None
    differ, unserved = compare_paths(golden, read, ids)
    if differ:
        return "FAIL", "%d paths differ from the golden: %s" % (len(differ), ", ".join(differ))
    return "ok", "every served path equals the golden; %d paths of the golden are not served by the fixture: %s" % (len(unserved), ", ".join(unserved))      # the whole list, none cut


# ---- round 6 addition: `shot` names the window it returns --------------------------------------------------------

def shot_static(src):
    need = [("window:<id> argument", 'hasPrefix("window:")'), ("refusal sentence", "is not a surface and not an open window"),
            ("no blank window for a Settings-hosted slot", "SettingsPane.pane(for: slot) != nil"), ("reply names the window", '"window": s.window')]
    miss = [n for n, k in need if k not in src]
    return ("ok", "ShotVerb parses window:<id>, refuses a name that is no window, and routes a Settings-hosted slot to the Settings window") if not miss \
        else ("FAIL", "ShotVerb lacks: " + ", ".join(miss))


def shot_driven(ctl):
    """-> list of (case, state, text). Needs the Settings window open: `shot mac-settings-general` opens it."""
    out = []
    ctl.send("shot", "mac-settings-general")
    bands = ctl.send("winbands").get("bands", [])
    wrong = []
    for w in bands:
        if not w.get("id"):
            continue
        r = ctl.send("shot", "probes-r1-window", "window:" + w["id"])
        if r.get("window") != w["id"]:
            wrong.append("%s -> %s" % (w["id"], r.get("window")))
    out.append(("shot-names-its-window", "ok" if bands and not wrong else "FAIL",
                "%d windows asked by id; the picture's window differs for: %s" % (len(bands), ", ".join(wrong) or "none")))
    try:
        r = ctl.send("shot", "mac-settings-window")
        out.append(("shot-refuses-what-is-no-window", "FAIL", "mac-settings-window was served (window %r)" % r.get("window")))
    except Unreadable as e:
        out.append(("shot-refuses-what-is-no-window", "ok" if "not a surface" in str(e) else "FAIL", "refused: %s" % str(e)[:160]))
    before = len(ctl.send("winbands").get("bands", []))
    r = ctl.send("shot", "mac-settings-replies")
    after = ctl.send("winbands").get("bands", [])
    blank = [w for w in after if w.get("title") == "mac-settings-replies"]
    out.append(("shot-opens-no-blank-window", "ok" if not blank and len(after) == before and r.get("title") != "mac-settings-replies" else "FAIL",
                "windows %d -> %d; a window titled mac-settings-replies: %s; the picture is of %r" % (before, len(after), bool(blank), r.get("title"))))
    return out


class Rig7:
    """Stand-in for the driven shot cases. `mut`: ignores-window-arg | serves-front | opens-slot-window."""
    def __init__(self, mut=None):
        self.mut = mut
        self.wins = [{"id": "mac-main", "title": "Whoosh"}, {"id": "mac-settings-general", "title": "Settings"}]

    def send(self, verb, *a):
        if verb == "winbands":
            return {"bands": [dict(w) for w in self.wins]}
        if verb == "shot" and a[0] == "mac-settings-general":
            return {"window": "mac-settings-general", "title": "Settings"}
        if verb == "shot" and len(a) > 1 and a[1].startswith("window:"):
            want = a[1][7:]
            return {"window": "mac-main" if self.mut == "ignores-window-arg" else want, "title": "x"}
        if verb == "shot" and a[0] == "mac-settings-window":
            if self.mut == "serves-front": return {"window": "mac-main", "title": "Whoosh"}
            raise Unreadable("mac-settings-window is not a surface and not an open window")
        if verb == "shot" and a[0] == "mac-settings-replies":
            if self.mut == "opens-slot-window":
                self.wins.append({"id": "surface", "title": "mac-settings-replies"})
                return {"window": "surface", "title": "mac-settings-replies"}
            return {"window": "mac-settings-general", "title": "Settings"}
        return {"ok": True}


# ---- delta 24: a mail folder gone from the server -------------------------------------------------------------

def first_folder(ctl):
    src = ctl.send("core", "state", "reply/sources").get("value") or {}
    acc = (src.get("accounts") or [{}])[0]
    fol = (acc.get("folders") or [{}])[0]
    return acc.get("account_id"), (fol.get("source") or {}).get("folder"), (fol.get("progress") or {}).get("state"), fol.get("enabled")


def folder_gone_driven(ctl):
    ctl.send("fixture", "reset")
    acc, folder, before, _ = first_folder(ctl)
    if not acc or not folder:
        return "NOT JUDGED", "reply/sources lists no folder"
    ctl.send("fixture", "reply-folder-gone", acc, folder)
    _, _, gone, en = first_folder(ctl)
    ctl.send("fixture", "reply-folder-gone", acc, folder, "back")
    _, _, back, _ = first_folder(ctl)
    ctl.send("fixture", "reply-folder-gone", acc, folder)
    ctl.send("fixture", "reset")
    _, _, reset, _ = first_folder(ctl)
    bad = []
    if gone != "gone" or en is not False: bad.append("gone: state %r enabled %r (want gone, off)" % (gone, en))
    if back == "gone": bad.append("back: still gone")
    if reset == "gone": bad.append("after fixture reset: still gone")
    return ("ok", "%s %s: %s -> gone (off) -> %s; after reset %s" % (acc, folder, before, back, reset)) if not bad else ("FAIL", "; ".join(bad))


class Rig8:
    """Stand-in. `mut`: verb-does-nothing | reset-keeps-gone."""
    def __init__(self, mut=None):
        self.mut, self.gone = mut, False

    def send(self, verb, *a):
        if verb == "fixture" and a[0] == "reply-folder-gone":
            if self.mut != "verb-does-nothing": self.gone = len(a) == 3
            return {"ok": True}
        if verb == "fixture" and a[0] == "reset":
            if self.mut != "reset-keeps-gone": self.gone = False
            return {"ok": True}
        if verb == "core":
            return {"value": {"accounts": [{"account_id": "acc-01", "folders": [{"source": {"folder": "INBOX"}, "enabled": not self.gone,
                                                                               "progress": {"state": "gone" if self.gone else "up_to_date"}}]}]}}
        return {"ok": True}


# ---- the selftest model ------------------------------------------------------------------------------------------

CLASSES = {"mail-flags": ["archived", "deleted", "spam", "starred"], "corpus": ["messages"], "drafts-outbox": ["draftRows", "entries", "undo"],
           "labels": ["labelRows", "assigned"], "accounts": ["added", "accountNames"], "ai-settings": ["agents"], "view": ["__view"]}


class Model:
    """A stand-in for the app: the same verbs, state grouped in the classes the real reset restores."""

    def __init__(self, skip=None, refuse_star=0):
        self.skip = skip
        self.refuse_star = refuse_star       # S is refused this many times (the failing account's rows before a healthy one)
        self.seed = {p: "seed" for ps in CLASSES.values() for p in ps}
        self.s = dict(self.seed)
        self.touches = 0
        self.held = ["msg-held"]              # the one message the model's list holds; a reply to any other id is refused like the app does

    def uistate(self):
        return {"surface": "mac-main"}

    def digest(self):
        parts = {k: "%s" % v for k, v in self.s.items() if k != "__view"}
        return {"ok": True, "digest": "|".join("%s=%s" % kv for kv in sorted(parts.items())), "parts": parts, "view": self.s["__view"]}

    def send(self, verb, *a):
        self.touches += 1
        t = "t%d" % self.touches
        if verb == "fixture" and a[0] == "digest":
            return self.digest()
        if verb == "rows" and a[0] == "mac-main.list":
            return {"rows": [{"id": i} for i in self.held]}
        if (verb, a[0]) == ("fixture", "reply") and a[1] not in self.held:
            raise Unreadable("no message %s" % a[1])
        if verb == "fixture" and a[0] == "reset":
            for cls, ps in CLASSES.items():
                if cls == self.skip:
                    continue
                for p in ps:
                    self.s[p] = self.seed[p]
            return {"ok": True}
        if verb == "key":
            if a[0] == "S" and self.refuse_star:
                self.refuse_star -= 1
                raise Unreadable("key ('S',): the key S does nothing on mac-main")
            for p in {"E": ["archived"], "#": ["deleted"], "!": ["spam"], "S": ["starred"], "J": []}[a[0]]:
                self.s[p] = t
            if a[0] != "J":
                self.s["__view"] = t
        elif (verb, a[0]) == ("fixture", "corpus"):
            self.s["messages"] = t
        elif (verb, a[0]) == ("fixture", "reply"):
            for p in ("draftRows", "entries", "undo", "labelRows", "assigned", "added", "accountNames", "agents"):
                self.s[p] = t
        return {"ok": True}


def band_case(ctl):
    """DRIVEN: every full-size window's controls clear its title-bar band. -> (state, text)."""
    wins = ctl.send("winbands").get("bands", [])
    def controls_of(surface):
        try:
            return ctl.rows(surface + ".controls")
        except Unreadable:
            return None
    def measure(ax):
        try:
            return ctl.measure(ax).get("frame")
        except Unreadable:
            return None
    bad, unj = titlebar.offenders(wins, controls_of, measure)
    full = [w for w in wins if w.get("fullsize")]
    text = "%d windows, %d full-size (bands %s)" % (len(wins), len(full), ", ".join("%s %.0f" % (w.get("title") or w.get("id"), w["band"]) for w in full))
    if bad:
        return "FAIL", text + "; under the band: " + "; ".join("%s %s by %s pt" % o for o in bad)
    if unj:
        return "NOT JUDGED", text + "; " + "; ".join("%s: %s" % u for u in unj)
    return "ok", text


def catalogue_words(key, code=None):
    """The catalogue's sentences for `key` (Sources/Resources/Localizable.xcstrings): the one in language `code`, or (code unknown) the one of every language."""
    import json
    loc = json.load(open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8"))["strings"].get(key, {}).get("localizations", {})
    pick = {code: loc[code]} if code in loc else loc
    return {v.get("stringUnit", {}).get("value") for v in pick.values()}


COMPOSER_KEY = "New message"
COMPOSER_WORD_EN = "New message"     # selftest only: the model window's title, the catalogue key itself (the source language)        # WindowBridge.titleKeys["mac-composer"] (Sources/App/WindowBridge.swift:15)


def titles_case(wins, code=None):
    """Every window's title is words for the user, never an internal id: no title starts with mac- or iphone-. (state, text) over `winbands` rows;
    the composer window (surface mac-composer) must be there and show the catalogue's word for COMPOSER_KEY in the app's language `code` (when the
    app's language is not a catalogue code, any language's word)."""
    ids = [w.get("title", "") for w in wins if re.match(r"(mac|iphone)-", w.get("title", ""))]
    shown = ", ".join("%s=%r" % (w.get("surface") or w.get("id"), w.get("title", "")) for w in wins)
    if ids:
        return "FAIL", "a window title is an internal id: %s (windows: %s)" % (", ".join(ids), shown)
    comp = [w for w in wins if (w.get("surface") or w.get("id")) == "mac-composer"]   # a `winbands` row names its window by `id` (the surface id of a surface window: WindowBridge.id(of:)); main88 read `surface`, which the row never has
    if not comp:
        return "NOT JUDGED", "no window carries the surface mac-composer: the composer was not opened (windows: %s)" % shown
    want = catalogue_words(COMPOSER_KEY, code)
    if comp[0].get("title") not in want:
        return "FAIL", "the composer window's title is %r, not the catalogue word for %r in %s (%s)" % (comp[0].get("title"), COMPOSER_KEY, code or "any language", sorted(want))
    return "ok", "%d windows, no title is an id, the composer's title is the catalogue word %r (%s)" % (len(wins), comp[0].get("title"), shown)


def titles_driven(ctl):
    """DRIVEN: open the composer window (`shot mac-composer`, as shots-sheet does), then read `winbands` and the app's language."""
    ctl.send("shot", "mac-composer")
    try:
        code = ctl.rows("mac-settings-general.settings")[0].get("language")
    except (Unreadable, IndexError):
        code = None
    return titles_case(ctl.send("winbands").get("bands", []), code)


# The six places `ai-setup.places` lists (AISetupRows.swift:24-31) and the ax id each one DRAWS (None: the build draws none; the id is read from the views):
#   Settings sidebar rows: SettingsSidebar.swift:97 `mac-settings-window.dot.<pane>`; the reader's summary card: ReaderSummary.swift:22 `mac-main.reader.summary.dot`;
#   the search list: SearchListPane.swift:37 `MissingSetupMark(need: .meaning)` whose default id is `ai-setup.mark.meaning` (its button `.fix`);
#   the foot's reply-with-AI entry: MacFootHints.swift:152 `MissingSetupDot(needs: [.writing], id: "mac-main.foot.mark")` (gap-findings, db1e78f3).
MARK_PLACES = [("settings-sidebar.decisions", "mac-settings-window.dot.decisions"), ("settings-sidebar.ai", "mac-settings-window.dot.ai"),
               ("settings-sidebar.replies", "mac-settings-window.dot.replies"), ("reader-summary", "mac-main.reader.summary.dot"),
               ("foot-reply-with-ai", "mac-main.foot.mark"), ("search-meaning", "ai-setup.mark.meaning.fix")]


def marks_drawn(listed, measure):
    """Every listed place DRAWS its mark: `listed` the place names of `rows ai-setup.places`, `measure(ax)` a frame or None (nothing on screen carries the id).
    -> (state, text): FAIL when a place with an id is not on screen or a listed place is not in the table; NOT JUDGED when a place has no id to measure
    (the build draws none) or sits on a surface that is not open (the reader's card with no message open)."""
    drawn, absent, noid = [], [], []
    for place, ax in MARK_PLACES:
        if place not in listed:
            absent.append(place + " (not listed by the row source)")
        elif ax is None:
            noid.append(place)
        elif measure(ax):
            drawn.append(place)
        elif place == "reader-summary":
            noid.append(place + " (no frame: no message open in the reader?)")
        else:
            absent.append("%s (%s not on screen)" % (place, ax))
    extra = [p for p in listed if p not in [m[0] for m in MARK_PLACES]]
    text = "%d of %d places drawn: %s" % (len(drawn), len(MARK_PLACES), ", ".join(drawn) or "none")
    if absent or extra:
        return "FAIL", text + "; not drawn: " + "; ".join(absent + ["%s (listed, not in this check's table)" % e for e in extra])
    if noid:
        return "NOT JUDGED", text + "; cannot be measured: " + "; ".join(noid)
    return "ok", text


def marks_driven(ctl):
    """DRIVEN: with all three needs shown missing (the preview door), the Settings window is open and a search is launched in the main window, each place's
    mark is measured on screen. The search's mark sits in the results pane of the main window's list (SearchListPane.swift:37, mounted by InboxListView
    `content` once Enter launched a search); `shot mac-search` opens the search WINDOW, which draws no such mark (main88: ai-setup.mark.meaning.fix not on screen)."""
    for need in ("decision", "writing", "meaning"):
        ctl.click("ai-setup.preview.%s.missing" % need)
    try:
        ctl.send("shot", "mac-settings-general")
        ctl.send("key", "/")                             # the search key opens the field: a click on `mac-search.bar` does nothing while the search is not active (SearchMacFeature.swift:54)
        for _ in range(30):
            if ctl.rows("mac-search.bar")[0].get("active"):
                break
            time.sleep(0.1)
        ctl.click("mac-search.bar")
        ctl.type("quote")
        ctl.send("key", "Enter")
        listed = [r["place"] for r in ctl.rows("ai-setup.places") if r.get("shown")]
        def measure(ax):
            end = time.time() + 3.0                      # the pane mounts on the turn after Enter: read until it is drawn (a read, never a blind sleep)
            while True:
                try:
                    frame = ctl.measure(ax).get("frame")
                except Unreadable:
                    frame = None
                if frame or time.time() >= end:
                    return frame
                time.sleep(0.1)
        return marks_drawn(listed, measure)
    finally:
        try:
            ctl.send("key", "Escape")                    # the search closes; the list is the inbox again for the cases after this one
        except Unreadable:
            pass
        ctl.click("ai-setup.reset")


READY_SENTENCES = ("A provider is connected.", "The decision model is on this Mac.", "The meaning model is on this Mac.")   # AISetupWords: writingReady, layaReady, meaningReady


def marks_words(rows):
    """With the three needs shown missing, no drawn mark says a READY sentence: `rows` are `rows ai-setup.marks` (shown, words, ax). The preview door once kept
    the status the rig computed (a rig that holds the meaning model: "The meaning model is on this Mac." in the warn colour beside Download, main200 picture)."""
    shown = [r for r in rows if isinstance(r, dict) and r.get("shown")]
    if not shown:
        return "NOT JUDGED", "no mark is shown under the missing preview (%d rows)" % len(rows)
    bad = ["%s says '%s'" % (r.get("ax"), r.get("words")) for r in shown if r.get("words") in READY_SENTENCES]
    if bad:
        return "FAIL", "a mark shown as missing says a ready sentence: " + "; ".join(bad)
    return "ok", "%d marks shown, none says a ready sentence" % len(shown)


FOOT_AI_WORDS, FOOT_TOL_PT = "mac-main.foot.ai-mark.words", 0.5


def foot_uncut(labels):
    """The foot strip's AI entry is drawn whole: `labels` are the rows of `labels window:<main>`; the entry's words (FOOT_AI_WORDS) need no more width than their
    box. Left out or whole, never cut (the hints' own rule): main200 at 1280 points with a search open read "Reply with AI · connect a mo…"."""
    mine = [x for x in labels if isinstance(x, dict) and x.get("ax") == FOOT_AI_WORDS and x.get("text") is not None]
    if not mine:
        return "NOT JUDGED", "the strip draws no AI entry (no label %s): the writing need is not shown missing, or the build does not mark the words" % FOOT_AI_WORDS
    cut = ['"%s" needs %.1f pt, box %.1f pt' % (x["text"], x.get("need_w", 0), x.get("box_w", 0)) for x in mine if x.get("need_w", 0) > x.get("box_w", 0) + FOOT_TOL_PT]
    if cut:
        return "FAIL", "the foot strip's AI entry is cut: " + "; ".join(cut)
    return "ok", "the AI entry is whole: %s" % "; ".join('"%s" %.1f of %.1f pt' % (x["text"], x.get("need_w", 0), x.get("box_w", 0)) for x in mine)


def marks_words_driven(ctl):
    """DRIVEN: the three needs shown missing and a search launched at the window's own size: (the marks' sentences, the foot strip's AI entry)."""
    for need in ("decision", "writing", "meaning"):
        ctl.click("ai-setup.preview.%s.missing" % need)
    try:
        ctl.send("key", "/")
        for _ in range(30):
            if ctl.rows("mac-search.bar")[0].get("active"):
                break
            time.sleep(0.1)
        ctl.click("mac-search.bar")
        ctl.type("quote")
        ctl.send("key", "Enter")
        time.sleep(0.6)
        words = marks_words(ctl.rows("ai-setup.marks"))
        main = next((f.get("id") for f in ctl.winframes() if isinstance(f, dict) and str(f.get("id", "")).startswith("mac-main")), None)
        if main is None:
            return words, ("NOT JUDGED", "no main window in winframes")
        try:
            foot = foot_uncut(ctl.send("labels", "window:%s" % main).get("labels") or [])
        except Unreadable as e:
            foot = ("NOT JUDGED", "labels of the main window could not be read: %s" % e)
        return words, foot
    finally:
        try:
            ctl.send("key", "Escape")
        except Unreadable:
            pass
        ctl.click("ai-setup.reset")


def settings_one_window(wins):
    """After open, reset, open: exactly one window holds the Settings pages and its id starts mac-settings; no screen window carries a settings slot
    (title or id naming mac-settings-... on a window whose id does not start mac-settings). (state, text) over `winbands` rows."""
    mine = [w for w in wins if str(w.get("id", "")).startswith("mac-settings")]
    stray = [w for w in wins if not str(w.get("id", "")).startswith("mac-settings") and "mac-settings" in (str(w.get("title", "")) + str(w.get("surface", "")))]
    if len(mine) != 1 or stray:
        return "FAIL", "%d Settings windows (%s); a screen window holds a settings page: %s" % (
            len(mine), ", ".join(str(w.get("id")) for w in mine), ", ".join("%s|%s" % (w.get("id"), w.get("title")) for w in stray) or "none")
    return "ok", "one Settings window (%s) and no screen window for a settings page" % mine[0].get("id")


def settings_driven(ctl):
    """DRIVEN: open a Settings pane by its slot, `fixture reset`, open it again; judged on winbands."""
    ctl.send("shot", "mac-settings-replies")
    ctl.send("fixture", "reset")
    ctl.send("shot", "mac-settings-replies")
    return settings_one_window(ctl.send("winbands").get("bands", []))


def settings_static(bridge):
    """STATIC: WindowBridge.open sends every settings slot (WindowBridge.isSettingsSlot, Replies included) to the Settings window."""
    miss = [n for n, k in (("open routes by isSettingsSlot", "if isSettingsSlot(slot) { open(id: settingsWindowId) }"),
                           ("window(for:) routes by isSettingsSlot", "if isSettingsSlot(slot) { return visible.first")) if k not in bridge]
    return ("FAIL", "missing: " + ", ".join(miss)) if miss else ("ok", "WindowBridge opens and finds every settings slot in the one Settings window")


def titles_static(scenes, bridge):
    """STATIC: the surface window's title comes from WindowBridge.title(for:) and no `.navigationTitle(...)` in the scenes takes a slot or surface id;
    a window is found by its tag, not its title."""
    bad = re.findall(r"\.navigationTitle\(\s*(?:slot|surface)[^)]*\)", scenes)
    miss = [n for n, k in (("title from the catalogue", ".navigationTitle(WindowBridge.title(for: slot))"),
                           ("window found by its tag", "surfaceId(of: $0) == slot.id")) if k not in scenes + bridge]
    if bad or miss:
        return "FAIL", "; ".join(filter(None, ["id as title: " + ", ".join(bad) if bad else "", "missing: " + ", ".join(miss) if miss else ""]))
    return "ok", "ShellScenes titles a surface window with the catalogue word and WindowBridge finds it by its tag"


FAKE_WINDOWS = [{"id": "mac-settings", "title": "mac-settings-general", "band": 52.0, "fullsize": True, "lights": [{"x": 8, "y": 14, "w": 14, "h": 16}]},
                {"id": "mac-main", "title": "mac-main", "band": 28.0, "fullsize": True, "lights": []},
                {"id": "x", "title": "mac-plain", "band": 28.0, "fullsize": False, "lights": []}]
FAKE_FRAMES = {"mac-settings.a": {"x": 200, "y": 16, "w": 80, "h": 24}, "mac-settings.b": {"x": 200, "y": 60, "w": 80, "h": 24},
               "mac-settings.light": {"x": 8, "y": 14, "w": 14, "h": 16}, "mac-main.a": {"x": 20, "y": 8, "w": 60, "h": 20},
               "mac-plain.a": {"x": 0, "y": 0, "w": 10, "h": 10}}


def fake_controls(surface):
    return [{"ax": k} for k in FAKE_FRAMES if k.startswith(surface + ".")]


def selftest():
    bad, n = 0, 0

    def report(ok, text):
        nonlocal bad, n
        n += 1
        bad += not ok
        print(("ok  " if ok else "RED ") + text)
    core, reset = open(CORE, encoding="utf-8").read(), open(RESET, encoding="utf-8").read()
    st, text = static_case(core, reset)
    report(st == "ok", "reset-covers-every-stored-property on the real source: " + text)
    gone = re.sub(r"\n        labelRows = \[\]; labelRuleSets = \[:\];", "\n        labelRuleSets = [:];", reset, count=1)
    mut = static_case(core, gone)
    report(gone != reset and mut[0] == "FAIL" and "labelRows" in mut[1], "mutant source-drops-a-restore is red on reset-covers-every-stored-property (%s)" % mut[1])
    want = [("mac-settings-general", "mac-settings.a", 36.0), ("mac-main", "mac-main.a", 20.0)]
    got, unj = titlebar.offenders(FAKE_WINDOWS, fake_controls, FAKE_FRAMES.get)
    report(got == want and not unj, "no-control-under-the-title-bar on the fake windows names both offenders with their points (%s)" % got)
    ok_windows = [dict(w, band=8.0) for w in FAKE_WINDOWS]
    got, _ = titlebar.offenders(ok_windows, fake_controls, FAKE_FRAMES.get)
    report(got == [], "no-control-under-the-title-bar passes when the band is 8 pt (the traffic light and the plain window are excepted)")
    for mu in ("ignore-window", "band-zero", "skip-under-band"):
        got, _ = titlebar.offenders(FAKE_WINDOWS, fake_controls, FAKE_FRAMES.get, mutant=mu)
        report(got != want, "mutant %s is red on no-control-under-the-title-bar (reports %s)" % (mu, got))
    cal, dic, hid = (read("Sources/Features/SettingsCalendarsMac/CalendarsModel.swift"), read("Sources/Features/Dictation/DictationRig.swift"),
                     read("Sources/App/HiddenSystemUI.swift"))
    ke = read("Sources/ControlSocket/KeyEventVerb.swift")
    st = keyevent_static(ke)
    report(st[0] == "ok", "keyevent-is-a-real-event on the real source: " + st[1])
    mut = keyevent_static(ke.replace("NSApp.sendEvent(e)", "m.press(chord)"))
    report(mut[0] == "FAIL", "mutant press-instead is red on keyevent-is-a-real-event (static: %s)" % mut[1])
    report(keyevent_driven(Rig5())[0] == "ok" and keyevent_driven(Rig5("press-instead"))[0] == "FAIL", "mutant press-instead is red on keyevent-is-a-real-event (driven: the monitor counter stays 0)")
    st = seams_static(cal, dic, hid)
    report(st[0] == "ok", "permission-seams-follow-the-rig-flag on the real source: " + st[1])
    for nm, args in (("CalendarsModel", (cal.replace("CorePlace.onRig", "false"), dic, hid)), ("DictationRig", (cal, dic.replace("CorePlace.onRig", "false"), hid)),
                     ("HiddenSystemUI", (cal, dic, hid.replace('"microphone"', '"mic"')))):
        mut = seams_static(*args)
        report(mut[0] == "FAIL", "mutant core-kind (%s) is red on permission-seams-follow-the-rig-flag (%s)" % (nm, mut[1]))
    report(install_driven(Rig5())[0] == "ok" and install_driven(Rig5("keeps-sample"))[0] == "FAIL", "install-state-new-is-a-new-installation passes on a new installation; mutant keeps-sample is red")
    sc = read("scripts/launch-one.sh")
    st = relaunch_static(sc)
    report(st[0] == "ok", "relaunch-keeps-the-run-folder on the real script: " + st[1])
    mut = relaunch_static(sc.replace('  rm -f "$D/died" "$D/$SOCKET_NAME"\n  start_rig; rc=$?', '  rm -rf "$D"\n  start_rig; rc=$?'))
    report(mut[0] == "FAIL", "mutant wipes-folder is red on relaunch-keeps-the-run-folder (%s)" % mut[1])
    golden, where = load_golden()
    if golden is None:
        report(False, "firstrun-data-equals-golden: docs/FIRST-RUN.json is not there yet (looked in %s)" % ", ".join(GOLDEN_PATHS))
    else:
        data, app = read(DATA_SWIFT), read(APPLY_SWIFT)
        st = golden_static(golden, data, app)
        report(st[0] == "ok", "firstrun-data-equals-golden on the real files (%s): %s" % (where, st[1]))
        muts = {"labels-rule": ('"fact": "mailing_list"', '"fact": "unread"'), "labels-text": ('Marketing and offers', 'Marketing, offers'),
                "providers": ('"id": "gemini"', '"id": "gem"'), "filter": ('"ai_description": "Emails', '"ai_description": "x Emails'),
                "sources": ('"enabled": false', '"enabled": true'), "auto": ('"auto": false', '"auto": true'), "refusal": ('"code": "ai_no_model"', '"code": "other"')}
        for nm, (a, b) in muts.items():
            mut = golden_static(golden, data.replace(a, b, 1), app)
            report(a in data and mut[0] == "FAIL", "mutant %s is red on firstrun-data-equals-golden (%s)" % (nm, mut[1]))
        mut = golden_static(golden, data, app.replace("applyProviders(", "dropProviders("))
        report(mut[0] == "FAIL", "mutant apply-dropped is red on firstrun-data-equals-golden (%s)" % mut[1])
        ids = ["acc-01", "acc-02"]
        want_state = golden["defaults"]["state"]
        served = {p: (v if v is None else json_unmask(v, ids)) for p, v in want_state.items()}
        d, u = compare_paths(golden, served.get, ids)
        report(d == [] and u == [], "install-state-equals-the-golden passes when every path answers the golden")
        class Silent:        # a fixture that serves none of the golden's paths: every one is unserved
            def send(self, verb, *a):
                if verb == "rows": return {"rows": [{"id": "acc-01"}, {"id": "acc-02"}]}
                if verb == "core": raise Unreadable("no core value at " + a[1])
                return {"ok": True}
        st_, txt_ = golden_driven(Silent(), golden)
        n_unserved = len([p_ for p_, v_ in want_state.items() if p_.startswith(SERVED_PREFIXES) and v_ is not None])
        report(st_ == "ok" and n_unserved > 12 and all(p_ in txt_ for p_, v_ in want_state.items() if p_.startswith(SERVED_PREFIXES) and v_ is not None),
               "install-state-equals-the-golden lists every one of the %d unserved paths, none cut at 12 (%d characters)" % (n_unserved, len(txt_)))
        bent = dict(served); bent["reply/use/auto"] = {"enabled": True}
        d, _ = compare_paths(golden, bent.get, ids)
        report(d == ["reply/use/auto"], "install-state-equals-the-golden is red when one served path differs (%s)" % d)
    src = read("Sources/ControlSocket/ShotVerb.swift")
    st = shot_static(src)
    report(st[0] == "ok", "shot cases on the real source: " + st[1])
    for k in ('hasPrefix("window:")', "is not a surface and not an open window", "SettingsPane.pane(for: slot) != nil"):
        report(shot_static(src.replace(k, "X"))[0] == "FAIL", "mutant (ShotVerb without %s) is red on the shot cases" % k[:30])
    good = dict((n_, s_) for n_, s_, _ in shot_driven(Rig7()))
    scenes, bridge = read("Sources/App/ShellScenes.swift"), read("Sources/App/WindowBridge.swift")
    st = titles_static(scenes, bridge)
    report(st[0] == "ok", "window-titles-are-words on the real source: " + st[1])
    mut = titles_static(scenes.replace(".navigationTitle(WindowBridge.title(for: slot))", ".navigationTitle(slot.id)"), bridge)
    report(mut[0] == "FAIL", "mutant id-put-back is red on window-titles-are-words (static: %s)" % mut[1])
    words = [{"id": "mac-main", "surface": "", "title": "Whoosh"}, {"id": "mac-composer", "surface": "mac-composer", "title": COMPOSER_WORD_EN}]
    report(titles_case(words)[0] == "ok", "window-titles-are-words passes on windows titled with words")
    mut = titles_case([dict(words[0]), dict(words[1], title="mac-composer")])
    report(mut[0] == "FAIL", "mutant id-put-back is red on window-titles-are-words (driven: %s)" % mut[1])
    report(titles_case([words[0], dict(words[1], title="iphone-composer")])[0] == "FAIL", "an iphone- title is red on window-titles-are-words")
    report(titles_case([words[0]])[0] == "NOT JUDGED", "no composer window is NOT JUDGED on window-titles-are-words, never a pass")
    report(titles_case([words[0], dict(words[1], title="Compose")], "en")[0] == "FAIL", "a composer title that is words but not the catalogue's is red on window-titles-are-words")
    report(titles_case([words[0], dict(words[1], title=sorted(catalogue_words(COMPOSER_KEY, "fr"))[0])], "fr")[0] == "ok", "the composer's French catalogue word passes when the app speaks French")
    report(titles_case([words[0], dict(words[1], title=COMPOSER_WORD_EN)], "fr")[0] == "FAIL", "the English composer word is red when the app speaks French")
    said_ = [{"ax": "ai-setup.mark.meaning", "shown": True, "words": "Search by meaning is off: the meaning model is not on this Mac."}, {"ax": "ai-setup.mark.writing.status", "shown": False, "words": ""}]
    report(marks_words(said_)[0] == "ok", "missing-marks-say-what-is-missing passes on a mark that says what is missing")
    report(marks_words([dict(said_[0], words=READY_SENTENCES[2])])[0] == "FAIL", "mutant: a mark shown as missing that says 'The meaning model is on this Mac.' is red on missing-marks-say-what-is-missing")
    report(marks_words([dict(said_[0], ax="ai-setup.mark.writing.status", words=READY_SENTENCES[0])])[0] == "FAIL", "mutant: the writing mark saying 'A provider is connected.' is red on missing-marks-say-what-is-missing")
    report(marks_words([said_[1]])[0] == "NOT JUDGED", "no mark shown is NOT JUDGED on missing-marks-say-what-is-missing, never a pass")
    foot_ = {"ax": FOOT_AI_WORDS, "text": "Reply with AI · connect a model", "need_w": 190.0, "box_w": 190.0, "need_h": 16.0, "box_h": 16.0}
    report(foot_uncut([foot_])[0] == "ok", "foot-strip-ai-entry-is-whole passes on an entry drawn at its own width")
    report(foot_uncut([dict(foot_, box_w=171.0)])[0] == "FAIL", "mutant: the AI entry in a box 19 pt narrower than its words is red on foot-strip-ai-entry-is-whole")
    report(foot_uncut([{"ax": "mac-main.foot.cap.open", "text": "Open", "need_w": 30.0, "box_w": 30.0}])[0] == "NOT JUDGED", "a strip that draws no AI entry is NOT JUDGED on foot-strip-ai-entry-is-whole, never a pass")
    names_ = [m[0] for m in MARK_PLACES]
    on_screen = {m[1] for m in MARK_PLACES if m[1]}
    report(len(MARK_PLACES) == 6, "the table holds six places (%d)" % len(MARK_PLACES))
    all_ = marks_drawn(names_, lambda ax: {"x": 0} if ax in on_screen else None)
    report(all_[0] == "ok" and "foot-reply-with-ai" in all_[1], "the foot's mark (mac-main.foot.mark) is judged with the other five on ai-setup-marks-are-drawn-in-their-six-places (%s)" % all_[1])
    saved = list(MARK_PLACES)
    MARK_PLACES[4] = ("foot-reply-with-ai", None)
    noid_ = marks_drawn(names_, lambda ax: {"x": 0} if ax in on_screen else None)
    MARK_PLACES[:] = saved
    report(noid_[0] == "NOT JUDGED" and "foot-reply-with-ai" in noid_[1], "a place the build draws no mark for is NOT JUDGED on ai-setup-marks-are-drawn-in-their-six-places, never a pass (%s)" % noid_[1])
    for place, ax in MARK_PLACES:
        if ax and place != "reader-summary":
            got = marks_drawn(names_, lambda a, _ax=ax: None if a == _ax else ({"x": 0} if a in on_screen else None))
            report(got[0] == "FAIL" and place in got[1], "mutant: %s listed and never drawn is red on ai-setup-marks-are-drawn-in-their-six-places (%s)" % (place, got[1][-70:]))
    report(marks_drawn(names_[:5], lambda ax: {"x": 0})[0] == "FAIL", "a place the row source stopped listing is red on ai-setup-marks-are-drawn-in-their-six-places")
    report(marks_drawn(names_, lambda ax: None if ax == "mac-main.foot.mark" else {"x": 0})[0] == "FAIL", "mutant: the foot's mark listed and never drawn is red on ai-setup-marks-are-drawn-in-their-six-places")
    st = settings_static(bridge)
    report(st[0] == "ok", "settings-pages-live-in-one-window on the real source: " + st[1])
    mut = settings_static(bridge.replace("if isSettingsSlot(slot) { open(id: settingsWindowId) }", "if slot.isSettingsPage { open(id: settingsWindowId) }"))
    report(mut[0] == "FAIL", "mutant generic-window is red on settings-pages-live-in-one-window (static: %s)" % mut[1])
    one = [{"id": "mac-main", "title": "Whoosh"}, {"id": "mac-settings", "title": "Settings"}]
    report(settings_one_window(one)[0] == "ok", "settings-pages-live-in-one-window passes on one Settings window")
    mut = settings_one_window(one + [{"id": "surface-AppWindow-1", "title": "mac-settings-replies"}])
    report(mut[0] == "FAIL", "mutant generic-window is red on settings-pages-live-in-one-window (driven: %s)" % mut[1])
    report(settings_one_window(one[:1])[0] == "FAIL", "no Settings window at all is red on settings-pages-live-in-one-window")
    report(set(good.values()) == {"ok"} and len(good) == 3, "the three shot cases pass on a window-true stand-in")
    for mu, case in (("ignores-window-arg", "shot-names-its-window"), ("serves-front", "shot-refuses-what-is-no-window"), ("opens-slot-window", "shot-opens-no-blank-window")):
        got = dict((n_, s_) for n_, s_, _ in shot_driven(Rig7(mu)))
        report(got.get(case) == "FAIL", "mutant %s is red on %s" % (mu, case))
    report(folder_gone_driven(Rig8())[0] == "ok", "reply-folder-gone-and-back passes on a stand-in that follows the rule")
    for mu in ("verb-does-nothing", "reset-keeps-gone"):
        report(folder_gone_driven(Rig8(mu))[0] == "FAIL", "mutant %s is red on reply-folder-gone-and-back" % mu)
    res = dict((n_, (s_, t_)) for n_, s_, t_ in drive(Model()))
    report(res["steps-change-the-digest"][0] == "ok" and res["reset-restores-the-launch-state"][0] == "ok", "the model passes both driven cases")
    # the reply step goes to a message the list holds (main79: a hard-coded msg-02 the copy no longer held was refused)
    report("reply" in res["steps-change-the-digest"][1].split("; ")[0].split(", "), "the reply step changed the digest: it named a message the model's list holds (%s)" % res["steps-change-the-digest"][1])
    try:
        Model().send("fixture", "reply", "msg-02")
        report(False, "mutant hard-coded reply target msg-02 is refused by the model (it was accepted)")
    except Unreadable as e:
        report(True, "mutant hard-coded reply target msg-02 is refused by the model (%s)" % e)
    res_s = dict((n_, (s_, t_)) for n_, s_, t_ in drive(Model(refuse_star=2)))
    report(res_s["steps-change-the-digest"][0] == "ok" and "star" in res_s["steps-change-the-digest"][1].split("; ")[0].split(", "),
           "S refused on the failing account's rows, taken on the third row: the star step changes the digest (%s)" % res_s["steps-change-the-digest"][1])
    res_s = dict((n_, (s_, t_)) for n_, s_, t_ in drive(Model(refuse_star=99)))
    report(res_s["steps-change-the-digest"][0] == "FAIL" and "star refused" in res_s["steps-change-the-digest"][1],
           "mutant: S does nothing on any row is still red on steps-change-the-digest (%s)" % res_s["steps-change-the-digest"][1][:90])
    report(drive(types.SimpleNamespace(send=lambda *a, _m=Model(): _m.send(*a) if a[:2] != ("rows", "mac-main.list") else {"rows": []}, uistate=lambda: {}))[0][1] == "NOT JUDGED",
           "an empty list leaves the digest case NOT JUDGED, never a pass")
    for cls in CLASSES:
        res = dict((n_, (s_, t_)) for n_, s_, t_ in drive(Model(skip=cls)))
        s_, t_ = res["reset-restores-the-launch-state"]
        names_it = all(p in t_ for p in CLASSES[cls] if p != "__view") and (cls != "view" or "view" in t_)
        report(s_ == "FAIL" and names_it, "mutant %s is red on reset-restores-the-launch-state (%s)" % (cls, t_.split("; ")[1] if "; " in t_ else t_))
    print("%d checks, %d not as expected" % (n, bad))
    return 1 if bad else 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    core, reset = open(CORE, encoding="utf-8").read(), open(RESET, encoding="utf-8").read()
    results = [("reset-covers-every-stored-property",) + static_case(core, reset)]
    golden, where = load_golden()
    if golden is None:
        results.append(("firstrun-data-equals-golden", "NOT JUDGED", "docs/FIRST-RUN.json is not there yet"))
    else:
        results.append(("firstrun-data-equals-golden",) + golden_static(golden, read(DATA_SWIFT), read(APPLY_SWIFT)))
    results.append(("window-titles-are-words (source)",) + titles_static(read("Sources/App/ShellScenes.swift"), read("Sources/App/WindowBridge.swift")))
    results.append(("settings-pages-live-in-one-window (source)",) + settings_static(read("Sources/App/WindowBridge.swift")))
    results.append(("shot cases (source)",) + shot_static(read("Sources/ControlSocket/ShotVerb.swift")))
    names = ("steps-change-the-digest", "reset-restores-the-launch-state", "no-control-under-the-title-bar", "keyevent-is-a-real-event",
             "install-state-new-is-a-new-installation", "relaunch-keeps-the-run-folder", "reply-folder-gone-and-back", "window-titles-are-words", "settings-pages-live-in-one-window",
             "ai-setup-marks-are-drawn-in-their-six-places", "missing-marks-say-what-is-missing", "foot-strip-ai-entry-is-whole", "frame-follows-the-scroll")
    results.append(("keyevent-is-a-real-event (source)",) + keyevent_static(read("Sources/ControlSocket/KeyEventVerb.swift")))
    results.append(("permission-seams-follow-the-rig-flag",) + seams_static(read("Sources/Features/SettingsCalendarsMac/CalendarsModel.swift"),
                   read("Sources/Features/Dictation/DictationRig.swift"), read("Sources/App/HiddenSystemUI.swift")))
    results.append(("relaunch-keeps-the-run-folder (source)",) + relaunch_static(read("scripts/launch-one.sh")))
    try:
        ctl = Ctl().connect()
    except CouldNotDrive as e:
        results += [(n, "NOT JUDGED", "no app to drive: %s" % e) for n in names]
    else:
        try:
            results += drive(ctl)
            results.append(("no-control-under-the-title-bar",) + band_case(ctl))
            results.append(("keyevent-is-a-real-event",) + keyevent_driven(ctl))
            results.append(("frame-follows-the-scroll",) + scroll_frame_driven(ctl))
            results += shot_driven(ctl)
            results.append(("settings-pages-live-in-one-window",) + settings_driven(ctl))
            results.append(("ai-setup-marks-are-drawn-in-their-six-places",) + marks_driven(ctl))
            said, foot = marks_words_driven(ctl)
            results.append(("missing-marks-say-what-is-missing",) + said)
            results.append(("foot-strip-ai-entry-is-whole",) + foot)
            results.append(("window-titles-are-words",) + titles_driven(ctl))
            results.append(("reply-folder-gone-and-back",) + folder_gone_driven(ctl))
            results.append(("install-state-equals-the-golden",) + (golden_driven(ctl, golden) if golden else ("NOT JUDGED", "no golden")))
            results.append(("install-state-new-is-a-new-installation",) + install_driven(ctl))
            ctl.send("fixture", "reset")
            results.append(("relaunch-keeps-the-run-folder",) + relaunch_driven(ctl))
        except Unreadable as e:
            results += [(n, "NOT JUDGED", str(e)) for n in names]
        finally:
            ctl.close()
    for n, s, t in results:
        print("%-8s %s: %s" % (s.upper(), n, t))
    f = sum(1 for _, s, _ in results if s == "FAIL")
    u = sum(1 for _, s, _ in results if s == "NOT JUDGED")
    print("%d checks, %d failed, %d not judged" % (len(results), f, u))
    return 1 if f or u else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
