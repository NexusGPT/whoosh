#!/usr/bin/env python3
"""sweep-settingscalendars: the control sweep of Settings > Agendas (`mac-settings-calendars`). Owner, 2026-11-11: "you haven't properly tested every button/feature".

ONE case per control, clicked once through its door and read through the rows the pane serves: the Agendas row of the Settings sidebar (the window's door
`mac-settings-window.pane-row.agendas`), Allow access (`mac-settings-calendars.access.ask`), Open System Settings (`.access.open-settings`) and the link
« Open Calendars » on Settings > Replies > Sources (`mac-settings-replies.sources.calendars.open`). A fifth case reads the pane's source and holds its control
constructors against that list, so a control added to the pane without a case turns it red.

THE FIXTURE CALENDARS ONLY. The pane runs on `FixtureCalendarAccess` on any rig copy (CalendarsModel.init: a rig copy never reaches EventKit), whose two calendars
are Travail and Personnel. Before it clicks Allow access the sweep sets the access state through the rig's own door `mac-settings-calendars.fixture.not-asked`, which the
app answers only when its access IS the fixture (CalendarsModel.fixtureSet); a refusal there stops the case NOT JUDGED and Allow access is never clicked.
Open System Settings opens the real System Settings window on a copy whose core is not the fixture (CalendarsModel.openSystemSettings), so its case first reads
`uistate.core` and is NOT JUDGED unless it says `fixture`. Findings: the real EventKit path of Allow access (EventKitCalendarAccess.requestAccess) and the real System Settings
window are never clicked (docs/lanes/gap-sweep-b.md, F-cal-1 and F-cal-2). The calendar rows are a reading (no control: CalendarListGroup).

    python3 checks/parity/sweep-settingscalendars.py              drive the running rig copy (hidden, --fixture-core)
    python3 checks/parity/sweep-settingscalendars.py --dry        the same code with no socket: the driven cases are `unjudged`, the count is exact
    python3 checks/parity/sweep-settingscalendars.py --selftest   every case against a model of the pane (checks/settingscalendars-check.py Fake, imported, not edited),
                                                                  clean, then once per case against its own broken copy: each must go red

Prints one `name  pass|fail|unjudged  detail` line per case (print_cases of journeys/lib/harness.py), the MUTANTS count, then 'N checks, F failed, U not judged'."""
import importlib.util
import json
import os
import re
import shutil
import socketserver
import sys
import tempfile
import threading

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, Unreadable                                     # noqa: E402
from lib.harness import Journey, run_journey, print_cases, declared_mutants   # noqa: E402

_spec = importlib.util.spec_from_file_location("sc", os.path.join(ROOT, "checks", "settingscalendars-check.py"))
sc = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(sc)                                            # read for its rows, its doors and its Fake; never edited

ID = "par-settingscalendars-sweep"
TITLE = "Settings > Agendas: every control clicked once"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "each control of the Agendas pane changes what the pane draws: the pane opens, access is asked of the fixture, System Settings is pressed once, the link returns to the pane"
P = sc.P
PANE_ROW, PLACE, SRC_LINK = sc.PANE_ROW, sc.PLACE, sc.SRC_LINK
SRC_DIR = sc.SRC_DIR
CASES = []      # (name, doors covered, the door the broken copy disables, fn(ctl))
STATIC = {"controls-in-the-source-are-the-listed-ones"}      # read the source, not the app: they pass in a dry run too


class Red(Exception):
    """The app answered and the answer is wrong: the case is red."""


def case(name, doors, noop, fn):
    CASES.append((name, list(doors), noop, fn))


def need(cond, words):
    if not cond:
        raise Red(words)


def hit(ctl, ax):
    """One click on a door; a refusal ("nothing to click", "did nothing") is the app's answer: False, and the outcome read after it shows the dead control."""
    try:
        ctl.click(ax)
        return True
    except Unreadable as e:
        if "nothing" in str(e) or "no control" in str(e):
            return False
        raise


def start(ctl, word="not-asked"):
    """The pane open, the rig's access in `word`. Only the rig's own doors move the access state, and only the fixture access answers them."""
    try:
        ctl.send("fixture", "reset")
    except Unreadable:
        pass
    try:
        sc.set_access(ctl, word)
    except sc.Refused as e:
        raise Unreadable("the access is not the fixture's (%s): no click on a control that could ask macOS" % e)
    hit(ctl, PANE_ROW)


def access(ctl):
    return ctl.rows(P + ".access")[0]


# ---------------------------------------------------------------- one case per control
def _pane_row(ctl):
    start(ctl)
    hit(ctl, "mac-settings-window.pane-row.general")
    before = ctl.rows(PLACE)[0]["pane"]
    hit(ctl, PANE_ROW)
    after = ctl.rows(PLACE)[0]["pane"]
    need(before == "general" and after == "agendas", "the Agendas row of the sidebar: the pane was %r, after the click %r" % (before, after))
    return "pane %s -> %s" % (before, after)


def _ask(ctl):
    start(ctl, "not-asked")
    a0 = access(ctl)
    need(a0["state"] == "not-asked" and a0["ask"] == P + ".access.ask", "setup: access %r, ask door %r" % (a0["state"], a0["ask"]))
    hit(ctl, P + ".access.ask")
    a1 = access(ctl)
    rows = [r["name"] for r in ctl.rows(P + ".list")]
    need(a1["state"] == "allowed" and a1["ask"] is None and len(rows) == 2, "Allow access: access %r, ask door %r, the fixture calendars %r" % (a1["state"], a1["ask"], rows))
    return "access not-asked -> allowed, the fixture calendars %s, the Allow access door gone" % rows


def _open_settings(ctl):
    st = ctl.uistate()
    core, rig = st.get("core"), bool((st.get("hidden") or {}).get("on"))
    # F-cal-2 (closed): a rig copy on ANY core only counts the press (CalendarsModel.openSystemSettings opens the system window only off a rig, `!CorePlace.onRig`),
    # so the case is judged on a fixture core or a hidden rig; a copy that is neither would open the real System Settings window and is not clicked.
    if core != "fixture" and not rig:
        raise Unreadable("the app's core is %r and it is not a hidden rig: Open System Settings would open the real System Settings window; not clicked" % core)
    start(ctl, "refused")
    a0 = access(ctl)
    need(a0["state"] == "refused" and a0["open_settings"] == P + ".access.open-settings" and a0["opened_settings"] == 0, "setup: access %r, presses %r" % (a0["state"], a0["opened_settings"]))
    hit(ctl, P + ".access.open-settings")
    a1 = access(ctl)
    need(a1["opened_settings"] == 1, "Open System Settings: the rig counted %r presses, expected 1" % a1["opened_settings"])
    return "Open System Settings: presses 0 -> %d (the rig only counts, no window opens)" % a1["opened_settings"]


def _link(ctl):
    start(ctl, "allowed")
    hit(ctl, "mac-settings-window.pane-row.general")
    before = ctl.rows(PLACE)[0]["pane"]
    hit(ctl, SRC_LINK)
    after = ctl.rows(PLACE)[0]["pane"]
    need(before == "general" and after == "agendas", "the Calendars link of Sources: the pane was %r, after the click %r" % (before, after))
    return "pane %s -> %s" % (before, after)


BUTTONS = r"\b(SettingsButton|Button|Toggle|Picker|Link|TextField|Menu|Slider|Stepper)\(|\.onTapGesture|\.onKeyPress|\.keyboardShortcut|\.onDrop|\.contextMenu"
LISTED = {"CalendarsPaneView.swift": ["SettingsButton(title: CalendarWords.allowAccess", "SettingsButton(title: CalendarWords.openSettings"]}


def controls_in_source(root):
    """{file: [the first words of every control constructor]} for the pane's folder, files that draw no pane (the access, the fixture, the words) included."""
    out = {}
    d = os.path.join(root, "Sources", "Features", "SettingsCalendarsMac")
    for f in sorted(os.listdir(d)):
        if f.endswith(".swift"):
            for line in open(os.path.join(d, f), encoding="utf-8"):
                m = re.search(BUTTONS, line)
                if m and not line.lstrip().startswith("///") and not line.lstrip().startswith("//"):
                    out.setdefault(f, []).append(line.strip()[line.strip().find(m.group(0)):][:48])
    return out


def _source(ctl):
    got = controls_in_source(SRC_ROOT[0])
    same = set(got) == set(LISTED) and all(len(got[k]) == len(LISTED[k]) and all(g.startswith(w) for g, w in zip(got[k], LISTED[k])) for k in LISTED)
    want = LISTED
    need(same, "the pane draws controls the sweep does not list: %r, expected %r" % (got, want))
    return "the pane's source holds %d control constructors, the two buttons the cases click: %s" % (sum(len(v) for v in got.values()), [x for v in got.values() for x in v])


SRC_ROOT = [ROOT]
case("agendas-sidebar-row-opens-the-pane", ["mac-settings-window.pane-row.agendas"], "mac-settings-window.pane-row.agendas", _pane_row)
case("allow-access-asks-the-fixture-and-lists-its-calendars", [P + ".access.ask"], P + ".access.ask", _ask)
case("open-system-settings-presses-once", [P + ".access.open-settings"], P + ".access.open-settings", _open_settings)
case("sources-calendars-link-opens-the-pane", [SRC_LINK], SRC_LINK, _link)
case("controls-in-the-source-are-the-listed-ones", [], "source:extra-button", _source)

# the lines the generator cannot match by their text (the ids are built from CalendarsIds)
SRC = {
    "agendas-sidebar-row-opens-the-pane": "SettingsWindowRegistry.swift:out[\"\\(W).pane-row.\\(p.id)\"]",
    "allow-access-asks-the-fixture-and-lists-its-calendars": "CalendarsPaneView.swift:SettingsButton(title: CalendarWords.allowAccess",
    "open-system-settings-presses-once": "CalendarsPaneView.swift:SettingsButton(title: CalendarWords.openSettings",
    "sources-calendars-link-opens-the-pane": "SettingsCalendarsMacFeature.swift:\"mac-settings-replies.sources.calendars.open\"",
    "controls-in-the-source-are-the-listed-ones": "CalendarsPaneView.swift:SettingsFootnote(text: CalendarWords.footnote",
}
CHANGE = {
    "agendas-sidebar-row-opens-the-pane": "the Agendas row's door does nothing: `{ _ in _ = model.select(p); return true }` becomes `{ _ in true }` for p = agendas",
    "allow-access-asks-the-fixture-and-lists-its-calendars": "the button's action `{ model.ask() }` becomes `{ }`",
    "open-system-settings-presses-once": "the button's action `{ model.openSystemSettings() }` becomes `{ }`",
    "sources-calendars-link-opens-the-pane": "the link's door does nothing: `{ _ in openPane() }` becomes `{ _ in true }`",
    "controls-in-the-source-are-the-listed-ones": "one more control is drawn: a line `SettingsButton(title: CalendarWords.reading, ax: \"x\") { }` is added after this one",
}


# ---------------------------------------------------------------- the run
def run_case(s, name, fn):
    def run():
        if name in STATIC:
            try:
                return True, fn(None)
            except Red as e:
                return False, str(e)
        ctl = s.ctl
        try:
            return True, fn(ctl)
        except Red as e:
            return False, str(e)
        except sc.Refused as e:
            return False, "an action was refused: %s" % e
    s._record(name, run)


def drive(j, only=None):
    s = j.step(ID, TITLE)
    for name, _doors, _noop, fn in CASES:
        if only is None or name == only:
            run_case(s, name, fn)


# MUTANTS: per case, the one-line change of the app that must turn it red; the selftest makes the same change in its model (a dead door).
# Format: # <step>/<case> | <file>:<line>: the change
# par-settingscalendars-sweep/agendas-sidebar-row-opens-the-pane | Sources/Features/SettingsWindow/SettingsWindowRegistry.swift:135: the Agendas row's door does nothing: `{ _ in _ = model.select(p); return true }` becomes `{ _ in true }` for p = agendas (line: for p in SettingsPane.allCases { out["\(W).pane-row.\(p.id)"] = { _ in _ = model.select(p); return true } })
# par-settingscalendars-sweep/allow-access-asks-the-fixture-and-lists-its-calendars | Sources/Features/SettingsCalendarsMac/CalendarsPaneView.swift:45: the button's action `{ model.ask() }` becomes `{ }` (line: SettingsButton(title: CalendarWords.allowAccess, primary: true, ax: CalendarsIds.ask) { model.ask() })
# par-settingscalendars-sweep/open-system-settings-presses-once | Sources/Features/SettingsCalendarsMac/CalendarsPaneView.swift:65: the button's action `{ model.openSystemSettings() }` becomes `{ }` (line: if i == 0 { SettingsButton(title: CalendarWords.openSettings, ax: CalendarsIds.openSettings) { model.openSy...)
# par-settingscalendars-sweep/sources-calendars-link-opens-the-pane | Sources/Features/SettingsCalendarsMac/SettingsCalendarsMacFeature.swift:58: the link's door does nothing: `{ _ in openPane() }` becomes `{ _ in true }` (line: "mac-settings-replies.sources.calendars.open": { _ in openPane() },)
# par-settingscalendars-sweep/controls-in-the-source-are-the-listed-ones | Sources/Features/SettingsCalendarsMac/CalendarsPaneView.swift:13: one more control is drawn: a line `SettingsButton(title: CalendarWords.reading, ax: "x") { }` is added after this one (line: SettingsFootnote(text: CalendarWords.footnote, ax: CalendarsIds.footnote))
# MUTANTS-END


# ---------------------------------------------------------------- the self-test: the model is checks/settingscalendars-check.py Fake; a broken copy makes one door dead
class Model(sc.Fake):
    def __init__(self, mutants=()):
        super().__init__(mutants, stored="en")

    def v_click(self, ax):
        if "dead:" + ax in self.m:
            return {"ok": True}
        return super().v_click(ax)

    def v_uistate(self):
        return {"ok": True, "state": {"surface": "mac-settings-" + self.pane, "core": "fixture"}}


def serve(mutants):
    d = tempfile.mkdtemp(prefix="sw-cal-")
    srv = socketserver.ThreadingUnixStreamServer(os.path.join(d, "whoosh.sock"), sc._Handler)
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


def source_copy(extra):
    t = tempfile.mkdtemp(prefix="sw-cal-src-")
    shutil.copytree(os.path.join(ROOT, "Sources", "Features", "SettingsCalendarsMac"), os.path.join(t, "Sources", "Features", "SettingsCalendarsMac"))
    if extra:
        p = os.path.join(t, "Sources", "Features", "SettingsCalendarsMac", "CalendarsPaneView.swift")
        open(p, "a", encoding="utf-8").write('\nstruct Extra: View { var body: some View { SettingsButton(title: CalendarWords.reading, ax: "x") { } } }\n')
    return t


def selftest():
    bad = 0
    clean = model_run(())
    for name, (st, why) in clean.items():
        print("model %-58s %-8s %s" % (name, st, " ".join(why.split())[:150]))
        bad += st != "pass"
    declared = {l.split(" | ")[0][2:].strip().split("/", 1)[1] for l in open(os.path.abspath(__file__), encoding="utf-8") if l.startswith("# " + ID + "/")}
    for name, _ds, noop, _fn in CASES:
        if name not in declared:
            print("NO MUTANT declared for %s" % name)
            bad += 1
        if noop.startswith("source:"):
            t = source_copy(True)
            SRC_ROOT[0] = t
            try:
                st = "fail" if drive_one_static(name) else "pass"
            finally:
                SRC_ROOT[0] = ROOT
                shutil.rmtree(t, ignore_errors=True)
            print("mutant %-58s %-32s -> %s" % (name, noop, "RED" if st == "fail" else "NOT RED"))
            bad += st != "fail"
            continue
        st = model_run(("dead:" + noop,), only=name)[name][0]
        print("mutant %-58s %-32s -> %s" % (name, "dead:" + noop, "RED" if st == "fail" else "NOT RED (%s)" % st))
        bad += st != "fail"
    for l in sorted(declared - {c[0] for c in CASES}):
        print("STRAY MUTANT %s" % l)
        bad += 1
    dry = Journey(ID, None)
    drive(dry)
    states = {c.label.split("/", 1)[1]: c.state for c in dry.checks()}
    ok = len(states) == len(CASES) and all(st == ("pass" if n in STATIC else "unjudged") for n, st in states.items())
    print("whole check, undriven: %d cases (the driven ones unjudged, the source one read): %s" % (len(states), ok))
    bad += not ok
    print("selftest: %d cases, %d on the clean model green, %d mutants, %d not red or missing" % (len(CASES), sum(1 for v in clean.values() if v[0] == "pass"), len(declared), bad))
    if not bad:
        print("selftest: every check was seen red on its broken model")
    return 1 if bad else 0


def drive_one_static(name):
    try:
        [fn for n, _d, _m, fn in CASES if n == name][0](None)
        return False
    except Red:
        return True


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
