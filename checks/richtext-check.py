#!/usr/bin/env python3
"""richtext-check: the composer's rich text survives a round trip to the MIME body and back.

    python3 checks/richtext-check.py              drive the running rig copy; exit 1 when F > 0 or U > 0
    python3 checks/richtext-check.py --selftest   baseline green and one red per MUTANT, on the model in checks/composermac-check.py

Acceptance (docs/plan.json fn-richtext): bold, list, link and inline picture each round-trip to the MIME body and back;
Cmd-Z after an AI rewrite restores the previous paragraph text exactly. Each format is its own check, so one passing
format never hides a failing one. Every absence assertion carries a positive control in the same value.

The two doors the live run drives (lane richtext-r2): the verb `fixture format <bold|list|link|picture> <args...>`
(Sources/Features/RichText/RichTextFixture.swift, with `reload-draft` and `paste-html` beside it) and the state read
`core state draft/current/mime` (FixtureCore.state, from `RichTextFeature.session.mimeHTML`). The three `door-*` checks
judge the doors themselves, each with a mutant on the model below, so a door that is served but wrong goes red.
"""
import importlib.util
import os
import sys
import time

_p = os.path.join(os.path.dirname(os.path.abspath(__file__)), "composermac-check.py")
_s = importlib.util.spec_from_file_location("composermac_check", _p)
H = importlib.util.module_from_spec(_s)
sys.modules["composermac_check"] = H
_s.loader.exec_module(H)
T, BODY, compose = H.T, H.BODY, H.compose


class DoorModel(H.ModelApp):
    """The composer model plus the three ways a door can be served but wrong (each is one `door-*` mutant)."""

    def reset(self):
        super().reset()
        self.stale_mime = None

    def v_click(self, ax):
        if ax == "mac-composer.dev.fixture-writer":
            return                                                  # the rig's stand-in AI writer: the model always has one
        out = super().v_click(ax)
        if ax == "mac-composer.rewrite.shorter":
            super().v_key("Enter")                                  # the app applies the pick on the click itself (ComposerModel.rewrite); the base model waits for Enter
        return out

    def v_key(self, chord):
        if chord == "R" and self.has("reset-empties-the-draft"):
            kept = self.runs
            out = super().v_key(chord)
            self.runs = kept                                        # the base model blanks the body when the composer opens; the app does not, so the mutant keeps what the last check left
            return out
        return super().v_key(chord)

    def v_fixture(self, name, *a):
        if name == "reset" and self.has("reset-empties-the-draft"):
            kept = self.runs
            super().v_fixture(name, *a)
            self.runs = kept                                        # the reset leaves the words one check typed in the next check's composer
            return
        if name == "format" and self.has("door-format-replaces-the-draft"):
            return                                                  # accepted, applied to nothing
        if name == "format" and a and a[0] not in ("bold", "list", "link", "picture") and self.has("door-unknown-format-is-refused"):
            return                                                  # a format nobody serves is taken without a word
        return super().v_fixture(name, *a)

    def v_core(self, kind, path):
        out = super().v_core(kind, path)
        if path == "draft/current/mime" and self.has("door-mime-follows-the-draft"):
            if self.stale_mime is None:
                self.stale_mime = out["value"]
            return {"value": self.stale_mime}                       # the first read is served for ever after
        return out


H.ModelApp = DoorModel
RT = T["richtext"]
FMT = {f["id"]: f for f in RT["formats"]}

REG = H.Registry()
check = REG.check


def mime(c):
    return c.core("draft/current/mime")


def runs(c):
    return c.rows("mac-composer.runs")


def until(fn, ok, secs=4.0):
    """The rewrite runs on a task after the click: read until `ok(value)` or the time is up; the last value is returned."""
    end, v = time.time() + secs, fn()
    while not ok(v) and time.time() < end:
        time.sleep(0.05)
        v = fn()
    return v


def has_any(text, options):
    return any(o in text for o in options)


@check("round-trip-bold", "bold is lost when the MIME body is read back", (True, ["bold"], "firm price"))
def _(c):
    compose(c)
    c.send("fixture", "format", "bold", FMT["bold"]["plain"])
    ok = has_any(mime(c), FMT["bold"]["mime_any_of"])
    c.send("fixture", "reload-draft")
    r = runs(c)
    return (ok, [x["format"] for x in r], BODY(c))


@check("round-trip-list", "a two-item list comes back as one item", (True, True, ["unit price", "delivery week"]))
def _(c):
    compose(c)
    c.send("fixture", "format", "list", *FMT["list"]["items"])
    m = mime(c)
    ok = has_any(m, FMT["list"]["mime_any_of"]) and all(x in m for x in FMT["list"]["mime_all"])
    c.send("fixture", "reload-draft")
    r = runs(c)
    return (ok, all(x["format"] == "list-item" for x in r) and len(r) == 2, [x["text"] for x in r])


@check("round-trip-link", "the link target is lost when read back", (True, FMT["link"]["href"], FMT["link"]["plain"]))
def _(c):
    compose(c)
    c.send("fixture", "format", "link", FMT["link"]["plain"], FMT["link"]["href"])
    ok = all(x in mime(c) for x in FMT["link"]["mime_all"])
    c.send("fixture", "reload-draft")
    r = runs(c)[0]
    return (ok, r["href"], r["text"])


@check("round-trip-picture", "the inline picture is dropped when read back", (True, ["logo-1"]))
def _(c):
    compose(c)
    c.send("fixture", "format", "picture", FMT["picture"]["cid"])
    ok = has_any(mime(c), FMT["picture"]["mime_any_of"])
    c.send("fixture", "reload-draft")
    return (ok, [x["cid"] for x in runs(c)])


@check("special-characters-escape-and-return", "less-than and ampersand go into the MIME body unescaped", (True, RT["special"]["plain"]))
def _(c):
    compose(c, RT["special"]["plain"])
    ok = RT["special"]["mime_contains"] in mime(c)
    c.send("fixture", "reload-draft")
    return (ok, BODY(c))


@check("paste-match-style-strips-styles", "pasted text keeps the source font", ("pasted words", None))
def _(c):
    compose(c)
    c.send("fixture", "paste-html", RT["paste"]["html"])
    r = runs(c)
    return (BODY(c), r[0]["font"])


@check("hostile-html-is-neutralised", "pasted script and event handlers reach the MIME body", (True, True))
def _(c):
    compose(c)
    c.send("fixture", "paste-html", RT["hostile"]["html"])
    m = mime(c)
    clean = not any(x in m for x in RT["hostile"]["expect_absent"])
    kept = RT["hostile"]["expect_present"] in m          # positive control: the probe reads real markup, so the absence above means something
    return (clean, kept)


@check("rewrite-undo-keeps-formatting", "Cmd-Z after a rewrite restores the words but drops the bold", (True, True))
def _(c):
    compose(c)
    c.send("fixture", "format", "bold", RT["undo_source_bold"])
    c.type(" and more words.")
    before = [(x["text"], x["format"]) for x in runs(c)]
    words = "".join(t for t, _ in before)
    c.click("mac-composer.dev.fixture-writer")                # without a writer the rewrite says "no provider" and changes nothing, so there is nothing to undo
    c.click("mac-composer.rewrite")
    c.click("mac-composer.rewrite.shorter")
    changed = until(lambda: BODY(c), lambda b: b != words) != words   # positive control: the rewrite really replaced the paragraph
    c.key("⌘Z")
    return (changed, [(x["text"], x["format"]) for x in runs(c)] == before)


@check("long-text-roundtrip-keeps-all-chars", "the MIME body is cut at one thousand characters", True)
def _(c):
    compose(c, RT["long_text"])
    return mime(c).count("a") == len(RT["long_text"])


@check("empty-draft-mime-is-empty", "an empty draft serialises to an empty paragraph", ("", "x"))
def _(c):
    compose(c)
    empty = mime(c)
    c.type("x")
    return (empty, mime(c))        # control: the same path returns the typed text, so the empty string above is a real read


@check("reset-empties-the-draft", "fixture reset leaves the words the last check typed in the composer", ("", "left over"))
def _(c):
    compose(c, "left over")
    typed = BODY(c)
    compose(c)                                                # the reset every check opens with
    return (BODY(c), typed)                                   # control: the same path read "left over" a moment ago, so the empty string is a real read


@check("door-format-replaces-the-draft", "the format verb is accepted and changes nothing", (["bold"], ["list-item", "list-item"]))
def _(c):
    compose(c)
    c.send("fixture", "format", "bold", FMT["bold"]["plain"])
    first = [x["format"] for x in runs(c)]
    c.send("fixture", "format", "list", *FMT["list"]["items"])
    return (first, [x["format"] for x in runs(c)])        # the second call replaces the first: two items, no bold left


@check("door-mime-follows-the-draft", "the MIME read is stale: the first value is served after the draft changes", (True, True, True))
def _(c):
    compose(c)
    c.send("fixture", "format", "bold", FMT["bold"]["plain"])
    a = mime(c)
    c.send("fixture", "format", "list", *FMT["list"]["items"])
    b = mime(c)
    return (a != b, has_any(a, FMT["bold"]["mime_any_of"]), has_any(b, FMT["list"]["mime_any_of"]))


@check("door-unknown-format-is-refused", "a format nobody serves is accepted without a word", ("refused", "keep"))
def _(c):
    compose(c, "keep")
    try:
        c.send("fixture", "format", "sparkle", "x")
        said = "accepted"
    except H.Unreadable:
        said = "refused"
    return (said, BODY(c))                                  # refused, and the typed words are still there


MUTANTS = {n: m for n, m, _, _ in REG.items}
if __name__ == "__main__":
    sys.exit(H.main("richtext", [REG], MUTANTS))
