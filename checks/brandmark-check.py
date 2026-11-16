#!/usr/bin/env python3
"""Brand mark check: the Whoosh envelope is ONE shared glass view (BrandMark) on every surface that carries it.

Reads `rows whoosh.brand-mark` (fields ax, id, surface, size, material) on each surface. `material` is what the glass modifier
reported when it drew (a mark whose glass was removed reads "flat"); `size` is only what the call site asked for, so the RENDERED
size is read with `measure <row ax>` (the mark is a `.whooshAx` view: the socket answers from the drawn frame, never a constant).
Expected sizes come from the confirmed boards, not from a screen: add-account head 36 pt (44 once an account is added), welcome
card up to 160 pt, sidebar empty state 24 pt.
Routes, all served by the fixture core: the add-account sheet by `mac-add-account.open`; the Welcome screen by
`mac-settings-accounts.move-to-iphone` (the same control journeys/J4.9 uses; `dock.whoosh` opens mac-main only); the sidebar by `dock.whoosh`.
The sidebar's empty state is reached by `fixture accounts none` (the fixture core lists no account) and left by `fixture accounts all`.
Self-test: `python3 checks/brandmark-check.py --selftest` runs the journey against a stand-in socket (ModelCtl), once clean (every check green)
and once per MUTANT below (the check named must go red on that case).
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/brandmark-check.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
"""
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, Journey                         # noqa: E402
from lib.ctl import Ctl, Unreadable                                    # noqa: E402

ID = "brandmark"
TITLE = "Brand mark: one glass envelope"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "each surface that shows the mark shows BrandMark in a glass material"
MARK = "whoosh.brand-mark"
GLASS = ("glass-26", "glass-15")
WELCOME = "mac-welcome-accounts"
SEEN = {}   # every mark row any step read, by ax id (with the rendered height once measured)


def mark(s, surface, wait_s=3.0):
    """The mark rows a surface draws. A surface in its own window (Welcome) registers its mark on the view's first frame, which lands
    after the window opens: read until a row is there or the budget ends (main50 to main52: 0 rows read on the frame the window opened)."""
    import time
    end = time.time() + wait_s
    while True:
        rows = [r for r in s.ctl.rows(MARK) if r.get("surface") == surface]
        if rows or time.time() >= end or getattr(s.j.ctl, "instant", False):
            break
        time.sleep(0.1)
    for r in rows:
        SEEN.setdefault(r["ax"], dict(r))
    return rows


def drawn_size(s, row):
    """The mark's rendered height in points, from the socket's measurement of its own view."""
    h = s.ctl.measure(row["ax"])["frame"]["h"]
    SEEN.setdefault(row["ax"], dict(row))["_h"] = h
    return h


def sizes_in(s, surface, lo, hi):
    return [lo <= drawn_size(s, r) <= hi for r in mark(s, surface)]


def sheet_gone(s, wait_s=3.0):
    """Wait until no mark row is on screen. The closing sheet and Welcome's inline card share ONE AddAccountModel: when the card appears it flips
    `style` to inline, the sheet's head (still fading out) then registers as "welcome" too, and Welcome reads two marks (main53) and measures
    one that is gone. The wait is the check's own: it is the app's defect (AddAccount/**), reported in docs/lanes/brandmark-r2.md."""
    import time
    if s.j.ctl is None or getattr(s.j.ctl, "instant", False):
        return
    end = time.time() + wait_s
    while s.ctl.rows(MARK) and time.time() < end:
        time.sleep(0.1)


def drive(j):
    s = j.step("par-brandmark-add-account", "Add-account head")
    s.raw("fixture", "reset")
    s.raw("fixture", "accounts", "all")   # reset does not restore the account list (FixtureEvents is not this lane's file)
    s.arrive(("click", "mac-add-account.open"), "mac-add-account")
    s.expect("add-account-carries-one-mark", lambda: len(mark(s, "add-account")), 1)
    s.expect("add-account-material-is-glass", lambda: [r["material"] in GLASS for r in mark(s, "add-account")], [True])
    s.expect("add-account-size-near-36", lambda: sizes_in(s, "add-account", 32, 48), [True])

    s = j.step("par-brandmark-welcome", "Welcome card")
    s.click("mac-add-account.cancel")   # the sheet's own marks must not be counted on the Welcome screen
    sheet_gone(s)
    s.arrive(("click", "mac-settings-accounts.move-to-iphone"), WELCOME)
    s.expect("welcome-carries-one-mark", lambda: len(mark(s, "welcome")), 1)
    s.expect("welcome-material-is-glass", lambda: [r["material"] in GLASS for r in mark(s, "welcome")], [True])
    s.expect("welcome-size-at-most-160", lambda: sizes_in(s, "welcome", 24, 160), [True])

    s = j.step("par-brandmark-sidebar", "Sidebar empty state")
    s.raw("fixture", "accounts", "none")   # the fixture core lists no account: the sidebar shows its empty state
    s.arrive(("click", "dock.whoosh"), "mac-main")
    s.expect("sidebar-empty-size-24", lambda: sidebar_sizes(s), [24.0])
    s.expect("sidebar-empty-material-is-glass", lambda: [r["material"] in GLASS for r in mark(s, "sidebar-empty")], [True])
    s.expect("sidebar-empty-offers-add-account", lambda: offers_add_account(s), True)
    s.click("mac-add-account.cancel")      # the sheet closes; the sidebar is the main window's again
    s.raw("fixture", "accounts", "all")
    s.expect("sidebar-empty-gone-with-an-account", lambda: len(mark(s, "sidebar-empty", wait_s=0.5)), 0)

    s = j.step("par-brandmark-one-view", "One shared view")
    s.expect("every-mark-row-is-the-shared-view", lambda: seen(s) and {r["id"] for r in SEEN.values()} == {MARK}, True)
    s.expect("no-flat-material-anywhere", lambda: seen(s) and all(r["material"] in GLASS for r in SEEN.values()), True)
    s.expect("drawn-size-is-the-declared-size", lambda: seen(s) and all(abs(r["_h"] - r["size"]) <= 1 for r in SEEN.values() if "_h" in r)
             and any("_h" in r for r in SEEN.values()), True)


def seen(s):
    """True once at least one mark row was read on a live socket; a check over no marks proves nothing, so it is not judged."""
    s.ctl
    if not SEEN:
        raise Unreadable("not judged: no mark row was read on any surface")
    return True


def sidebar_sizes(s):
    rows = mark(s, "sidebar-empty")
    if not rows:
        return []   # a read that found no mark is a failed check (the empty state is served by `fixture accounts none`), not an unjudged one
    return [round(drawn_size(s, r), 1) for r in rows]


EMPTY_ADD = "mac-main.sidebar.empty.add"


def offers_add_account(s):
    """The empty sidebar offers one button row, and its click opens the add-account sheet."""
    rows = [r for r in s.ctl.rows("mac-main.sidebar.empty") if r.get("ax") == EMPTY_ADD]
    if len(rows) != 1:
        return False
    s.ctl.click(EMPTY_ADD)
    return s.ctl.uistate().get("surface") == "mac-add-account"


# MUTANTS: per check, the mutation of the app that must turn it red.
# par-brandmark-add-account/arrives-by-control | the add-account control opens another surface
# par-brandmark-add-account/add-account-carries-one-mark | the head draws the flat envelope chip again
# par-brandmark-add-account/add-account-material-is-glass | BrandMark drops the .whooshGlass(in:report:) call: no glass is drawn, nothing reports, the row reads flat
# par-brandmark-add-account/add-account-size-near-36 | the head mark is drawn 20 pt
# par-brandmark-welcome/arrives-by-control | the Welcome route (the Settings Accounts move-to-iphone control) opens another surface
# par-brandmark-welcome/welcome-carries-one-mark | the welcome card keeps the gradient tile with the SF envelope
# par-brandmark-welcome/welcome-material-is-glass | the welcome mark loses its glass modifier (reads flat)
# par-brandmark-welcome/welcome-size-at-most-160 | the welcome mark is drawn 300 pt
# par-brandmark-sidebar/arrives-by-control | the Dock launch opens another surface
# par-brandmark-sidebar/sidebar-empty-size-24 | the sidebar empty state mark is drawn 40 pt
# par-brandmark-sidebar/sidebar-empty-material-is-glass | the sidebar empty mark loses its glass modifier (reads flat)
# par-brandmark-sidebar/sidebar-empty-offers-add-account | the empty state draws no Add account button, or its click opens another surface
# par-brandmark-sidebar/sidebar-empty-gone-with-an-account | the sidebar keeps the empty state (and its mark) once the core lists an account again
# par-brandmark-one-view/every-mark-row-is-the-shared-view | a surface registers its own drawn envelope under another id
# par-brandmark-one-view/no-flat-material-anywhere | the flat envelope is restored on one surface
# par-brandmark-one-view/drawn-size-is-the-declared-size | a call site passes size: 36 but frames the mark at 20 pt


class ModelCtl(Ctl):
    """A stand-in for the control socket: the surfaces and mark rows the real app serves, with one MUTANT label applied at a time."""
    instant = True

    def __init__(self, mutants=()):
        super().__init__("")
        self.mut, self.surface, self.none, self.h = set(mutants), "mac-main", False, {}

    def close(self):
        pass

    def _route(self, label, target):
        return "mac-other" if label in self.mut else target

    def _marks(self):
        m, out = self.mut, []
        add, wel, sb = "par-brandmark-add-account/", "par-brandmark-welcome/", "par-brandmark-sidebar/"
        if self.surface == "mac-add-account" and add + "add-account-carries-one-mark" not in m:
            out.append(("add-account", 20 if add + "add-account-size-near-36" in m else 36,
                        "flat" if add + "add-account-material-is-glass" in m else "glass-26", 36))
        if self.surface == "mac-welcome-accounts" and wel + "welcome-carries-one-mark" not in m:
            out.append(("welcome", 300 if wel + "welcome-size-at-most-160" in m else 160,
                        "flat" if wel + "welcome-material-is-glass" in m or "par-brandmark-one-view/no-flat-material-anywhere" in m else "glass-26", 160))
        if self.surface == "mac-main" and (self.none or sb + "sidebar-empty-gone-with-an-account" in m):
            out.append(("sidebar-empty", 40 if sb + "sidebar-empty-size-24" in m else 24,
                        "flat" if sb + "sidebar-empty-material-is-glass" in m else "glass-26", 24))
        rows = []
        for i, (surf, h, mat, declared) in enumerate(out):
            ax = "whoosh.brand-mark.%s.%d" % (surf, i)
            other = "par-brandmark-one-view/every-mark-row-is-the-shared-view" in m and surf == "add-account"
            if "par-brandmark-one-view/drawn-size-is-the-declared-size" in m and surf == "add-account":
                h = 20
            self.h[ax] = h
            rows.append({"ax": ax, "id": "whoosh.drawn-envelope" if other else MARK, "surface": surf, "size": declared, "material": mat})
        return rows

    def send(self, verb, *args):
        sb = "par-brandmark-sidebar/"
        if verb == "click":
            routes = {"mac-add-account.open": ("par-brandmark-add-account/arrives-by-control", "mac-add-account"),
                      "mac-settings-accounts.move-to-iphone": ("par-brandmark-welcome/arrives-by-control", "mac-welcome-accounts"),
                      "dock.whoosh": (sb + "arrives-by-control", "mac-main"), "mac-add-account.cancel": ("", "mac-main"),
                      EMPTY_ADD: (sb + "sidebar-empty-offers-add-account", "mac-add-account")}
            if args[0] not in routes or (args[0] == EMPTY_ADD and not self.none):
                raise Unreadable("click %s: no such control" % args[0])
            self.surface = self._route(*routes[args[0]])
            return {"ok": True}
        if verb == "fixture":
            if args[0] == "reset":
                self.surface = "mac-main"
            elif args[:1] == ("accounts",):
                self.none = args[1] == "none"
            return {"ok": True}
        if verb == "uistate":
            return {"ok": True, "state": {"surface": self.surface}}
        if verb == "rows":
            if args[0] == MARK:
                return {"ok": True, "rows": self._marks()}
            if args[0] == "mac-main.sidebar.empty":
                gone = sb + "sidebar-empty-offers-add-account" in self.mut
                return {"ok": True, "rows": [{"ax": EMPTY_ADD}] if self.none and not gone else []}
            return {"ok": True, "rows": []}
        if verb == "measure":
            self._marks()
            return {"ok": True, "frame": {"x": 0, "y": 0, "w": 1, "h": self.h[args[0]]}}
        raise Unreadable("%s: not modelled" % verb)


def model_run(mutants):
    """The journey on the stand-in: the labels that failed and the labels not judged."""
    SEEN.clear()
    j = Journey(ID, ModelCtl(mutants))
    drive(j)
    cs = j.checks()
    return ({c.label for c in cs if c.state == "fail"}, {c.label for c in cs if c.state == "unjudged"}, [c for c in cs if c.state == "fail"])


def selftest(path):
    labels = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# par-") and " | " in l]
    base_f, base_u, base_fail = model_run(())
    rows, missed = [], []
    for label, what in labels:
        f, _, _ = model_run((label,))
        red = label in f
        rows.append((label, what, red))
        if not red:
            missed.append(label)
    total = len(labels)
    print("clean model: failed=%s unjudged=%s" % (sorted(base_f), sorted(base_u)))
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant | seen red |")
    print("|---|---|---|")
    for label, what, red in rows:
        print("| %s | %s | %s |" % (label, what, "RED on this case" if red else "NOT RED"))
    bad = len(base_f) + len(base_u) + len(missed)
    print("%d mutants, %d not seen red, %d clean failures, %d clean not judged" % (total, len(missed), len(base_f), len(base_u)))
    return 0 if bad == 0 else 1


def main():
    path = os.path.abspath(__file__)
    if "--selftest" in sys.argv:
        return selftest(path)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    rows = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# par-") and " | " in l]
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant |")
    print("|---|---|")
    for k, v in rows:
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
    sys.exit(main())
