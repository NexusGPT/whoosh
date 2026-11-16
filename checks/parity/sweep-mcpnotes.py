#!/usr/bin/env python3
"""sweep-mcpnotes: the control sweep of the MCP notes screen `mac-mcp-notes` (the window) and its twin in Settings (`mac-settings-mcp`).

Owner, 2026-11-11: "the email sender account selector button isn't working btw, you haven't properly tested every button/feature".
Every control of the screen, clicked ONCE through the door the app registers, its visible outcome read through the control socket.
The table of controls is docs/COVERAGE.md "Control sweep: MCP notes"; the notes are docs/lanes/gap-sweep-a.md.

    sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/sweep-mcpnotes.py'
    python3 checks/parity/sweep-mcpnotes.py --selftest     a model of the screen: every case green, then one broken copy per case (no app, no socket)

Prints one line per case `name  pass|fail|unjudged  detail` (print_cases of journeys/lib/harness.py), then 'N checks, F failed, U not judged'.
Nothing here talks to a network or a real account: the server and the grants are the fixture core's. Expected values come from the screen's
own words (Sources/Features/McpNotesMac/McpWords.swift) and the fixture verbs of docs/CONTROL-SOCKET.md, never from a built screen.
"""
import os
import re
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable      # noqa: E402
from lib.harness import print_cases, declared_mutants   # noqa: E402

DRAWN_WAIT_S = 3.0      # how long a pill that a state change just brought on screen may take to be drawn

M = "mac-mcp-notes"
SCREEN = "sweep-mcpnotes"
APPS = ("Claude Desktop", "Cursor", "Raycast")           # the apps the core lists (FixtureSettings.mcpApps), as slugs below
CASES = []                                                # (name, door, words, fn)


def case(name, door, words):
    def deco(fn):
        CASES.append((name, door, words, fn))
        return fn
    return deco


def slug(app):
    return app.lower().replace(" ", "-")


def server(c):
    return c.rows(M + ".server")[0]


def app_row(c, app):
    return [r for r in c.rows(M + ".apps") if r["app"] == app][0]


def reset(c):
    """The screen as a person finds it: server on, no app allowed, no request waiting, the main window in front."""
    c.send("fixture", "reset")
    for a in APPS:
        if app_row(c, a)["state"] == "allowed":
            c.click(M + ".revoke." + slug(a))
    c.send("fixture", "mcp-server", "on")


# ---------------------------------------------------------------------------------------------------------------- the cases
@case("start-turns-the-server-on", M + ".server.start", "Start does nothing: the server stays off")
def _(c):
    reset(c)
    c.send("fixture", "mcp-server", "off")
    before = server(c)["state"]
    c.click(M + ".server.start")
    after = server(c)["state"]
    reset(c)
    return (before, after), ("off", "on")


def allow_case(app):
    @case("allow-%s-allows-the-app" % slug(app), "%s.allow.%s" % (M, slug(app)), "the Allow button of %s does nothing: the row stays 'not allowed'" % app)
    def _(c):
        reset(c)
        before = app_row(c, app)["state"]
        c.click("%s.allow.%s" % (M, slug(app)))
        after = app_row(c, app)["state"]
        reset(c)
        return (before, after), ("not allowed", "allowed")


def revoke_case(app):
    @case("revoke-%s-takes-the-access-back" % slug(app), "%s.revoke.%s" % (M, slug(app)), "the Revoke button of %s does nothing: the row stays 'allowed'" % app)
    def _(c):
        reset(c)
        c.click("%s.allow.%s" % (M, slug(app)))
        before = app_row(c, app)["state"]
        c.click("%s.revoke.%s" % (M, slug(app)))
        after = app_row(c, app)["state"]
        reset(c)
        return (before, after), ("allowed", "not allowed")


for _a in APPS:
    allow_case(_a)
for _a in APPS:
    revoke_case(_a)


def pending(c):
    """Ask as Claude Desktop (allowed first, the core refuses an app with no grant); the one waiting request."""
    reset(c)
    c.click(M + ".allow.claude-desktop")
    rid = c.send("fixture", "mcp-request", "Claude Desktop", "search_mail", "search", "your", "mail")["request_id"]
    return rid


@case("request-allow-answers-yes", M + ".request.<id>.allow", "the Allow button of a waiting request does nothing: the request keeps waiting")
def _(c):
    rid = pending(c)
    before = (len(c.rows(M + ".requests")), c.core("mcp_request/%s/state" % rid))
    c.click("%s.request.%s.allow" % (M, rid))
    after = (len(c.rows(M + ".requests")), c.core("mcp_request/%s/state" % rid))
    reset(c)
    return (before, after), ((1, "pending"), (0, "allowed"))


@case("request-refuse-answers-no", M + ".request.<id>.refuse", "the Refuse button of a waiting request does nothing: the request keeps waiting")
def _(c):
    rid = pending(c)
    before = (len(c.rows(M + ".requests")), c.core("mcp_request/%s/state" % rid))
    c.click("%s.request.%s.refuse" % (M, rid))
    after = (len(c.rows(M + ".requests")), c.core("mcp_request/%s/state" % rid))
    reset(c)
    return (before, after), ((1, "pending"), (0, "refused"))


@case("close-pill-puts-the-main-window-back", M + ".close", "the close pill does nothing: the MCP window stays the surface")
def _(c):
    reset(c)
    c.click(M + ".open")
    before = c.uistate().get("surface")
    c.click(M + ".close")
    after = c.uistate().get("surface")
    reset(c)
    return (before, after), (M, "mac-main")


@case("escape-closes-the-window", "key Escape", "Escape does nothing while the MCP window holds the keys")
def _(c):
    reset(c)
    c.click(M + ".open")
    before = c.uistate().get("surface")
    c.key("Escape")
    after = c.uistate().get("surface")
    reset(c)
    return (before, after), (M, "mac-main")


@case("palette-line-opens-the-window", "mac-palette.result.outside-ai-apps", "the palette line 'Outside AI apps (MCP)' opens nothing")
def _(c):
    reset(c)
    before = c.uistate().get("surface")
    c.click("mac-palette.result.outside-ai-apps")
    after = c.uistate().get("surface")
    c.click(M + ".close")
    reset(c)
    return (before, after), ("mac-main", M)


def drawn(c, ax):
    """True when `measure <ax>` answers a frame with a size; False when nothing on screen carries the id (the reply says so)."""
    # a pill that a state change just put on screen (Start after `mcp-server off`) is drawn on the next frames: read until it has a size, bounded
    end = time.monotonic() + DRAWN_WAIT_S
    while True:
        try:
            f = c.measure(ax).get("frame") or {}
            if f.get("w", 0) > 0 and f.get("h", 0) > 0:
                return True
        except Unreadable as e:
            if "no measurement" not in str(e):
                raise
        if time.monotonic() >= end:
            return False
        time.sleep(0.1)


@case("pills-are-drawn-with-their-own-id", "measure " + M + ".pills", "a pill (Start, Allow or Revoke) carries no id the socket can measure")
def _(c):
    reset(c)
    c.send("fixture", "mcp-server", "off")           # BEFORE the window opens: the session believes the core when the pane appears (McpSession.syncFromCore, onAppear); main92 sent it after, the session kept serverOn and drew no Start
    c.click(M + ".open")
    missing = [] if drawn(c, M + ".server.start") else [M + ".server.start"]
    c.send("fixture", "mcp-server", "on")
    for a in APPS:
        if not drawn(c, "%s.allow.%s" % (M, slug(a))):
            missing.append("%s.allow.%s" % (M, slug(a)))
    for a in APPS:
        c.click("%s.allow.%s" % (M, slug(a)))
        if not drawn(c, "%s.revoke.%s" % (M, slug(a))):
            missing.append("%s.revoke.%s" % (M, slug(a)))
    c.click(M + ".close")
    reset(c)
    return missing, []


# Controls with no door, no drawn control or no readable outcome: findings (file, line, a snippet that must be on that line).
# F3 (the pills carried accessibilityIdentifier only) is a driven case now: pills-are-drawn-with-their-own-id.
FINDINGS = [
    ("F1 standalone window: no request rows or Allow/Refuse", "Sources/Features/McpNotesMac/McpPaneView.swift", 5, "struct McpPaneView: View {",
     "the window (McpPaneView) draws the server card and the apps card only; a waiting request is drawn and answered in the Settings page (McpSettingsPane.swift:12), so in the window a waiting request has no control"),
    ("F2 Start has two doors, one control", "Sources/Features/McpNotesMac/McpNotesMacFeature.swift", 40, '"mac-mcp-notes.start": { _ in s.start() },',
     "`mac-mcp-notes.start` is a second door for the one Start button (`.server.start` is the drawn id); it is an alias, not a control, and is not clicked"),
]


# ---------------------------------------------------------------------------------------------------------------- the model (self-test only)
class Model:
    """A model of the screen as the app answers the socket; a broken copy `sw-<case>` makes that case's control do nothing."""
    def __init__(self, m=()):
        self.m = set(m)
        self.door_case = {}
        for name, door, _w, _f in CASES:
            self.door_case.setdefault(door, []).append(name)
        self.reset_state()

    def reset_state(self):
        self.on, self.grants, self.reqs, self.surface, self.seq = True, set(), {}, "mac-main", 0

    def dead(self, door):
        return any("sw-" + n in self.m for n in self.door_case.get(door, ()))

    def send(self, verb, *a):
        if verb == "click":
            return self.click(a[0])
        if verb == "fixture" and a[0] == "reset":
            self.reset_state()
        elif verb == "fixture" and a[0] == "mcp-server":
            self.on = a[1] == "on"
        elif verb == "fixture" and a[0] == "mcp-request":
            if a[1] not in self.grants:
                raise Unreadable("fixture mcp-request: %s has no grant" % a[1])
            self.seq += 1
            self.reqs["mcp-req-%d" % self.seq] = {"app": a[1], "state": "pending"}
            return {"ok": True, "request_id": "mcp-req-%d" % self.seq}
        return {"ok": True}

    def click(self, ax):
        k = ax[len(M) + 1:] if ax.startswith(M + ".") else ax
        if ax == "mac-palette.result.outside-ai-apps":
            if not self.dead(ax):
                self.surface = M
        elif k == "server.start":
            if not self.dead(ax):
                self.on = True
        elif k == "open":
            self.surface = M
        elif k == "close":
            if not self.dead(ax):
                self.surface = "mac-main"
        elif k.startswith("allow."):
            if not self.dead("%s.allow.%s" % (M, k[6:])):
                self.grants |= {a for a in APPS if slug(a) == k[6:]}
        elif k.startswith("revoke."):
            if not self.dead("%s.revoke.%s" % (M, k[7:])):
                self.grants -= {a for a in APPS if slug(a) == k[7:]}
        elif k.startswith("request."):
            rid, _, verb = k[8:].rpartition(".")
            if rid not in self.reqs:
                raise Unreadable("click %s: nothing registered" % ax)
            if not self.dead("%s.request.<id>.%s" % (M, verb)):
                self.reqs[rid]["state"] = "allowed" if verb == "allow" else "refused"
        else:
            raise Unreadable("click %s: nothing registered" % ax)
        return {"ok": True}

    def click_(self, ax):
        return self.click(ax)

    def key(self, chord):
        if chord == "Escape" and self.surface == M:
            if not self.dead("key Escape"):
                self.surface = "mac-main"
            return {"ok": True}
        raise Unreadable("key %s: no binding" % chord)

    def type(self, text):
        return {"ok": True}

    def uistate(self):
        return {"surface": self.surface}

    def measure(self, ax):
        k = ax[len(M) + 1:]
        seen = self.surface == M and ((k == "server.start" and not self.on) or (k.startswith("allow.") and True) or
                                      (k.startswith("revoke.") and any(slug(a) == k[7:] for a in self.grants)))
        if k.startswith("allow.") and any(slug(a) == k[6:] for a in self.grants):
            seen = False
        if not seen or self.dead("measure " + M + ".pills"):
            raise Unreadable("measure ('%s',): no measurement at %s: nothing on screen carries that id" % (ax, ax))
        return {"ok": True, "frame": {"x": 1, "y": 1, "w": 40, "h": 24}}

    def core(self, path):
        rid = path.split("/")[1]
        return self.reqs[rid]["state"]

    def rows(self, ax):
        if ax == M + ".server":
            return [{"state": "on" if self.on else "off"}]
        if ax == M + ".apps":
            return [{"app": a, "state": "allowed" if a in self.grants else "not allowed"} for a in APPS]
        if ax == M + ".requests":
            return [{"id": i} for i, r in self.reqs.items() if r["state"] == "pending"]
        raise Unreadable("no rows at %s" % ax)


# ---------------------------------------------------------------------------------------------------------------- runner and self-test
class Shim:
    """The Ctl calls the cases use, over the model (the real Ctl is used on the app)."""
    def __init__(self, model):
        self.m = model

    def send(self, verb, *a):
        return self.m.send(verb, *a)

    def click(self, ax):
        return self.m.click(ax)

    def key(self, chord):
        return self.m.key(chord)

    def uistate(self):
        return self.m.uistate()

    def rows(self, ax):
        return self.m.rows(ax)

    def core(self, path):
        return self.m.core(path)

    def measure(self, ax):
        return self.m.measure(ax)


def run(c):
    out = []
    for name, _door, _w, fn in CASES:
        label = "%s/%s" % (SCREEN, name)
        try:
            got, want = fn(c)
            out.append((label, "pass" if got == want else "fail", "expected %r, got %r" % (want, got)))
        except Unreadable as e:
            out.append((label, "unjudged", str(e)))
        except (KeyError, IndexError, TypeError) as e:
            out.append((label, "fail", "reply shape: %s %s" % (type(e).__name__, e)))
    return out


# A case whose set-up clicks another case's control is red on that control's broken copy too (named here, never silently): revoke needs allow first.
DEPENDENT = {"allow-%s-allows-the-app" % slug(a): ["revoke-%s-takes-the-access-back" % slug(a), "pills-are-drawn-with-their-own-id"] for a in APPS}


# MUTANTS: per case, the one-line change in the app that turns it red: `label | file | find (occurs once in the file) | replace`.
# sweep-mcpnotes/start-turns-the-server-on | Sources/Features/McpNotesMac/McpSession.swift | _ = AppModel.shared.perform(.setMcpServer(on: true)) | _ = 0
# sweep-mcpnotes/allow-claude-desktop-allows-the-app | Sources/Features/McpNotesMac/McpNotesMacFeature.swift | out["mac-mcp-notes.allow.\(slug)"] = { _ in s.set(name, allowed: true) } | out["mac-mcp-notes.allow.\(slug)"] = { _ in s.set(name, allowed: name != "Claude Desktop") }
# sweep-mcpnotes/allow-cursor-allows-the-app | Sources/Features/McpNotesMac/McpNotesMacFeature.swift | out["mac-mcp-notes.allow.\(slug)"] = { _ in s.set(name, allowed: true) } | out["mac-mcp-notes.allow.\(slug)"] = { _ in s.set(name, allowed: name != "Cursor") }
# sweep-mcpnotes/allow-raycast-allows-the-app | Sources/Features/McpNotesMac/McpNotesMacFeature.swift | out["mac-mcp-notes.allow.\(slug)"] = { _ in s.set(name, allowed: true) } | out["mac-mcp-notes.allow.\(slug)"] = { _ in s.set(name, allowed: name != "Raycast") }
# sweep-mcpnotes/revoke-claude-desktop-takes-the-access-back | Sources/Features/McpNotesMac/McpNotesMacFeature.swift | out["mac-mcp-notes.revoke.\(slug)"] = { _ in s.set(name, allowed: false) } | out["mac-mcp-notes.revoke.\(slug)"] = { _ in s.set(name, allowed: name == "Claude Desktop") }
# sweep-mcpnotes/revoke-cursor-takes-the-access-back | Sources/Features/McpNotesMac/McpNotesMacFeature.swift | out["mac-mcp-notes.revoke.\(slug)"] = { _ in s.set(name, allowed: false) } | out["mac-mcp-notes.revoke.\(slug)"] = { _ in s.set(name, allowed: name == "Cursor") }
# sweep-mcpnotes/revoke-raycast-takes-the-access-back | Sources/Features/McpNotesMac/McpNotesMacFeature.swift | out["mac-mcp-notes.revoke.\(slug)"] = { _ in s.set(name, allowed: false) } | out["mac-mcp-notes.revoke.\(slug)"] = { _ in s.set(name, allowed: name == "Raycast") }
# sweep-mcpnotes/request-allow-answers-yes | Sources/Features/McpNotesMac/McpNotesMacFeature.swift | out["mac-mcp-notes.request.\(r.id).allow"] = { _ in s.answer(r.id, allowed: true) } | out["mac-mcp-notes.request.\(r.id).allow"] = { _ in false }
# sweep-mcpnotes/request-refuse-answers-no | Sources/Features/McpNotesMac/McpNotesMacFeature.swift | out["mac-mcp-notes.request.\(r.id).refuse"] = { _ in s.answer(r.id, allowed: false) } | out["mac-mcp-notes.request.\(r.id).refuse"] = { _ in false }
# sweep-mcpnotes/close-pill-puts-the-main-window-back | Sources/Features/McpNotesMac/McpLayout.swift | w.close() | return false
# sweep-mcpnotes/escape-closes-the-window | Sources/Features/McpNotesMac/McpNotesMacFeature.swift | ["Escape": { m in m.surface == McpLayout.slot.id && McpWindowDoor.close(m) }] | ["Escape": { m in false }]
# sweep-mcpnotes/pills-are-drawn-with-their-own-id | Sources/Features/McpNotesMac/McpPaneView.swift | .whooshAx(id)   // the pill's id plus a frame `measure` reads | .accessibilityIdentifier(id)
# sweep-mcpnotes/palette-line-opens-the-window | Sources/Features/McpNotesMac/McpNotesMacFeature.swift | out["mac-palette.result.outside-ai-apps"] = { m in McpWindowDoor.open(m) } | out["mac-palette.result.outside-ai-apps"] = { m in false }


def mutant_lines():
    rows = []
    for line in open(os.path.abspath(__file__), encoding="utf-8"):
        m = re.match(r"# (sweep-mcpnotes/[\w-]+) \| (\S+) \| (.+?) \| (.+)$", line.rstrip("\n"))
        if m:
            rows.append(m.groups())
    return rows


def selftest():
    bad = []
    names = [n for n, *_ in CASES]
    clean = run(Shim(Model()))
    print("clean model:")
    for label, st, d in clean:
        print("model %-28s %-8s %s" % (label.split("/", 1)[1][:28], st, d[:110]))
        bad += [] if st == "pass" else ["clean model: %s is %s" % (label, st)]
    seen = 0
    for n in names:
        res = run(Shim(Model({"sw-" + n})))
        red = sorted([l.split("/", 1)[1] for l, st, _ in res if st == "fail"], key=lambda x: x != n)
        ok = red[:1] == [n] and set(red) <= {n} | set(DEPENDENT.get(n, ()))
        seen += ok
        print("broken %-48s red: %s" % (n, ", ".join(red) or "NONE"))
        bad += [] if ok else ["broken copy of %s: red cases %s" % (n, red)]
    declared = {l for l in declared_mutants(os.path.abspath(__file__))}
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
    print("%d cases, %d seen red on their broken model, %d MUTANTS rows checked against the files, %d findings checked at their line" % (len(names), seen, len(lines), len(FINDINGS)))
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
            res = run(c)
        finally:
            c.close()
    print_cases({"cases": res})
    f, u = sum(r[1] == "fail" for r in res), sum(r[1] == "unjudged" for r in res)
    print("%d checks, %d failed, %d not judged" % (len(res), f, u))
    return 1 if f or u else 0


if __name__ == "__main__":
    sys.exit(main())
