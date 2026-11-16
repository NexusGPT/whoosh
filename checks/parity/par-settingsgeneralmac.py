#!/usr/bin/env python3
"""par-settingsgeneralmac: Settings > General round 2, the language picker, the pinned-labels wording and the emblem switch.

Drives the RUNNING rig copy over the control socket (docs/CONTROL-SOCKET.md). Expected French words are read from
Sources/Resources/Localizable.xcstrings (the catalogue is the source of what French says), never typed here; the
stored choice is also read from the host with `defaults read`, not from the app's own report.
Start from a clean rig domain (`defaults delete io.nexus.whoosh.rig`): the check reads the defaults the app ships with.
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/par-settingsgeneralmac.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
Sweep (lane sweep-settingsgeneral, step par-settingsgeneralmac-sweep): the controls of the General pane no case clicked (every pinned-label switch),
and the app's own control list held against the inventory; docs/lanes/sweep-settingsgeneral.md holds the control table.
    python3 checks/parity/par-settingsgeneralmac.py --selftest   the sweep step against a model of the pane, clean (all green), then once per mutant (each must go red)
"""
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey                                  # noqa: E402

ID = "par-settingsgeneralmac"
TITLE = "Settings > General, round 2: language, pinned labels, account emblems"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "choosing Français turns the main window French at once and the choice stays; pinned labels are the tabs; emblems off widens the text"
P, MAIN = "mac-settings-general", "mac-main"
DOMAIN = "io.nexus.whoosh.rig"
CATALOGUE = json.load(open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8"))["strings"]
OWN_NAMES = [("da", "Dansk"), ("de", "Deutsch"), ("en", "English"), ("es", "Español"), ("fi", "Suomi"), ("fr", "Français"),
             ("it", "Italiano"), ("ja", "日本語"), ("nb", "Norsk bokmål"), ("nl", "Nederlands"), ("pl", "Polski"),
             ("pt", "Português"), ("sv", "Svenska"), ("zh-Hans", "简体中文")]
PANE_TABS = ["Shortcuts", "Notch", "Notifications", "Privacy", "Appearance", "Sync"]
REASON = "Could not save the language. Try again."


def word(key, code):
    return CATALOGUE[key]["localizations"][code]["stringUnit"]["value"]


def settings(s):
    return s.ctl.rows(P + ".settings")[0]


def tab_titles(s):
    return [r["title"] for r in s.ctl.rows(MAIN + ".tabs")]


def stored():
    out = subprocess.run(["defaults", "read", DOMAIN, "whoosh.language"], capture_output=True, text=True, timeout=10)
    return out.stdout.strip() if out.returncode == 0 else "system"


def mac_code():
    """The catalogue language the Mac itself prefers, read from the host: what "System" must give back."""
    # A test copy has its own language list (the build job writes `AppleLanguages` into the test copy's settings on purpose, and the launcher keeps it): the
    # app follows the list its process is given, so "System" on a test copy is that list's first language. The Mac's own list answers when the copy has none.
    out = ""
    for domain in ("io.nexus.whoosh.rig", "-g"):
        r = subprocess.run(["defaults", "read", domain, "AppleLanguages"], capture_output=True, text=True, timeout=10)
        if r.returncode == 0 and r.stdout.strip():
            out = r.stdout
            break
    first = [l.strip().strip('",') for l in out.splitlines()[1:] if l.strip() not in (")", "")][0]
    codes = [c for c, _ in OWN_NAMES]
    return next((c for c in sorted(codes, key=len, reverse=True) if first.lower().startswith(c.lower())), "en")


def primary(code):
    return word("Primary", code)


def pick(s, code):
    s.click("%s.appearance.language.%s" % (P, code))


def drive(j):
    s = j.step(ID, "Language, pinned labels, emblems")
    pick(s, "system")
    s.expect("language-popup-options", lambda: [(r["code"], r["name"]) for r in s.ctl.rows(P + ".languages")][1:], OWN_NAMES)
    s.expect("language-system-first-and-default", lambda: ([r["code"] for r in s.ctl.rows(P + ".languages")][0], settings(s)["language"]), ("system", "system"))
    pick(s, "en")
    s.expect("main-window-english-before", lambda: (settings(s)["language"], "Primary" in tab_titles(s)), ("en", True))

    # the language to pick is never the Mac's own, so a choice that does nothing cannot pass by luck
    L = "fr" if (not j.ctl or mac_code() != "fr") else "de"
    pick(s, L)
    s.expect("language-saved-in-settings", lambda: settings(s)["language"], L)
    s.expect("language-stored-on-the-host", lambda: (s.ctl, stored())[1], L)
    s.expect("main-window-turns-french", lambda: primary(L) in tab_titles(s) and "Primary" not in tab_titles(s), True)
    s.expect("settings-pane-turns-french", lambda: [r["title"] for r in s.ctl.rows(P + ".tabs")], [word(t, L) for t in PANE_TABS])
    s.shot("par-settingsgeneralmac-" + L)

    pick(s, "system")
    s.expect("back-to-system-gives-the-macs-language", lambda: (primary(mac_code()) in tab_titles(s), (s.ctl, stored())[1]), (True, "system"))

    def every_language():
        out = {}
        for code, _ in OWN_NAMES:
            pick(s, code)
            out[code] = (settings(s)["language"], tab_titles(s)[0] == primary(code)) if "Primary" in CATALOGUE else None
        pick(s, "system")
        return out
    s.expect("all-14-languages-apply", every_language, {c: (c, True) for c, _ in OWN_NAMES})

    if j.ctl is not None:
        s.raw("fixture", "fail-next", "language-save", REASON)
    pick(s, L)
    s.expect("refused-language-save-keeps-old-and-says-why",
             lambda: (settings(s)["language"], [r["text"] for r in s.ctl.rows(P + ".language.error")]), ("system", [REASON]))
    pick(s, "system")

    pick(s, "en")
    heading = lambda: s.ctl.rows(P + ".pinned.heading")[0]
    s.expect("pinned-labels-wording", lambda: heading()["text"], "Pinned labels")
    s.expect("no-split-tabs-wording", lambda: "split" in (heading()["text"] + heading()["line"]).lower(), False)

    chosen = {}

    def unpin_one():
        rows = [r for r in s.ctl.rows(P + ".pinned") if r["pinned"]]
        chosen["row"] = rows[-1]
        before = list(settings(s)["pinned_labels"])
        s.click(chosen["row"]["ax"])
        after = list(settings(s)["pinned_labels"])
        return (len(rows) > 1, before[:-1] == after, chosen["row"]["name"] in tab_titles(s))
    s.expect("unpin-removes-the-tab", unpin_one, (True, True, False))
    s.expect("pin-again-restores-the-tab", lambda: (s.ctl, s.click(chosen["row"]["ax"]), chosen["row"]["name"] in settings(s)["pinned_labels"])[2], True)

    s.expect("emblems-default-on", lambda: (settings(s)["emblems"], {r["emblem"] for r in s.ctl.rows(MAIN + ".list")}), (True, {"row"}))
    wide = {}

    def text_width():   # the text's left edge: it moves left by the emblem's width when the emblem goes
        first = s.ctl.rows(MAIN + ".list")[0]["id"]
        return s.ctl.measure("%s.list.row.%s.text" % (MAIN, first))["frame"]["x"]
    wide["on"] = None

    def emblem_off():
        wide["on"] = text_width()
        s.click(P + ".appearance.emblems")
        return ({r["emblem"] for r in s.ctl.rows(MAIN + ".list")}, settings(s)["emblems"])
    s.expect("emblems-off-removes-every-emblem", emblem_off, ({"hidden"}, False))
    s.expect("emblems-off-text-takes-the-width", lambda: text_width() < wide["on"], True)
    s.click(P + ".appearance.emblems")
    s.expect("emblems-on-again", lambda: {r["emblem"] for r in s.ctl.rows(MAIN + ".list")}, {"row"})
    # Owner 13 Nov, pictures 213 and 214: web pictures load when a mail opens; this switch goes back to ask first. A test copy starts with it off (it may ask
    # no server off this Mac). The mail view's own check (messagebody-check J1.6f) reads what the rule does to an opened mail.
    pics = {}
    s._act(lambda: pics.update(start=settings(s)["pictures_on_open"]))
    s.click(P + ".privacy.pictures")
    s.expect("pictures-switch-flips-the-rule", lambda: settings(s)["pictures_on_open"], (not pics["start"]) if "start" in pics else None)
    s.click(P + ".privacy.pictures")
    s.expect("pictures-switch-flips-back", lambda: settings(s)["pictures_on_open"], pics.get("start"))
    pick(s, "system")
    drive_sweep(j)


W = "mac-settings-window"
CONSTANTS = open(os.path.join(ROOT, "Sources", "CoreBridge", "Generated", "Constants.swift"), encoding="utf-8").read()


def constant(name):
    import re
    return int(re.search(r"static let %s: Int = (\d+)" % name, CONSTANTS).group(1))


UNDO_CHOICES = list(range(constant("UNDO_SEND_MIN"), constant("UNDO_SEND_MAX") + 1, constant("UNDO_SEND_MIN")))
PIN_MAX = constant("TAB_PINNED_MAX")


def flat(row):
    """The settings row with its pinned labels sorted: pinning a label again may put it last in the core's own order, which is not the screen's state."""
    return dict(row, pinned_labels=sorted(row["pinned_labels"]))


def inventory_of(s):
    """Every control of the General pane the sweep names: the ids the app must list (the pinned-label switches come from the pane's own rows)."""
    ax = [P + ".appearance.language.system"] + [P + ".appearance.language." + c for c, _ in OWN_NAMES]
    ax += [P + ".appearance.theme." + a for a in ("auto", "light", "dark")]
    ax += [P + ".appearance.density." + d for d in ("regular", "compact")]
    ax += [P + ".appearance.motion." + m for m in ("full", "reduced")]
    ax += ["%s.appearance.undo-send.%d" % (P, n) for n in UNDO_CHOICES]
    ax += [P + ".appearance.emblems", P + ".privacy.pictures"]
    ax += [r["ax"] for r in s.ctl.rows(P + ".pinned")]
    return ax


def drive_sweep(j):
    """Every control of Settings > General, clicked once with its outcome read: the cases above click the language choices (every one of the 14 and System)
    and the emblem switch; par-settingswindow clicks the three looks, the two densities, the two motions and the six undo delays (step 5); this step
    clicks every pinned-label switch (the cases above clicked one) and holds the app's own control list against the inventory."""
    s = j.step("par-settingsgeneralmac-sweep", "Settings > General: every control clicked once")
    start = {}

    def listed_controls():
        start["settings"] = flat(settings(s))
        start["pinned"] = {r["ax"]: r["pinned"] for r in s.ctl.rows(P + ".pinned")}
        want = set(inventory_of(s))
        listed = {r["ax"] for r in s.ctl.rows(W + ".controls") if r["pane"] == "general"}
        return (sorted(want - listed), sorted(a for a in listed - want if ".pinned.label." not in a), len(want) > 20)
    s.expect("general-controls-are-the-listed-ones", listed_controls, ([], [], True))
    pick(s, "en")      # a label's own name is the tab's title: the sweep reads both in English, then puts the language back

    def every_pin_switch():
        bad = []
        for r in s.ctl.rows(P + ".pinned"):
            before = {x["ax"]: x["pinned"] for x in s.ctl.rows(P + ".pinned")}
            n = sum(1 for v in before.values() if v)
            refused = (r["pinned"] and n <= 1) or (not r["pinned"] and n >= PIN_MAX)      # the core keeps one tab and at most TAB_PINNED_MAX
            s.ctl.click(r["ax"])
            after = {x["ax"]: x["pinned"] for x in s.ctl.rows(P + ".pinned")}
            want = r["pinned"] if refused else not r["pinned"]
            seen = (after[r["ax"]], r["name"] in tab_titles(s), r["name"] in settings(s)["pinned_labels"])
            if seen != (want, want, want):
                bad.append((r["name"], "pinned/tab/settings", seen, "wanted", want))
            if {k: v for k, v in after.items() if k != r["ax"]} != {k: v for k, v in before.items() if k != r["ax"]}:
                bad.append((r["name"], "another label changed"))
            if refused:
                if not [e for e in s.ctl.rows(P + ".pinned.error") if e.get("text")]:
                    bad.append((r["name"], "a refused switch says nothing"))
            else:
                s.ctl.click(r["ax"])      # put it back
                if {x["ax"]: x["pinned"] for x in s.ctl.rows(P + ".pinned")} != before:
                    bad.append((r["name"], "not put back"))
        return bad
    s.expect("every-pin-switch-flips-its-label-and-its-tab", every_pin_switch, [])

    def back_as_it_was():
        s.ctl.click("%s.appearance.language.%s" % (P, start["settings"]["language"]))
        return (flat(settings(s)) == start["settings"], {r["ax"]: r["pinned"] for r in s.ctl.rows(P + ".pinned")} == start["pinned"])
    s.expect("sweep-leaves-the-screen-as-it-was", back_as_it_was, (True, True))


# MUTANTS: per check, the mutation of the app that must turn it red.
# par-settingsgeneralmac/language-popup-options | the popup lists fewer languages, in another order or under English names
# par-settingsgeneralmac/language-system-first-and-default | System is not first or the default is a language
# par-settingsgeneralmac/main-window-english-before | choosing English leaves the main window in another language
# par-settingsgeneralmac/language-saved-in-settings | choosing a language changes nothing
# par-settingsgeneralmac/language-stored-on-the-host | the choice lives in memory only and is never stored
# par-settingsgeneralmac/main-window-turns-french | the choice is stored but the main window keeps its old words
# par-settingsgeneralmac/settings-pane-turns-french | the Settings pane keeps English after the choice
# par-settingsgeneralmac/back-to-system-gives-the-macs-language | System keeps the language chosen before instead of the Mac's
# par-settingsgeneralmac/all-14-languages-apply | one language of the 14 is ignored or maps to another
# par-settingsgeneralmac/refused-language-save-keeps-old-and-says-why | a refused save still changes the language or hides the reason
# par-settingsgeneralmac/pinned-labels-wording | the section is still called Split tabs
# par-settingsgeneralmac/no-split-tabs-wording | the copy under the heading still says split tabs
# par-settingsgeneralmac/unpin-removes-the-tab | the pin switch changes nothing in the tab strip
# par-settingsgeneralmac/pin-again-restores-the-tab | pinning again does not bring the tab back
# par-settingsgeneralmac/emblems-default-on | the emblem is off by default
# par-settingsgeneralmac/emblems-off-removes-every-emblem | the switch leaves emblems on some rows
# par-settingsgeneralmac/emblems-off-text-takes-the-width | the text stays where the emblem left room when it is off
# par-settingsgeneralmac/emblems-on-again | switching back on does not bring the emblems back
# par-settingsgeneralmac/pictures-switch-flips-the-rule | the pictures switch does nothing
# par-settingsgeneralmac/pictures-switch-flips-back | the pictures switch only flips once
# par-settingsgeneralmac-sweep/general-controls-are-the-listed-ones | the app's control list leaves out a control of the pane (here the density choice) or lists one the pane lacks
# par-settingsgeneralmac-sweep/every-pin-switch-flips-its-label-and-its-tab | one label's pin switch changes nothing
# par-settingsgeneralmac-sweep/sweep-leaves-the-screen-as-it-was | a pin switch also changes the density, so the sweep cannot put the screen back


# ---- the selftest: the sweep step against a model of the General pane, clean and once per mutant ----
# GModel answers the verbs drive_sweep uses, the way Sources/Features/SettingsGeneralMac (SettingsGeneralMacFeature.rowSources and clickTargets,
# GeneralStore.togglePin) and SettingsWindowRegistry.controls are written to answer them. One mutant (named by its case) is switched on at a time.
class GModel:
    LABELS = [("lab-needs", "Needs Reply", True), ("lab-updates", "Updates", True), ("lab-promos", "Promotions", True),
              ("lab-news", "Newsletters", False), ("lab-receipts", "Receipts", False)]
    VIEWS = ["view-primary", "view-all"]       # views are listed as controls by the window, never as pinnable rows of the pane

    def __init__(self, mut):
        self.m = mut
        self.lang, self.appearance, self.density, self.motion, self.undo, self.emblems = "system", "auto", "regular", "full", 10, True
        self.pins = {i: (n, p) for i, n, p in self.LABELS}
        self.error = ""

    def on(self, name): return name in self.m

    def pinned_names(self): return [n for n, p in self.pins.values() if p]

    def controls(self):
        out = [{"pane": "window", "ax": W + ".back", "kind": "button"}]
        out += [{"pane": "general", "ax": P + ".appearance.language." + c, "kind": "choice"} for c in ["system"] + [c for c, _ in OWN_NAMES]]
        out += [{"pane": "general", "ax": P + ".appearance.theme." + a, "kind": "choice"} for a in ("auto", "light", "dark")]
        if not self.on("general-controls-are-the-listed-ones"):
            out += [{"pane": "general", "ax": P + ".appearance.density." + d, "kind": "choice"} for d in ("regular", "compact")]
        out += [{"pane": "general", "ax": P + ".appearance.motion." + m, "kind": "choice"} for m in ("full", "reduced")]
        out += [{"pane": "general", "ax": "%s.appearance.undo-send.%d" % (P, n), "kind": "choice"} for n in UNDO_CHOICES]
        out += [{"pane": "general", "ax": P + ".appearance.emblems", "kind": "switch"}]
        out += [{"pane": "general", "ax": P + ".privacy.pictures", "kind": "switch"}]
        out += [{"pane": "general", "ax": P + ".pinned.label." + i, "kind": "switch"} for i in list(self.pins) + self.VIEWS]
        out += [{"pane": "notifications", "ax": P + ".notch.show", "kind": "switch"}]
        return out

    def rows(self, ax):
        if ax == W + ".controls": return self.controls()
        if ax == P + ".settings":
            return [{"language": self.lang, "appearance": self.appearance, "density": self.density, "motion": self.motion, "undo_seconds": self.undo,
                     "emblems": self.emblems, "pinned_labels": self.pinned_names()}]
        if ax == P + ".pinned": return [{"ax": P + ".pinned.label." + i, "name": n, "pinned": p} for i, (n, p) in self.pins.items()]
        if ax == P + ".pinned.error": return [{"text": self.error}] if self.error else []
        if ax == MAIN + ".tabs": return [{"title": t} for t in ["Primary", "All"] + self.pinned_names()]
        return []

    def click(self, ax):
        if ax.startswith(P + ".appearance.language."):
            self.lang = ax.rsplit(".", 1)[1]; return True
        if ax.startswith(P + ".pinned.label."):
            i = ax.rsplit(".", 1)[1]
            if i not in self.pins: return False
            if self.on("every-pin-switch-flips-its-label-and-its-tab") and i == "lab-news": return True
            n, p = self.pins[i]
            if (p and len(self.pinned_names()) <= 1) or (not p and len(self.pinned_names()) >= PIN_MAX):
                self.error = "Keep at least one tab." if p else "Eight tabs at most."; return True
            self.error = ""
            self.pins[i] = (n, not p)
            if self.on("sweep-leaves-the-screen-as-it-was"): self.density = "compact"
            return True
        return False

    def handle(self, verb, args):
        if verb == "click": return {"ok": True} if self.click(args[0]) else {"ok": False, "error": "did nothing"}
        if verb == "rows": return {"ok": True, "rows": self.rows(args[0])}
        return {"ok": False, "error": "nothing"}


def model_run(mut):
    """Drives the sweep step over a socket served by GModel with the mutants `mut` on; returns {check label: (state, detail)}."""
    import json as _json
    import shutil
    import socketserver
    import tempfile
    import threading
    from lib.ctl import Ctl                                              # noqa: E402
    from lib.harness import Journey                                      # noqa: E402
    model = GModel(mut)
    d = tempfile.mkdtemp(prefix="par-sgm-")

    class H(socketserver.StreamRequestHandler):
        def handle(self):
            for line in self.rfile:
                q = _json.loads(line.decode())
                self.wfile.write((_json.dumps(model.handle(q["verb"], q["args"])) + "\n").encode())
    srv = socketserver.ThreadingUnixStreamServer(os.path.join(d, "whoosh.sock"), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    ctl = None
    try:
        ctl = Ctl(d).connect()
        j = Journey(ID, ctl)
        drive_sweep(j)
        return {c.label: (c.state, c.detail) for c in j.checks()}
    finally:
        if ctl is not None:
            ctl.close()
        srv.shutdown(); srv.server_close()
        shutil.rmtree(d, ignore_errors=True)


def selftest():
    path = os.path.abspath(__file__)
    clean = model_run(set())
    bad = 0
    for label, (state, why) in clean.items():
        if state != "pass":
            print("RED on the model: %s %s" % (label, why)); bad += 1
    print("model, no mutant: %d of %d checks green" % (sum(1 for st, _ in clean.values() if st == "pass"), len(clean)))
    declared = {l.split(" | ")[0][2:].strip(): l.split(" | ", 1)[1].strip() for l in open(path, encoding="utf-8") if l.startswith("# par-settingsgeneralmac-sweep/")}
    for label in clean:
        name = label.split("/", 1)[1]
        if label not in declared:
            print("NO MUTANT %s" % label); bad += 1; continue
        got = model_run({name})
        state = got[label][0]
        others = sorted(k.split("/", 1)[1] for k, (st, _) in got.items() if st == "fail" and k != label)
        print("mutant %-52s -> %s%s" % (name, "RED" if state == "fail" else "NOT RED (%s)" % state, ("  also red: " + ", ".join(others)) if others else ""))
        if state != "fail" or others: bad += 1
    for l in sorted(set(declared) - set(clean)):
        print("STRAY MUTANT %s" % l); bad += 1
    # the whole drive, undriven (no socket: nothing is launched or clicked): every case it records has its declared mutant and no declared mutant is stray
    from lib.harness import Journey, declared_mutants                    # noqa: E402
    dry = Journey(ID, None)
    drive(dry)
    labels = {c.label for c in dry.checks()}
    names = set(declared_mutants(path))
    for l in sorted(labels - names):
        print("NO MUTANT (whole check) %s" % l); bad += 1
    for l in sorted(names - labels):
        print("STRAY MUTANT (whole check) %s" % l); bad += 1
    print("whole check, undriven: %d cases, %d declared mutants" % (len(labels), len(names)))
    print("selftest: %d sweep checks, %d mutants, %d not red or missing" % (len(clean), len(declared), bad))
    return 1 if bad else 0


def main():
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    rows_ = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# par-") and " | " in l]
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant |")
    print("|---|---|")
    for k, v in rows_:
        print("| %s | %s |" % (k, v))
    for line in r["fail_list"]:
        sys.stderr.write("FAIL %s\n" % line)
    for line in r["no_mutant"]:
        sys.stderr.write("NO MUTANT %s\n" % line)
    for line in r["stray_mutant"]:
        sys.stderr.write("STRAY MUTANT %s\n" % line)
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    ok = r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else main())
