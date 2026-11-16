#!/usr/bin/env python3
"""Notch check: ix-notch (J6.1-J6.5, J6.7) failure paths and absences the journey leaves out.

Drives the RUNNING rig copy over the control socket (docs/CONTROL-SOCKET.md) through journeys/lib; expected
values come from fixtures/tests-j6.json, docs/design/fixtures/mac-main-sample.json and docs/CONSTANTS.md, never
from a built screen. The `fixture` verbs it uses are listed in docs/lanes/tests-j6.md "Requests to the root".
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/notch-check.py'
Self-test (no app): python3 checks/notch-check.py --selftest. Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
"""
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey                                  # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402
from lib.fixture import FX, const                                    # noqa: E402
import J6                                                            # noqa: E402
import J1                                                            # noqa: E402  (mac_language: the way the app picks its language when none was chosen)
from J6 import F6, FX_PATH, CODE, NEXT, NOTCH, LIST, MAIN, ids   # noqa: E402

TARGET = "LOCAL"
FAILING_LINE = FX["states"]["sync"]["failing_line"]
MUTED, FAIL = F6["arrivals"]["muted-mail"], F6["arrivals"]["failing-account-mail"]

ID = "notch"
TITLE = "Notch: muted accounts, no-notch panel, empty state, refusals, oversize drop"
VISIBLE_OUTCOME = "A muted account never drops; a refused send or archive says why and keeps the mail; an oversize file is refused before send"
deliver = lambda s, n: s.raw("fixture", "deliver", n, FX_PATH, "eventsource")
CATALOGUE = os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings")


def catalogue_value(key, lang, catalogue=None):
    """The catalogue's sentence for `key` in `lang` (the app speaks the language its copy was started in; a check never pins English)."""
    cat = catalogue if catalogue is not None else json.load(open(CATALOGUE, encoding="utf-8"))["strings"]
    v = cat.get(key, {}).get("localizations", {}).get(lang, {}).get("stringUnit", {}).get("value")
    if v is None:
        raise Unreadable("the catalogue has no %r in %r" % (key, lang))
    return v


def app_language(s, mac=None):
    """The language the copy speaks, read through the Settings row the app serves (`language`). The row says `system` when the person never picked one
    (main81: all three language cases were not judged for that); the copy then speaks what Words.swift gives it, which J1 reads the same way
    (`J1.mac_language`: the stored pick, the rig identity's language list, the Mac's list). A copy for which no language can be read is not judged."""
    code = s.ctl.rows("mac-settings-general.settings")[0].get("language")
    if code in (None, "", "system"):
        found, where = (mac or J1.mac_language)()
        if where.startswith("no language list"):
            raise Unreadable("the copy's language is %r and no language list could be read: it names no catalogue column to read the expected words from" % (code,))
        s.j.notes.append("the copy's language is system; read as %s from %s" % (found, where))
        return found
    return code


def words(s, key):
    return catalogue_value(key, app_language(s))


def expect_lazy(s, name, got_fn, want_fn):
    """`expect` for a value read at the check's own time (the catalogue sentence of the copy's language needs the socket, so a dry run cannot form it)."""
    def run():
        got = got_fn()
        want = want_fn()
        s._eg = (want, got)
        return got == want, "expected %r, got %r" % (want, got)
    s._record(name, run)


def server_notch(s):
    """The Notch window's bounds as the window server reports them (Y from the top of the screen)."""
    s.ctl.rows("mac-notch")       # dry run: raises, the check is not judged
    title = words(s, "Notch")     # the window's title is the catalogue word for the copy's language (NotchIds.swift:37)
    js = ('ObjC.import("CoreGraphics");var l=ObjC.deepUnwrap(ObjC.castRefToObject($.CGWindowListCopyWindowInfo('
          '$.kCGWindowListOptionAll,0)));JSON.stringify(l.filter(function(w){return w.kCGWindowName==' + json.dumps(title) + '})'
          '.map(function(w){return {y:w.kCGWindowBounds.Y}}))')
    out = subprocess.run(["osascript", "-l", "JavaScript", "-e", js], capture_output=True, text=True, timeout=20)
    if out.returncode != 0:
        raise Unreadable("window server read failed: %s" % out.stderr.strip()[:120])
    found = json.loads(out.stdout or "[]")
    if not found:      # the title searched for is part of the answer: an empty list for a title the window does not carry decides nothing
        raise Unreadable("the window server lists no window titled %r" % title)
    return found


def drive(j):
    d = F6["drop"]
    s = j.step("J6.3", "Nothing new")
    s.raw("fixture", "pointer-rest", NOTCH)
    # F6["empty_line"] is the English key; the copy shows it in its own language, read from the catalogue
    expect_lazy(s, "empty-line", lambda: [r["text"] for r in s.ctl.rows(NOTCH + ".empty")], lambda: [words(s, F6["empty_line"])])

    s = j.step("J6.1", "Muted accounts never drop")
    s.raw("fixture", "mute-account", F6["muted_account"])
    deliver(s, "muted-mail")
    s.expect("muted-drops-nothing", lambda: ids(s), [])
    s.expect("muted-mail-still-in-inbox", lambda: MUTED["id"] in ids(s, MAIN), True)
    deliver(s, "login-code")
    s.expect("unmuted-control-drops", lambda: ids(s), [CODE["id"]])
    s.expect("empty-line-gone-with-mail", lambda: s.ctl.rows(NOTCH + ".empty"), [])
    deliver(s, "login-code")
    s.expect("resync-no-duplicate", lambda: ids(s), [CODE["id"]])

    s = j.step("J6.1", "A Mac without a notch")
    s.raw("fixture", "notch-geometry", "none")
    deliver(s, "next-mail")
    s.expect("no-notch-still-drops", lambda: ids(s), [NEXT["id"], CODE["id"]])
    # winframes is AppKit's frame (origin bottom-left), so its y is not the panel's top. The window server's own
    # bounds (origin top-left) are: a rig copy parks its panel `parked` points below the screen (NOTCH_RIG_PARK_PT),
    # so the top edge less that offset is 0 when the panel sits at the top of the screen it targets.
    s.expect("panel-at-top-when-no-notch", lambda: [w["y"] - int(s.ctl.rows("mac-notch")[0].get("parked", 0)) for w in server_notch(s)], [0])

    s = j.step("J6.4", "A refused send keeps the text")
    deliver(s, "failing-account-mail")
    s.click(LIST + ".row.1")
    s.expect("row-opens-in-place", lambda: [r["id"] for r in s.ctl.rows(NOTCH + ".open.body")], [FAIL["id"]])
    s.click(NOTCH + ".open.quickreply")
    s.type(F6["reply"])
    s.key("⌘Enter")
    s.expect("failed-send-says-why", lambda: [r["text"] for r in s.ctl.rows(NOTCH + ".error")], [FAILING_LINE])
    s.expect("failed-send-keeps-text", lambda: s.ctl.rows(NOTCH + ".open.quickreply")[0]["text"], F6["reply"])
    s.expect("failed-send-queues-nothing", lambda: s.ctl.core("outbox_entry/count"), 0)

    s = j.step("J6.5", "A refused archive keeps the mail")
    s.click(LIST + ".row.2.archive")
    s.expect("control-archive-works", lambda: ids(s), [FAIL["id"], CODE["id"]])
    s.click(LIST + ".row.1.archive")
    s.expect("refused-says-why", lambda: [r["text"] for r in s.ctl.rows(NOTCH + ".error")], [FAILING_LINE])
    s.expect("refused-mail-stays", lambda: FAIL["id"] in ids(s), True)
    s.expect("refused-not-archived", lambda: s.ctl.core("message/%s/archived" % FAIL["id"]), False)

    s = j.step("J6.7", "An oversize file is refused before send")
    s.raw("fixture", "drop-file", NOTCH, d["file"])
    s.expect("normal-drop-attaches", lambda: [a["name"] for a in s.ctl.rows("mac-composer.attachments")], [d["file"]])
    # The rig writes a bare name as a file of a few bytes (NotchEvents.rigFile), never over the limit; a name with a folder is used as given
    # (the same door), so the check writes a real file one byte over ATTACHMENT_FILE_MAX_BYTES (sparse: it takes no disk) and drops that path.
    big = oversize_path(j, d["oversize_file"])
    s.raw("fixture", "drop-file", NOTCH, big)
    # the refusal is the composer's own line (`mac-composer.line`: ComposerWords.refusal "attachment_too_large", the core's refusal code), in the copy's language
    expect_lazy(s, "oversize-says-so", lambda: [r["text"] for r in s.ctl.rows("mac-composer.line")], lambda: [words(s, "This file is too large to send.")])
    s.expect("oversize-not-attached", lambda: [a["name"] for a in s.ctl.rows("mac-composer.attachments")], [d["file"]])


def oversize_path(j, name):
    """A file `name` one byte over the limit, in this repo's scratch folder; the bare name in a dry run (nothing is written)."""
    if j.ctl is None:
        return name
    folder = os.path.join(ROOT, "_captures", ".tmp", "notch-oversize")
    os.makedirs(folder, exist_ok=True)
    path = os.path.join(folder, name)
    with open(path, "wb") as f:
        f.truncate(const("ATTACHMENT_FILE_MAX_BYTES") + 1)
    return path


# MUTANTS: per check, the mutation of the app that must turn it red.
# J6.3/empty-line | the empty notch says nothing
# J6.1/muted-drops-nothing | a muted account's mail drops into the notch
# J6.1/muted-mail-still-in-inbox | muting an account deletes its mail from the inbox
# J6.1/unmuted-control-drops | no mail drops at all (the muted check would pass for the wrong reason)
# J6.1/empty-line-gone-with-mail | the empty line stays when mail is listed
# J6.1/resync-no-duplicate | a resync drops the same mail twice
# J6.1/no-notch-still-drops | without a notch nothing drops
# J6.1/panel-at-top-when-no-notch | without a notch there is no top-of-screen panel
# J6.4/row-opens-in-place | the mail does not open in place
# J6.4/failed-send-says-why | a refused send is silent
# J6.4/failed-send-keeps-text | a refused send clears the typed text
# J6.4/failed-send-queues-nothing | a refused send still queues an outbox entry
# J6.5/control-archive-works | the archive button does nothing for an ordinary mail
# J6.5/refused-says-why | a refused archive is silent
# J6.5/refused-mail-stays | a refused archive removes the mail from the notch anyway
# J6.5/refused-not-archived | a refused archive is written as archived
# J6.7/normal-drop-attaches | the drop attaches nothing
# J6.7/oversize-says-so | an oversize file is refused without a line
# J6.7/oversize-not-attached | an oversize file is attached anyway


def selftest():
    """No app: the dry structure (every check has its mutant, no stray one) and the language helpers on stand-ins, each with its mutant. Exit 0 when all hold."""
    import types
    bad = []

    def report(ok, what):
        print(("ok   " if ok else "FAIL ") + what)
        if not ok:
            bad.append(what)

    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry=True)
    report(r["failed"] == 0 and not r["no_mutant"] and not r["stray_mutant"] and r["checks"] > 0,
           "dry run: %d checks, %d failed, no check without a mutant, no stray mutant" % (r["checks"], r["failed"]))
    en, fr = catalogue_value(F6["empty_line"], "en"), catalogue_value(F6["empty_line"], "fr")
    report(en == F6["empty_line"] and fr != en, "the catalogue gives the empty line in two languages (%r, %r)" % (en, fr))
    try:
        catalogue_value("no such key in the catalogue", "fr")
        report(False, "a key the catalogue lacks is not judged (it answered)")
    except Unreadable:
        report(True, "a key the catalogue lacks is not judged")
    stub = lambda lang: types.SimpleNamespace(ctl=types.SimpleNamespace(rows=lambda ax: [{"language": lang}]), j=types.SimpleNamespace(notes=[]))
    report(app_language(stub("fr")) == "fr", "the language is read from the Settings row")
    try:
        app_language(stub("system"), mac=lambda: ("en", "no language list read: English"))
        report(False, "language 'system' with no readable language list is not judged (it answered)")
    except Unreadable:
        report(True, "language 'system' with no readable language list is not judged")
    sy = stub("system")
    report(app_language(sy, mac=lambda: ("fr", "AppleLanguages of the Mac")) == "fr" and "read as fr" in sy.j.notes[0],
           "language 'system' is read the way the app picks it (the Mac's list) and the note says where")
    report(app_language(stub(""), mac=lambda: ("de", "whoosh.language of the rig identity")) == "de", "an empty language row is read the same way")
    seen = {}
    rec = types.SimpleNamespace(_eg=None, _record=lambda name, fn: seen.__setitem__(name, fn()))
    expect_lazy(rec, "same", lambda: ["a"], lambda: ["a"])
    expect_lazy(rec, "other", lambda: ["a"], lambda: ["b"])
    report(seen["same"][0] is True and seen["other"][0] is False and "expected ['b'], got ['a']" in seen["other"][1],
           "expect_lazy passes equal values and fails different ones with both values in the line")
    # mutant: the French copy shows the French line; a check that pins the English key reads red on it
    report([fr] != [F6["empty_line"]], "mutant: pinning the English empty line is red on a French copy")
    print("notch selftest: %d problems" % len(bad))
    return 1 if bad else 0


def main():
    if "--selftest" in sys.argv:
        return selftest()
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    rows = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# J") and " | " in l]
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
