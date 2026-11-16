#!/usr/bin/env python3
"""mcpnotesmac-check: J5.7, outside AI apps over MCP: an ungranted app gets nothing, a granted one asks first.

    python3 checks/mcpnotesmac-check.py              drive the running rig copy; exit 1 when F>0 or U>0
    python3 checks/mcpnotesmac-check.py --selftest   each check against a stand-in app, clean and with its mutant

Shares the harness in checks/settingsaimac-check.py. Notes and Reminders are J8 and not covered here.

Round 2 (main54): the pane moved into the Settings window and the app list, the approval prompt and the results moved with the
core. The ids and shapes read here are the served ones: `mac-mcp-notes.requests` (one row per waiting approval, with its own allow
and refuse ids) and the core path `mcp_request/<id>/results`. The stand-in below (McpFake) serves the same shapes, so each check
still goes red on its mutant.
"""
import importlib.util
import os
import sys

_p = os.path.join(os.path.dirname(os.path.abspath(__file__)), "settingsaimac-check.py")
_s = importlib.util.spec_from_file_location("j5harness", _p)
H = importlib.util.module_from_spec(_s)
sys.modules["j5harness"] = H
_s.loader.exec_module(H)
A, FX, MSG, eq, act, soft, rows, core = H.A, H.FX, H.MSG, H.eq, H.act, H.soft, H.rows, H.core

S = H.Suite("mcpnotesmac")
APP, QUERY = FX["mcp"]["app"], FX["mcp"]["query"]
QUOTE_SUBJECT = MSG["msg-01"]["subject"]
REQUESTS = "mac-mcp-notes.requests"
KNOWN = ("Claude Desktop", "Cursor", "Raycast")  # the apps the core lists, in its order (FixtureSettings.mcpApps)


class McpFake(H.FakeApp):
    """The MCP half of the app as the served pane and core speak it: three listed apps, one request row per waiting approval."""

    def reset(self):
        super().reset()
        self.reqs, self.results_by_id, self.seq = [], {}, 0

    def v_fixture(self, name, *a):
        if name != "mcp-search":
            return super().v_fixture(name, *a)
        app, query = a[0], (a[1] if len(a) > 1 else "")
        granted = app in self.grants or "ungranted-gets-mail" in self.m
        if not self.mcp_on or not granted:
            return {"ok": True, "refused": "%s has no grant on the MCP server, so it gets nothing" % app}
        self.seq += 1
        rid = "mcp-req-%d" % self.seq
        hits = [m["subject"] for m in MSG.values() if query.lower() in m["subject"].lower()]
        if "no-prompt" in self.m:
            self.results_by_id[rid] = hits  # mail goes out with no owner answer
        else:
            self.reqs.append({"id": rid, "app": app, "tool": "search_mail", "hits": hits})
        return {"ok": True, "request_id": rid}

    def f_mcp_server(self, state):
        if "server-row-stale" not in self.m:  # the stale mutant: the pane never reads the core's server state again
            super().f_mcp_server(state)

    def v_click(self, ax):
        for r in list(self.reqs):
            if ax == "mac-mcp-notes.request.%s.allow" % r["id"]:
                self.results_by_id[r["id"]] = [] if "approve-returns-nothing" in self.m else r["hits"]
                self.reqs.remove(r)
                return {"ok": True}
            if ax == "mac-mcp-notes.request.%s.refuse" % r["id"]:
                self.reqs.remove(r)
                return {"ok": True}
        return super().v_click(ax)

    def v_core(self, kind, path):
        p = path.replace(".", "/").split("/")
        if len(p) == 3 and p[0] == "mcp_request" and p[2] == "results":
            return {"ok": True, "value": self.results_by_id.get(p[1], [])}
        return super().v_core(kind, path)

    def rows_(self, ax):
        if ax == "mac-mcp-notes.controls":
            return [{"ax": "mac-mcp-notes.close", "kind": "button"}]
        if ax == REQUESTS:
            return [{"id": r["id"], "app": r["app"], "tool": r["tool"], "allow": "mac-mcp-notes.request.%s.allow" % r["id"],
                     "refuse": "mac-mcp-notes.request.%s.refuse" % r["id"]} for r in self.reqs]
        if ax == A["mcp_apps"]:
            allowed = sorted(self.grants)
            return [{"app": a, "state": "allowed" if a in self.grants else "not allowed"} for a in allowed + [k for k in KNOWN if k not in allowed]]
        return super().rows_(ax)


    # doors-nav round 2: the Outside apps window as the shared title-bar case reads it. The window's panes begin at its top; the fixed app adds
    # max(0, band - 16) of room above the close pill, the mutant adds none (the pill then starts 16 pt down, under a 52 pt band).
    BAND, PILL_TOP = 52.0, 16.0

    def v_winbands(self):
        return {"ok": True, "bands": [{"id": "surface-AppWindow-2", "title": "mac-mcp-notes", "band": self.BAND, "fullsize": True, "lights": [{"x": 12, "y": 18, "w": 14, "h": 16}]}]}

    def v_measure(self, ax):
        room = 0.0 if "pill-under-the-band" in self.m else max(0.0, self.BAND - self.PILL_TOP)
        return {"ok": True, "frame": {"x": 1200, "y": self.PILL_TOP + room, "w": 32, "h": 32}}


H.FakeApp = McpFake  # FakeServer builds its stand-in by this name


def opened(ctl):
    """Start from the same place whatever ran before: `fixture reset` leaves the core's MCP state (server, grants, waiting
    approvals) as it was, so the server is put on and the app's grant is revoked here, and the checks count requests relative to
    what already waits."""
    soft(ctl, "fixture", "mcp-server", "on")
    soft(ctl, "click", A["mcp_revoke_claude"])
    act(ctl, "click", A["palette_mcp"])


def waiting(ctl):
    return [(r["app"], r["tool"]) for r in rows(ctl, REQUESTS)]


def mine(ctl, rid):
    return [r for r in rows(ctl, REQUESTS) if r.get("id") == rid]


@S.check("J5.7/arrives-by-palette", "no-mcp-entry", "the palette has no Outside AI apps entry")
def c_arrive(ctl, env):
    soft(ctl, "click", A["palette_mcp"])
    return eq(ctl.uistate().get("surface"), "mac-mcp-notes")


@S.check("J5.7/grant-listed-allowed", "grant-not-listed", "the grant is not listed")
def c_listed(ctl, env):
    opened(ctl)
    act(ctl, "click", A["mcp_allow_claude"])
    apps = {r["app"]: r["state"] for r in rows(ctl, A["mcp_apps"])}
    return apps == {APP: "allowed", "Cursor": "not allowed", "Raycast": "not allowed"}, "apps %r" % apps


@S.check("J5.7/grant-in-core", "grant-not-saved", "the grant is not saved")
def c_core(ctl, env):
    opened(ctl)
    act(ctl, "click", A["mcp_allow_claude"])
    return eq(core(ctl, "mcp_grant/" + APP), True)


@S.check("J5.7/ungranted-app-gets-nothing", "ungranted-gets-mail", "an app nobody allowed gets mail")
def c_ungranted(ctl, env):
    opened(ctl)
    act(ctl, "click", A["mcp_allow_claude"])
    base = len(waiting(ctl))
    first = act(ctl, "fixture", "mcp-search", APP, QUERY)  # control: the allowed app does reach the prompt
    control = len(waiting(ctl)) - base
    second = act(ctl, "fixture", "mcp-search", "Cursor", QUERY)
    after, results = len(waiting(ctl)) - base, core(ctl, "mcp_request/%s/results" % second.get("request_id", "none"))
    ok = bool(first.get("request_id")) and control == 1 and "refused" in second and not second.get("request_id") and after == 1 and results == []
    return ok, "allowed app %r, new waiting %r; Cursor reply %r, new waiting %r, results %r" % (first, control, second, after, results)


@S.check("J5.7/approval-prompt-before-results", "no-prompt", "a search returns mail with no prompt")
def c_prompt(ctl, env):
    opened(ctl)
    act(ctl, "click", A["mcp_allow_claude"])
    rid = act(ctl, "fixture", "mcp-search", APP, QUERY).get("request_id")
    before = ([(r["app"], r["tool"]) for r in mine(ctl, rid)], core(ctl, "mcp_request/%s/results" % rid))
    asked = mine(ctl, rid)
    if asked:
        act(ctl, "click", asked[0]["allow"])
    after = ([(r["app"], r["tool"]) for r in mine(ctl, rid)], core(ctl, "mcp_request/%s/results" % rid))
    ok = before == ([(APP, "search_mail")], []) and after == ([], [QUOTE_SUBJECT])
    return ok, "before approve %r; after approve %r" % (before, after)


@S.check("J5.7/server-off-reaches-the-pane", "server-row-stale", "the pane keeps showing on after the core's server went off")
def c_server_pane(ctl, env):
    opened(ctl)
    act(ctl, "fixture", "mcp-server", "off")
    r = rows(ctl, A["mcp_server"])[0]
    return eq(r["state"], "off")


@S.check("J5.7/server-off-says-so-and-starts", "no-start-control", "the off state has no Start control")
def c_server(ctl, env):
    opened(ctl)
    on = rows(ctl, A["mcp_server"])[0]["state"]  # control: the probe sees "on" when it is on
    act(ctl, "fixture", "mcp-server", "off")
    r = rows(ctl, A["mcp_server"])[0]
    if r.get("start") == A["mcp_start"]:
        act(ctl, "click", r["start"])
    back = rows(ctl, A["mcp_server"])[0]["state"] if r.get("start") else None
    ok = on == "on" and r["state"] == "off" and "off" in r["line"].lower() and back == "on"
    return ok, "initial %r, off row %r, after Start %r" % (on, r, back)


@S.check("J5.7/revoke-removes-access", "revoke-keeps-access", "a revoked app still gets mail")
def c_revoke(ctl, env):
    opened(ctl)
    act(ctl, "click", A["mcp_allow_claude"])
    control = act(ctl, "fixture", "mcp-search", APP, QUERY)
    waiting_before = len(waiting(ctl))
    act(ctl, "click", A["mcp_revoke_claude"])
    again = act(ctl, "fixture", "mcp-search", APP, QUERY)
    g = core(ctl, "mcp_grant/" + APP)
    after = len(waiting(ctl))
    ok = bool(control.get("request_id")) and g is False and "refused" in again and not again.get("request_id") and after == waiting_before
    return ok, "control %r, grant %r, search after revoke %r, waiting %r -> %r" % (control, g, again, waiting_before, after)


@S.check("J5.7/outside-apps-close-pill-clears-the-title-bar", "pill-under-the-band", "the close pill keeps its 16 pt top offset under a title bar band taller than 16 pt")
def c_band(ctl, env):
    """The close pill is the one control the window draws near its top. The window's real band comes from `winbands` (no typed number); the
    shared case (checks/checklib/titlebar.py, probes-r1) judges it at the default and the minimum size."""
    sys.path.insert(0, os.path.join(H.ROOT, "checks", "checklib"))     # main76: "checks/lib" holds no module since the rename to checklib
    import titlebar as TB
    opened(ctl)
    seen = []
    for door in ("mac-mcp-notes.window.default", "mac-mcp-notes.window.min"):
        env.setdefault("mcp_doors", {})[door] = H.soft_why(ctl, "click", door)      # the click itself (as `soft` made it), and what the size door answered: the evidence of a pill that is not found
        wins = [w for w in ctl.send("winbands").get("bands", []) if TB.surface_of(w) == "mac-mcp-notes"]
        if not wins:
            raise H.Unreadable("winbands lists no Outside apps window")
        bad, unj = TB.offenders(wins, lambda surface: ctl.rows(surface + ".controls"), lambda ax: ctl.measure(ax).get("frame"))
        seen.append((door.rsplit(".", 1)[1], [(a, p) for _, a, p in bad], [w for w, _ in unj]))
    return all(not b and not u for _, b, u in seen), "under the band (size, controls, not judged): %r" % seen


def _ev_band(env):
    """What decides the close pill case: which windows exist and whether the Outside apps window is one of them (winframes, winbands), the controls the pane
    lists (is `mac-mcp-notes.close` among them), what the id measures as, and what the two size doors answered."""
    return [("doors_answered", lambda c: env.get("mcp_doors")), ("close_pill", H.ev_measure("mac-mcp-notes.close")),
            ("controls_listed", lambda c: [r.get("ax") for r in c.rows("mac-mcp-notes.controls")][:8]),
            ("winbands", lambda c: [(b.get("id"), b.get("title"), b.get("band"), b.get("fullsize")) for b in c.send("winbands").get("bands", [])]),
            ("windows", H.ev_windows), ("surface", lambda c: c.uistate().get("surface")), ("apps_rows", lambda c: len(c.rows(A["mcp_apps"]))),
            ("marked_views_in_the_outside_apps_window", _ev_marked)]


def _ev_marked(c):
    """The views the Outside apps window marks (`labels window:<id>`, each window `winbands` lists as the pane): the ids on screen there, so a pill that is
    not found can be told from a pane that is not drawn (main79: the id was found nowhere, 15 accessibility elements walked, and nothing said what was drawn)."""
    out = {}
    for b in c.send("winbands").get("bands", []):
        if "mcp" in str(b.get("title", "")).lower() or "mcp" in str(b.get("id", "")).lower():
            r = c.send("labels", "window:%s" % b.get("id"))
            out[str(b.get("id"))] = {"marked": r.get("marked"), "capped": r.get("capped"), "ids": [x.get("ax") for x in r.get("labels", [])][:24]}
    return out or "no window of winbands names the Outside apps pane"


H.EVIDENCE["J5.7/outside-apps-close-pill-clears-the-title-bar"] = _ev_band


if __name__ == "__main__":
    sys.exit(H.main(S))
