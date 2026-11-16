#!/usr/bin/env python3
"""Reset guard and served-list row picking (tests-r58 prune, plan section 6).

A case group starts with `reset(ctl)`: `fixture reset`, then `fixture digest` must equal the digest read at launch. When it does not, the group is NOT
JUDGED naming the parts that differ (an Unreadable the harness records as not judged): a case that runs on a state the reset did not restore reads red on
the state, not on the feature (main63: 37 of 47 reds were checks, not the app). `pick(rows, fx_message)` finds a row of the SERVED list by what it says
(its subject and sender), never by a literal id that an earlier archive, delete or junk step may have taken away.
The digest helper is probes-r1's (checks/probes-check.py `digest`); the same three lines live here so a check can import them without that file.

    import resetguard as RG                  (sys.path gets checks/checklib)
    RG.capture_launch(ctl)                   the launch digest (optional: the first RG.reset otherwise resets, then captures the seed it finds)
    RG.reset(ctl)                            instead of ctl.send("fixture", "reset")
    python3 checks/checklib/resetguard.py --selftest
"""
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
if os.path.join(ROOT, "journeys") not in sys.path:
    sys.path.insert(0, os.path.join(ROOT, "journeys"))

LAUNCH = {}


def digest(ctl):
    r = ctl.send("fixture", "digest")
    return r.get("digest"), r.get("parts", {}) or {}, r.get("view")


def capture_launch(ctl):
    from lib.ctl import Unreadable
    d, parts, v = digest(ctl)
    if not d:
        raise Unreadable("fixture digest answers no value: the launch state cannot be compared")
    LAUNCH.update(d=d, parts=parts, v=v)


def differing(a, b):
    return sorted(k for k in set(a) | set(b) if a.get(k) != b.get(k))


def reset(ctl, compare=True):
    """`fixture reset`, then the digest must equal the launch digest; Unreadable (not judged) when it does not."""
    from lib.ctl import Unreadable
    first = compare and not LAUNCH
    if first:
        # main76: the digest was read HERE, before the first reset, so a group that ran earlier (a label dragged, a folder made: parts labelRows or
        # folderRows, nextId, undo) became "the launch state", and every later reset, which does restore the seed, read as not restoring it. The
        # state this call finds is whatever the earlier cases left; the state the reset brings back is the seed, so the seed is captured AFTER the reset.
        # That the reset equals the true launch is probes-check's case `fixture-reset-restores-the-launch-state`; this guard proves each LATER reset
        # reproduces that same state.
        ctl.send("fixture", "reset")
        capture_launch(ctl)
        return
    ctl.send("fixture", "reset")
    if not compare:
        return
    d, parts, v = digest(ctl)
    if not d:
        raise Unreadable("fixture digest answers no value after the reset")
    if d != LAUNCH["d"] or v != LAUNCH["v"]:
        raise Unreadable("fixture reset did not restore the launch state (parts that differ: %s; view %s): the case is not judged%s"
                         % (", ".join(differing(LAUNCH["parts"], parts)) or "none", "same" if v == LAUNCH["v"] else "differs", view_evidence(v)))


def view_evidence(now):
    """When the view differs: both view strings, the one read at launch and the one read after this reset (cut at 400 characters each), so the
    fields that differ are read from the message itself, not guessed (main77: `view differs` named no field)."""
    if now == LAUNCH.get("v"):
        return ""
    cut = lambda x: (lambda s: s if len(s) <= 400 else s[:400] + "...")(repr(x))
    return "; launch view %s; view after the reset %s" % (cut(LAUNCH.get("v")), cut(now))


def step_reset(step):
    """The same, for a harness Step: a refusal or a mismatch makes the checks of the step that read the list not judged (the note names it)."""
    from lib.ctl import Unreadable
    if isinstance(step.j.ctl, _Refusing):      # a refusal lasts to the next reset only: the next group starts from the real socket
        step.j.ctl = step.j.ctl.real
    if step.j.ctl is None:
        return
    try:
        reset(step.j.ctl)
    except Unreadable as e:
        step.j.notes.append("%s reset: %s" % (step.id, e))
        step.j.ctl = _Refusing(str(e), step.j.ctl)


class _Refusing:
    """Stands in for the socket after a failed reset: every read raises, so each check of the group is NOT JUDGED with the reason."""
    def __init__(self, why, real):
        self.why, self.real = why, real

    def __getattr__(self, name):
        from lib.ctl import Unreadable
        def refuse(*a, **k):
            raise Unreadable(self.why)
        return refuse


def pick(rows, fx_message):
    """The served row that says what the fixture message says (subject and sender name); None when the list does not hold it."""
    for r in rows:
        if r.get("subject") == fx_message.get("subject") and r.get("sender") in (fx_message.get("sender"), None):
            return r
    return None


def row_index(rows, fx_message):
    r = pick(rows, fx_message)
    return None if r is None else rows.index(r)


# ---------------------------------------------------------------- selftest
class _Ctl:
    def __init__(self, restore=True, digest_ok=True):
        self.state, self.restore, self.digest_ok, self.sent = {"rows": "A", "star": "0"}, restore, digest_ok, []
        self.view, self.reset_view = "surface=mac-main;typed=", None      # reset_view: what the view reads after a reset (None: the launch view)

    def send(self, *a):
        self.sent.append(a)
        if a == ("fixture", "reset"):
            self.state["rows"] = "A"
            if self.reset_view is not None:
                self.view = self.reset_view
            if self.restore:
                self.state["star"] = "0"
            return {}
        if a == ("fixture", "digest"):
            return {"digest": "|".join("%s=%s" % kv for kv in sorted(self.state.items())) if self.digest_ok else None, "parts": dict(self.state), "view": self.view}
        return {}


def selftest(reset_fn=None, pick_fn=None):
    from lib.ctl import Unreadable
    reset_fn, pick_fn = reset_fn or reset, pick_fn or pick
    out = []
    def run(c):
        LAUNCH.clear(); capture_launch(c) if c.digest_ok else None
        c.state["star"] = "1"                       # a destructive step
        try:
            reset_fn(c); return "ok"
        except Unreadable as e:
            return "not judged: %s" % e
    out.append(("a reset that restores passes", run(_Ctl()), "ok"))
    r = run(_Ctl(restore=False))
    out.append(("a reset that leaves a part is NOT JUDGED naming it", r.startswith("not judged") and "star" in r, True))
    LAUNCH.clear()
    c = _Ctl(); c.state["star"] = "1"; c.state["rows"] = "B"          # earlier cases left the state dirty: no capture_launch was called
    try:
        reset_fn(c); c.state["star"] = "1"; reset_fn(c); got = "ok"       # the first reset must not take the dirty state for the launch state
    except Unreadable as e:
        got = "not judged: %s" % e
    out.append(("dirty state before the first reset is not taken for the launch state", got, "ok"))
    LAUNCH.clear()
    c = _Ctl(); c.state["star"] = "1"
    try:
        reset_fn(c)
        c.restore = False; c.state["star"] = "1"
        reset_fn(c); got = "ok"
    except Unreadable as e:
        got = "not judged"
    out.append(("after the first reset a later reset that leaves a part is NOT JUDGED", got, "not judged"))
    LAUNCH.clear()
    try:
        capture_launch(_Ctl(digest_ok=False)); got = "captured"
    except Unreadable:
        got = "not judged"
    out.append(("no digest value is NOT JUDGED", got, "not judged"))
    LAUNCH.clear()
    c = _Ctl(); capture_launch(c); c.reset_view = "surface=mac-search;typed=q"
    try:
        reset_fn(c); got = "ok"
    except Unreadable as e:
        got = "not judged: %s" % e
    out.append(("a view the reset leaves differing is NOT JUDGED and the message quotes both views",
                got.startswith("not judged") and "launch view 'surface=mac-main;typed='" in got and "view after the reset 'surface=mac-search;typed=q'" in got, True))
    served = [{"id": "x9", "subject": "Quote", "sender": "Ana"}, {"id": "x1", "subject": "Invoice", "sender": "Bo"}]
    out.append(("pick finds the row by what it says, wherever it sits", (pick_fn(served, {"subject": "Invoice", "sender": "Bo"}) or {}).get("id"), "x1"))
    out.append(("pick does not use the literal id", (pick_fn(served, {"id": "x9", "subject": "Invoice", "sender": "Bo"}) or {}).get("id"), "x1"))
    out.append(("pick says None for a row the list lacks (the list is not empty)", (pick_fn(served, {"subject": "Gone", "sender": "Cy"}), len(served) > 0), (None, True)))
    return out


def main():
    bad = 0
    for n, got, want in selftest():
        ok = got == want; bad += not ok
        print("%-58s %s%s" % (n, "ok" if ok else "FAIL", "" if ok else "  got %r want %r" % (got, want)))
    def capture_before_reset(ctl, compare=True):     # the main76 behaviour: the launch digest is read at the first reset, before it
        from lib.ctl import Unreadable
        if compare and not LAUNCH: capture_launch(ctl)
        ctl.send("fixture", "reset")
        d, parts, v = digest(ctl)
        if d != LAUNCH["d"]: raise Unreadable("fixture reset did not restore the launch state (parts that differ: %s)" % ", ".join(differing(LAUNCH["parts"], parts)))
    def no_compare(ctl, compare=True): ctl.send("fixture", "reset")
    def by_id(rows, m): return next((r for r in rows if r.get("id") == m.get("id")), None)
    for name, kw in (("reset-never-compares", {"reset_fn": no_compare}), ("capture-before-the-first-reset", {"reset_fn": capture_before_reset}), ("pick-by-literal-id", {"pick_fn": by_id})):
        red = [n for n, got, want in selftest(**kw) if got != want]
        print("mutant %-22s -> %s" % (name, "RED on " + "; ".join(red) if red else "NOT RED")); bad += not red
    print("%d problems" % bad)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main() if "--selftest" in sys.argv else 0)
