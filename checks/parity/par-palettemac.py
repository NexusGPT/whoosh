#!/usr/bin/env python3
"""par-palettemac: the Mac command palette in the shell's overlay slot, with jumps (todo ix-palettemac, round 2).

    sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/par-palettemac.py'

Drives the RUNNING rig copy over the control socket. Expected values come from docs/design/fixtures/mac-main-sample.json
(journeys/lib/fixture.py), never from a built screen. Prints one line per check, then 'N checks, F failed, U not judged';
exit 1 when F > 0 or U > 0.

What the owner sees, per check (the MUTANT column is the change to the app that must turn the check red):
  open-by-key               | ⌘P puts the palette up                                  | the palette key opens nothing
  type-to-filter            | typing "archive" lists Archive first                    | the filter ignores the query
  nonsense-finds-nothing    | a query that matches nothing lists nothing, in words    | every query lists every line
  down-up-move-the-line     | Down then Up move the highlight one line each           | the arrows do nothing
  enter-runs-the-command    | Enter on Archive archives the focused message           | Enter does nothing
  escape-closes             | Escape puts the inbox back                              | Escape leaves the palette open
  account-jump-filters      | Enter on an account filters the list to that account    | the jump does not set the filter
  split-tab-jump            | a tab line opens that split tab                         | the tab list is empty
  label-at-depth-3          | "Work › Clients › Provanta" is a line, found by typing  | labels deeper than two levels are dropped
  keys-only-while-focused   | with the palette closed, Down moves the inbox, not it    | the palette answers keys while closed
  empty-state-words         | the empty state says what is missing, in words          | the empty state is blank
  move-to-another-account   | "Move to another account" lists the other accounts      | the picker lists the current account too
  pick-moves-the-row        | picking Work moves the focused message to Work          | the pick does nothing
  not-spam-command          | "Not spam" is a palette line with no key, and Enter on it | the line is missing, shows a key, or Enter leaves the
                            | brings a Junk message back to the inbox (fixture-r3)     | message in Junk
  vip-mark-command          | "Mark as VIP" is a line on the focused mail with no key;  | the line is missing, shows a key, Enter marks nothing,
                            | Enter puts its sender in the VIPs mailbox; on that mail    | or the line never turns into "Remove from VIPs"
                            | the line reads "Remove from VIPs" and Enter takes it out   |

  doors-nav (docs/lanes/doors-nav.md): every control of the palette has a door and is clicked once
  every-door-of-the-palette-is-in-the-control-table  | PaletteMacFeature.swift registers a click door the table does not name
  every-control-the-palette-draws-has-a-door          | a palette view gives a control an id no door and no display-only line names
  scrim-click-closes-the-palette                      | a click on the dimmed window leaves the palette open
  close-door-closes-the-palette                       | the close door leaves the palette open
  next-and-previous-doors-move-the-highlight          | the next door moves nothing
  run-door-runs-the-highlighted-command               | the run door runs nothing
  command-row-click-runs-the-command                  | a click on a command line runs nothing
  move-row-click-opens-the-account-picker             | a click on "Move to another account" opens no picker
  label-row-click-selects-the-label                   | a click on a label line selects nothing in the sidebar
  nested-label-row-click-selects-the-label            | a label under a folded parent is not selected (the stand-in model; the real door is read by the next case)
  label-jump-opens-the-parents-first                  | the label line runs the sidebar's click door (refuses a folded parent), not its jump door
  label-jump-opens-the-folded-parents-driven          | the jump to a depth-3 label under a folded parent leaves a parent folded or the label unselected
  settings-row-click-opens-general-settings           | a click on the General settings line opens no settings surface

    python3 checks/parity/par-palettemac.py --selftest   the not-spam and vip-mark cases and every doors-nav case on a model of the palette, clean and with a mutant each
"""
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable  # noqa: E402
from lib.fixture import FX, ACCOUNT, MSG, ROWS  # noqa: E402

FIRST = FX["focused_message"]
FIRST_ACCOUNT = MSG[FIRST]["account"]
OTHER = "acc-04"            # Nordlicht, not the first message's account
CATALOGUE = os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings")


def catalogue_value(key, lang):
    """What the app shows for `key` in `lang`: the catalogue's value, else the key itself (Words.text answers `lookup(key) ?? fallback ?? key`, Words.swift:56).
    A key the catalogue does not hold at all is not judged (a typo here would otherwise pass as 'the app shows the key')."""
    cat = json.load(open(CATALOGUE, encoding="utf-8"))["strings"]
    if key not in cat:
        raise Unreadable("the catalogue has no key %r" % key)
    return cat[key].get("localizations", {}).get(lang, {}).get("stringUnit", {}).get("value") or key


def app_language(c):
    """The language the copy speaks, from the Settings row the app serves (`language`); 'system' names no catalogue column, so it is not judged."""
    code = c.rows("mac-settings-general.settings")[0].get("language")
    if code in (None, "", "system"):
        raise Unreadable("the copy's language is %r: it names no catalogue column to read the palette's words from" % (code,))
    return code


def T(c, key):
    """The palette line `key` (an English catalogue key) as the copy shows it."""
    return catalogue_value(key, app_language(c))


def typed_of(title, n=None):
    """The words typed to find a line: its title in lower case (the first n letters when n is given)."""
    t = title.lower()
    return t[:n] if n else t


def state(c):
    return c.rows("mac-palette.state")[0]


def reset(c):
    """Back to the inbox, no account filter, the first tab, the first row focused."""
    if c.uistate().get("surface") == "mac-palette":
        c.key("Escape")
    c.send("click", "mac-main.accounts.all")
    while c.uistate().get("focus_index", 0) > 0:
        c.key("K")
    for _ in FX["tabs"]:                    # Tab walks the split tabs round; stop on the first
        if c.uistate().get("tab") == FX["tabs"][0]["id"]:
            break
        c.key("Tab")


def opened(c, query=""):
    reset(c)
    c.key("⌘P")
    if query:
        c.type(query)


def c_open(c):
    reset(c)
    c.key("⌘P")
    s = c.uistate().get("surface")
    return s == "mac-palette", "surface %r after ⌘P" % s


def c_filter(c):
    title = T(c, "Archive")
    opened(c, typed_of(title))
    r = c.rows("mac-palette.results")
    return bool(r) and r[0]["title"] == title and r[0]["key"] == "E", "first line %r (wanted title %r, key 'E')" % (r[0] if r else None, title)


def line_key(c, query, title):
    opened(c, query)
    for r in c.rows("mac-palette.results"):
        if r["title"].lower() == title.lower():
            return r["key"]
    raise Unreadable("no palette line %r for %r; the lines listed: %s" % (title, query, [r["title"] for r in c.rows("mac-palette.results")][:8]))


def c_preset_keys(c):
    # each line shows the preset's chord (Superhuman v8 as bound in KeyPreset.swift): the sidebar's own row key, the fixed Shift+Tab
    # that sits in no preset row; never the ⌃⌘S the owner banned
    side, prev = T(c, "Hide or show the sidebar"), T(c, "Previous split")   # the registry title "previous split" is translated through the catalogue ("Previous split") like every palette line
    got = (line_key(c, typed_of(side), side), line_key(c, typed_of(prev), prev))
    return got == ("⌘\\", "Shift+Tab"), "sidebar line %r, previous-split line %r" % got


LANG_DOOR = "mac-settings-general.appearance.language.%s"   # the pick of Settings > General, a click door that needs no window (journeys/J3.py:127)


def c_french_titles(c):
    """With the copy speaking French, every palette line's title is a French catalogue text: none is an English id left untranslated (main93 finding: 28 registry
    titles such as "Previous split" had no catalogue key and showed in English in every language; "don't mix languages"). A line is bad when its title is not
    one of the catalogue's French values, or is an English catalogue key whose French text differs. At least 20 lines must be listed, so a palette that
    lists nothing cannot pass. The language is put back as it was."""
    was = c.rows("mac-settings-general.settings")[0].get("language") or "system"
    c.click(LANG_DOOR % "fr")
    try:
        opened(c)
        titles = [r["title"] for r in c.rows("mac-palette.results")]
        cat = json.load(open(CATALOGUE, encoding="utf-8"))["strings"]
        def fr(k):
            return cat[k].get("localizations", {}).get("fr", {}).get("stringUnit", {}).get("value")
        fr_values = {fr(k) for k in cat if fr(k)}
        bad = [t for t in titles if t not in fr_values or (t in cat and fr(t) and fr(t) != t)]
        return (len(titles) >= 20 and not bad), "%d lines, not French: %s" % (len(titles), bad[:8])
    finally:
        c.click(LANG_DOOR % was)


def c_nothing(c):
    opened(c, "zzqxv")
    n = len(c.rows("mac-palette.results")) + len(c.rows("mac-palette.jumps"))
    return n == 0, "%d lines for a nonsense query" % n


def c_updown(c):
    opened(c)
    a = state(c)["selected"]
    c.key("Down")
    b = state(c)["selected"]
    c.key("Up")
    d = state(c)["selected"]
    return (a, b, d) == (0, 1, 0), "selected %s -> %s -> %s" % (a, b, d)


def c_enter(c):
    opened(c, typed_of(T(c, "Archive")))
    c.key("Enter")
    gone = c.core("message/%s/archived" % FIRST)
    surface = c.uistate().get("surface")
    c.send("key", "Z")                      # undo, so later checks see the same inbox
    return str(gone).lower() == "true" and surface == "mac-main", "archived %r, surface %r" % (gone, surface)


def c_escape(c):
    opened(c, typed_of(T(c, "Archive"), 4))
    c.key("Escape")
    s = c.uistate().get("surface")
    return s == "mac-main", "surface %r after Escape" % s


def jump(c, kind, title):
    for r in c.rows("mac-palette.jumps"):
        if r["kind"] == kind and r["title"] == title:
            return r
    return None


def c_account(c):
    name = ACCOUNT[OTHER]["name"]
    opened(c, name)
    r = jump(c, "account", name)
    if not r:
        return False, "no account line for %r" % name
    c.click(r["ax"])
    st = c.uistate()
    rows = c.rows("mac-main.list")
    # the core keeps a pinned message in every account's list; every other row must be the account's own
    only = bool(rows) and all(x["account"] == OTHER or x["pinned"] for x in rows)
    seen = sorted({x["account"] for x in rows if not x["pinned"]})
    c.send("click", "mac-main.accounts.all")
    return st.get("account_filter") == OTHER and st.get("surface") == "mac-main" and only, \
        "filter %r, %d rows, unpinned ones from accounts %s (want only %s)" % (st.get("account_filter"), len(rows), seen, OTHER)


def c_tab(c):
    tab = FX["tabs"][1]
    opened(c, tab["name"])
    r = jump(c, "tab", tab["name"])
    if not r:
        return False, "no tab line for %r" % tab["name"]
    c.click(r["ax"])
    got = c.uistate().get("tab")
    return got == tab["id"], "tab %r after the jump (want %r)" % (got, tab["id"])


def c_label3(c):
    want = "Work › Clients › Provanta"
    opened(c, "provanta")
    r = jump(c, "label", want)
    deeper = jump(c, "label", "Work › Clients › Provanta › RFP 2026 › Q4")
    return r is not None and deeper is not None, "depth 3 line %s, depth 5 line %s" % (bool(r), bool(deeper))


def c_focus(c):
    reset(c)
    before = c.uistate().get("focus_index")
    c.key("Down")
    after = c.uistate().get("focus_index")
    return len(ROWS) > 1 and after == before + 1 and c.uistate().get("surface") == "mac-main", "focus_index %s -> %s" % (before, after)


def c_empty(c):
    opened(c, "zzqxv")
    s = state(c)
    return s["empty"] and "zzqxv" in s["words"], "words %r" % s["words"]


def c_move(c):
    opened(c, typed_of(T(c, "Move to another account")))   # the whole title: "move" alone lists other Move commands first (main88: Enter ran one of them, step stayed commands)
    on_first = c.uistate().get("focus_message") == FIRST
    c.key("Enter")
    st = state(c)
    ids = [r["id"] for r in c.rows("mac-palette.accounts")]
    ok = st["step"] == "accounts" and FIRST_ACCOUNT not in ids and len(ids) == len(ACCOUNT) - 1 and bool(on_first)
    c.key("Escape")
    return ok, "focus %s, step %r, %d accounts (want %d), current account listed: %s; picker ids %s" % (on_first, st["step"], len(ids), len(ACCOUNT) - 1, FIRST_ACCOUNT in ids, ids[:8])


def c_pick(c):
    work = "acc-01"
    opened(c, typed_of(T(c, "Move to another account")))   # the whole title: "move" alone lists other Move commands first (main88: Enter ran one of them, step stayed commands)
    c.key("Enter")
    ids = [r["id"] for r in c.rows("mac-palette.accounts")]
    if work not in ids:
        return False, "the Work account is not in the picker: %s" % ids[:3]
    from_acc = c.core("message/%s/account" % FIRST)
    c.click("mac-palette.accounts.%d" % ids.index(work))
    to_acc = c.core("message/%s/account" % FIRST)
    surface = c.uistate().get("surface")
    return str(to_acc).strip('"') == work and surface == "mac-main", "account %s -> %s, surface %r" % (from_acc, to_acc, surface)


def c_not_spam(c):
    """"Not spam" has no key (the owner's rule: Superhuman's list has none), and the command returns the Junk message on screen to the inbox."""
    title = T(c, "Not spam")
    opened(c, typed_of(title))
    r = c.rows("mac-palette.results")
    line = r[0] if r else None
    if not line or line["title"] != title:
        return False, "first line for %r is %r; the lines listed: %s" % (typed_of(title), line, [x["title"] for x in r][:8])
    if line["key"] != "":
        return False, "the Not spam line shows the key %r; it has none" % line["key"]
    c.key("Escape")
    reset(c)
    c.key("⇧!")                                      # junk the focused message, then look at Junk
    c.send("click", "mac-main.sidebar.command.go-spam")
    mid = c.uistate().get("focus_message")
    if str(c.core("message/%s/spam" % mid)).lower() != "true":
        raise Unreadable("no junk message is focused in the Junk view (focus %r)" % mid)
    c.key("⌘P")
    c.type(typed_of(title))
    c.key("Enter")
    after = c.core("message/%s/spam" % mid)
    c.send("click", "mac-main.sidebar.command.go-inbox")
    return str(after).lower() == "false", "message %s: spam is %r after Not spam (want false)" % (mid, after)



def vips_rows(c):
    """How many rows the VIPs mailbox lists now (`mailbox/<name>/rows`, the test engine's state path)."""
    v = c.core("mailbox/vips/rows")
    if v is None:
        raise Unreadable("the core answers nothing for mailbox/vips/rows")
    return int(str(v).strip('"'))


def c_vip_mark(c):
    """vip-mark: one palette line on the focused mail, no key (owner, 7 Nov). "Mark as VIP" puts the mail's sender in the VIPs mailbox; on a mail of
    that sender (the VIPs mailbox's first row) the same line reads "Remove from VIPs", and Enter on it takes the sender out again."""
    mark, remove = T(c, "Mark as VIP"), T(c, "Remove from VIPs")
    reset(c)
    mid = c.uistate().get("focus_message")
    if not mid:
        raise Unreadable("no message is focused in the inbox")
    before = vips_rows(c)
    opened(c, typed_of(mark))
    r = c.rows("mac-palette.results")
    line = r[0] if r else None
    if not line or line["title"] != mark:
        return False, "first line for %r is %r; the lines listed: %s" % (typed_of(mark), line, [x["title"] for x in r][:8])
    if line["key"] != "":
        return False, "the %r line shows the key %r; it has none" % (mark, line["key"])
    c.key("Enter")
    marked = vips_rows(c)
    if marked <= before:
        return False, "message %s: the VIPs mailbox lists %d rows before and %d after %r (want more)" % (mid, before, marked, mark)
    c.send("click", "mac-main.sidebar.mailbox.vips")   # a mail of that sender, where the line must turn round
    c.key("⌘P")
    c.type(typed_of(remove))
    r = c.rows("mac-palette.results")
    line = r[0] if r else None
    if not line or line["title"] != remove:
        c.key("Escape")
        c.send("click", "mac-main.sidebar.command.go-inbox")
        return False, "on a VIP's mail the first line for %r is %r; the lines listed: %s" % (typed_of(remove), line, [x["title"] for x in r][:8])
    c.key("Enter")
    after = vips_rows(c)
    c.send("click", "mac-main.sidebar.command.go-inbox")
    return after == before, "VIPs mailbox rows: %d before, %d after %r, %d after %r (want back to %d)" % (before, marked, mark, after, remove, before)


# ---- doors-nav (docs/lanes/doors-nav.md) ---------------------------------------------------------------------------------------------------
FEATURE_DIR = os.path.join(ROOT, "Sources", "Features", "PaletteMac")
INTERP = re.compile(r"\\\((?:[^()]|\((?:[^()]|\([^()]*\))*\))*\)")
# Door table: every click id PaletteMacFeature.swift registers, the control it stands for, the case that clicks it. `*` stands for an interpolation.
PALETTE_DOORS = {
    "mac-palette.run": "the field's Return (onSubmit) and the highlighted line: run-door-runs-the-highlighted-command, enter-runs-the-command (key)",
    "mac-palette.next": "the field's Down arrow: next-and-previous-doors-move-the-highlight, down-up-move-the-line (keys)",
    "mac-palette.previous": "the field's Up arrow: next-and-previous-doors-move-the-highlight",
    "mac-palette.close": "Escape: close-door-closes-the-palette, escape-closes (key)",
    "mac-palette.scrim": "the dimmed window behind the card: scrim-click-closes-the-palette",
    "mac-palette.results.*": "a command line: command-row-click-runs-the-command, move-row-click-opens-the-account-picker, settings-row-click-opens-general-settings",
    "mac-palette.jumps.*": "a jump line (account, tab, label): account-jump-filters, split-tab-jump, label-row-click-selects-the-label",
    "mac-palette.accounts.*": "a line of the Move to another account picker: pick-moves-the-row",
}
PALETTE_DISPLAY = {
    "mac-palette": "the card",
    "mac-palette.field": "the text field: typed through the `type` verb (type-to-filter), Return and the arrows are the doors above",
    "mac-palette.empty": "the empty state's words (empty-state-words)",
}


def palette_sources():
    out = {}
    for name in sorted(os.listdir(FEATURE_DIR)):
        if name.endswith(".swift"):
            with open(os.path.join(FEATURE_DIR, name), encoding="utf-8") as f:
                out[name] = f.read()
    return out


def palette_doors(files):
    out, fn = set(), ""
    for line in files["PaletteMacFeature.swift"].splitlines():
        m = re.search(r"\bfunc (\w+)", line)
        if m:
            fn = m.group(1)
        if fn != "clickTargets":
            continue
        m = re.search(r'\bt\["([^"]+)"\]\s*=', line) or re.match(r'\s*"(mac-palette[^"]*)":\s*\{', line)
        if m:
            out.add(INTERP.sub("*", m.group(1)))
    return out


def palette_controls(files):
    out = set()
    for text in files.values():
        for m in re.finditer(r'(?:whooshAx\(|\bax:\s*)"(mac-palette[^"]*)"', text):
            out.add(INTERP.sub("*", m.group(1)))
    return out


def c_door_table(c, files=None):
    files = files or palette_sources()
    doors = palette_doors(files)
    extra, lost = sorted(doors - set(PALETTE_DOORS)), sorted(set(PALETTE_DOORS) - doors)
    return not extra and not lost, "doors the table lacks %s, table rows the file lacks %s" % (extra, lost)


def c_control_table(c, files=None):
    files = files or palette_sources()
    unknown = sorted(palette_controls(files) - set(PALETTE_DOORS) - set(PALETTE_DISPLAY))
    return not unknown, "control ids with no door and no display-only line: %s" % unknown


def surface(c):
    return c.uistate().get("surface")


def c_scrim(c):
    opened(c)
    c.click("mac-palette.scrim")
    return surface(c) == "mac-main", "surface %r after a click on the scrim" % surface(c)


def c_close_door(c):
    opened(c, typed_of(T(c, "Archive"), 4))
    c.click("mac-palette.close")
    return surface(c) == "mac-main", "surface %r after the close door" % surface(c)


def c_next_previous(c):
    opened(c)
    a = state(c)["selected"]
    c.click("mac-palette.next")
    b = state(c)["selected"]
    c.click("mac-palette.previous")
    d = state(c)["selected"]
    return (a, b, d) == (0, 1, 0), "selected %s -> %s -> %s" % (a, b, d)


def archived_then_undo(c):
    gone = c.core("message/%s/archived" % FIRST)
    surf = surface(c)
    c.send("key", "Z")                      # undo, so later checks see the same inbox
    return str(gone).lower() == "true" and surf == "mac-main", "archived %r, surface %r" % (gone, surf)


def c_run_door(c):
    opened(c, typed_of(T(c, "Archive")))
    c.click("mac-palette.run")
    return archived_then_undo(c)


def line_index(c, title):
    for i, r in enumerate(c.rows("mac-palette.results")):
        if r["title"].lower() == title.lower():
            return i
    raise Unreadable("no palette line %r; the lines listed: %s" % (title, [r["title"] for r in c.rows("mac-palette.results")][:8]))


def c_row_click(c):
    opened(c, typed_of(T(c, "Archive")))
    c.click("mac-palette.results.%d" % line_index(c, T(c, "Archive")))
    return archived_then_undo(c)


def c_move_row(c):
    title = T(c, "Move to another account")
    opened(c, typed_of(title.split()[0]))
    c.click("mac-palette.results.%d" % line_index(c, title))
    st = state(c)
    n = len(c.rows("mac-palette.accounts"))
    c.key("Escape")
    return st["step"] == "accounts" and n == len(ACCOUNT) - 1, "step %r, %d accounts in the picker (want %d)" % (st["step"], n, len(ACCOUNT) - 1)


def label_jump(c, title):
    opened(c, title.split(" › ")[-1].lower())
    r = jump(c, "label", title)
    if not r:
        raise Unreadable("no label line %r" % title)
    c.click(r["ax"])
    sel = c.rows("mac-main.sidebar.selection")[0]["id"]
    c.send("click", "mac-main.sidebar.mailbox.all-inboxes")
    return sel == "label." + r["id"].split(":", 1)[1], "sidebar selection %r after a click on the line (want label.%s)" % (sel, r["id"].split(":", 1)[1])


def c_label_row(c):
    return label_jump(c, "Finance")


def c_label_nested(c):
    return label_jump(c, "Work › Clients › Provanta")


def c_label_parents_driven(c):
    """The jump to a nested label opens the parents first, driven (the source half is c_label_jump_door): the label's top parent is folded with the Left key
    on the sidebar, the palette jump to the depth-3 label is clicked, and the sidebar rows are read: every parent open, the label drawn, the label selected."""
    title = "Work › Clients › Provanta"
    labels = lambda: {r["id"]: r for r in c.rows("mac-main.sidebar.labels")}
    opened(c, title.split(" › ")[-1].lower())
    r = jump(c, "label", title)
    if not r:
        raise Unreadable("no label line %r" % title)
    lid = r["id"].split(":", 1)[1]
    c.key("Escape")
    tree = labels()
    chain, p = [], tree[lid]["parent"]
    while p and p in tree and len(chain) < 10:
        chain.append(p)
        p = tree[p]["parent"]
    if not chain:
        raise Unreadable("label %s has no parent in the sidebar rows" % lid)
    c.send("click", "mac-main.pane.sidebar")
    c.send("click", "mac-main.sidebar.label." + chain[-1])
    c.key("Left")
    folded = labels()[chain[-1]]["open"]
    if folded:
        raise Unreadable("the top parent %s stayed open after Left on it: nothing is folded to reveal" % chain[-1])
    opened(c, title.split(" › ")[-1].lower())
    r = jump(c, "label", title)
    c.click(r["ax"])
    after = labels()
    sel = c.rows("mac-main.sidebar.selection")[0]["id"]
    c.send("click", "mac-main.sidebar.mailbox.all-inboxes")
    closed = [x for x in chain if not after[x]["open"]]
    ok = not closed and after[lid]["visible"] and sel == "label." + lid
    return ok, "parents %s (top one folded first): still folded %s; label %s visible %s; sidebar selection %r (want label.%s)" % (chain, closed, lid, after[lid]["visible"], sel, lid)


SIDEBAR_TARGETS = os.path.join(ROOT, "Sources", "Features", "Sidebar", "SidebarTargets.swift")


def c_label_jump_door(c, files=None):
    """F-P1 (docs/lanes/sidebar-r9.md): the jump runs the door that opens the parents (`.jump`), and the Sidebar registers it.
    A click on a drawn row keeps its own door, which refuses a row that is not drawn. Read from the Swift sources: the driven case needs a build."""
    files = files or palette_sources()
    acts = files["PaletteMacActs.swift"]
    m = re.search(r'case \.jumpLabel\(let id\):.*?Registrations\.click\("(mac-main\.sidebar\.label\.\\\(id\)(\.\w+)?)"\)', acts, re.S)
    if not m:
        return False, "no jumpLabel case in PaletteMacActs.swift that runs a sidebar label door"
    door = INTERP.sub("*", m.group(1))
    with open(SIDEBAR_TARGETS, encoding="utf-8") as f:
        targets = f.read()
    registered = re.search(r't\["(mac-main\.sidebar\.label\.\\\(l\.id\)\.jump)"\]\s*=\s*\{ _ in st\.revealLabel\(l\.id\)', targets)
    return door == "mac-main.sidebar.label.*.jump" and bool(registered), "the jump line runs %s; the Sidebar registers the jump door that reveals the label: %s" % (door, bool(registered))


def c_settings_row(c):
    title = T(c, "General settings")
    opened(c, typed_of(title))
    c.click("mac-palette.results.%d" % line_index(c, title))
    got = surface(c)
    try:
        c.send("click", "macshell.window.close-others")
    except Unreadable:
        pass
    c.send("click", "dock.whoosh")
    return got == "mac-settings-general", "surface %r after a click on General settings" % got


DOOR_CHECKS = [("every-door-of-the-palette-is-in-the-control-table", c_door_table), ("every-control-the-palette-draws-has-a-door", c_control_table),
               ("scrim-click-closes-the-palette", c_scrim), ("close-door-closes-the-palette", c_close_door),
               ("next-and-previous-doors-move-the-highlight", c_next_previous), ("run-door-runs-the-highlighted-command", c_run_door),
               ("command-row-click-runs-the-command", c_row_click), ("move-row-click-opens-the-account-picker", c_move_row),
               ("label-row-click-selects-the-label", c_label_row), ("nested-label-row-click-selects-the-label", c_label_nested),
               ("label-jump-opens-the-parents-first", c_label_jump_door),
               ("label-jump-opens-the-folded-parents-driven", c_label_parents_driven),
               ("settings-row-click-opens-general-settings", c_settings_row)]


class PalCtl:
    """The stand-in for the doors-nav cases: the palette, the message it archives, the sidebar's selection. A mutant breaks exactly one control."""
    COMMAND_KEYS = ["Archive", "Move to another account", "General settings", "Not spam"]
    LABELS = {"Finance": "lbl-14", "Work › Clients › Provanta": "lbl-3"}
    TREE = {"lbl-1": "", "lbl-2": "lbl-1", "lbl-3": "lbl-2", "lbl-14": ""}   # id -> parent id

    def __init__(self, mutant=None, lang="en"):
        self.mutant, self.lang = mutant, lang
        self.T = {k: catalogue_value(k, lang) for k in self.COMMAND_KEYS}       # the lines in the language the model speaks
        self.COMMANDS = list(self.T.values())
        self.open_, self.query, self.selected, self.step, self.sel, self.archived, self.surf = False, "", 0, "commands", "mailbox.all-inboxes", False, "mac-main"
        self.folds, self.focus_label = set(), None      # the sidebar's open flags: Work (lbl-1) and Clients (lbl-2) are open until the Left key folds one

    def mut(self, n):
        return self.mutant == n

    def parents(self, i):
        out = []
        while self.TREE.get(i):
            i = self.TREE[i]
            out.append(i)
        return out

    def lines(self):
        q = self.query.lower()
        return [t for t in self.COMMANDS if q in t.lower()]

    def jumps(self):
        q = self.query.lower()
        return [t for t in self.LABELS if q in t.lower()]

    def uistate(self):
        return {"surface": "mac-palette" if self.open_ else self.surf, "focus_index": 0, "tab": FX["tabs"][0]["id"], "focus_message": FIRST}

    def close(self):
        self.open_, self.query, self.selected, self.step = False, "", 0, "commands"

    def run(self, title):
        if title == self.T["Archive"]:
            self.close()
            self.archived = True
        elif title == self.T["Move to another account"]:
            self.query, self.selected, self.step = "", 0, "accounts"
        elif title == self.T["General settings"]:
            self.close()
            self.surf = "mac-settings-general"
        else:
            self.close()

    def key(self, k, *a):
        if k == "⌘P":
            self.open_, self.query, self.selected, self.step = True, "", 0, "commands"
        elif k == "Escape" and self.open_:
            self.close()
        elif k == "Z":
            self.archived = False
        elif k == "Left" and not self.open_ and self.focus_label in self.TREE and self.focus_label not in self.folds:
            self.folds.add(self.focus_label)
        return {}

    def type(self, q):
        self.query += q
        self.selected = 0
        return {}

    def send(self, verb, ax=None, *a):
        if verb == "click":
            self.click(ax)
        elif verb == "key":
            self.key(ax)
        return {"ok": True}

    def click(self, ax):
        if ax == "dock.whoosh":
            self.surf = "mac-main"
        elif ax.startswith("mac-main.sidebar.label.") and not self.open_:
            self.focus_label = ax.rsplit(".", 1)[1]
        elif ax == "mac-main.pane.sidebar":
            pass
        elif ax == "macshell.window.close-others" or ax.startswith("mac-main.accounts.") or ax == "mac-main.sidebar.mailbox.all-inboxes":
            if ax == "mac-main.sidebar.mailbox.all-inboxes":
                self.sel = "mailbox.all-inboxes"
        elif not self.open_:
            raise Unreadable("click %s: refused (palette closed)" % ax)
        elif ax == "mac-palette.scrim":
            if not self.mut("scrim-click-closes-the-palette"):
                self.close()
        elif ax == "mac-palette.close":
            if not self.mut("close-door-closes-the-palette"):
                self.close()
        elif ax == "mac-palette.next":
            if not self.mut("next-and-previous-doors-move-the-highlight"):
                self.selected = min(self.selected + 1, len(self.lines()) + len(self.jumps()) - 1)
        elif ax == "mac-palette.previous":
            self.selected = max(self.selected - 1, 0)
        elif ax == "mac-palette.run":
            lines = self.lines()
            if lines and not self.mut("run-door-runs-the-highlighted-command"):
                self.run(lines[self.selected])
        elif ax.startswith("mac-palette.results."):
            t = self.lines()[int(ax.rsplit(".", 1)[1])]
            if self.mut("command-row-click-runs-the-command") and t == self.T["Archive"]:
                pass
            elif self.mut("move-row-click-opens-the-account-picker") and t == self.T["Move to another account"]:
                pass
            elif self.mut("settings-row-click-opens-general-settings") and t == self.T["General settings"]:
                self.close()
            else:
                self.run(t)
        elif ax.startswith("mac-palette.jumps."):
            title = self.jumps()[int(ax.rsplit(".", 1)[1])]
            nested = "›" in title
            self.close()
            lid = self.LABELS[title]
            covered = any(a in self.folds for a in self.parents(lid))      # a label under a folded parent is not drawn
            if self.mut("label-jump-opens-the-folded-parents-driven") and covered:
                return {}                                                   # the click door's refusal: the parents stay folded, nothing is selected
            self.folds -= set(self.parents(lid))                            # the jump door opens the parents first
            if not (self.mut("label-row-click-selects-the-label") and not nested) and not (self.mut("nested-label-row-click-selects-the-label") and nested):
                self.sel = "label." + lid
        return {}

    def core(self, path):
        return "true" if self.archived else "false"

    def rows(self, ax):
        if ax == "mac-settings-general.settings":
            return [{"language": self.lang}]
        if ax == "mac-palette.state":
            n = len(self.lines()) + len(self.jumps()) if self.step == "commands" else len(ACCOUNT) - 1
            return [{"open": self.open_, "step": self.step, "query": self.query, "selected": self.selected, "count": n, "empty": n == 0, "words": ""}]
        if ax == "mac-palette.results":
            return [{"ax": "mac-palette.results.%d" % i, "id": t, "title": t, "key": "", "kind": "command", "selected": i == self.selected} for i, t in enumerate(self.lines())]
        if ax == "mac-palette.jumps":
            return [{"ax": "mac-palette.jumps.%d" % i, "id": "label:" + self.LABELS[t], "title": t, "detail": "Label", "kind": "label", "emblem": False, "selected": False}
                    for i, t in enumerate(self.jumps())]
        if ax == "mac-palette.accounts":
            return [{"ax": "mac-palette.accounts.%d" % i, "id": a, "title": a, "address": a} for i, a in enumerate(sorted(ACCOUNT)[1:])]
        if ax == "mac-main.sidebar.selection":
            return [{"id": self.sel}]
        if ax == "mac-main.sidebar.labels":
            return [{"id": i, "parent": par, "open": i not in self.folds, "visible": not any(a in self.folds for a in self.parents(i))} for i, par in self.TREE.items()]
        return []


def selftest_doors():
    """Every doors-nav case on the stand-in, clean and once per mutant. The two source checks read copies of the Swift files with one more door or control."""
    src0 = palette_sources()
    mutated = {
        "every-door-of-the-palette-is-in-the-control-table": lambda: dict(src0, **{"PaletteMacFeature.swift": src0["PaletteMacFeature.swift"].replace(
            '"mac-palette.run":', '"mac-palette.zzz": { _ in false },\n            "mac-palette.run":', 1)}),
        "every-control-the-palette-draws-has-a-door": lambda: dict(src0, **{"PaletteMacView.swift": src0["PaletteMacView.swift"].replace(
            '.whooshAx("mac-palette.field")', '.whooshAx("mac-palette.field").whooshAx("mac-palette.zzz")', 1)}),
        "label-jump-opens-the-parents-first": lambda: dict(src0, **{"PaletteMacActs.swift": src0["PaletteMacActs.swift"].replace(
            'sidebar.label.\\(id).jump")', 'sidebar.label.\\(id)")', 1)}),
    }
    for name, make in mutated.items():
        assert make() != src0, "mutant %s changes nothing: its anchor text is gone from the source" % name
    words = {}
    for line in open(os.path.abspath(__file__), encoding="utf-8"):
        m = re.match(r"\s{2}([\w-]+)\s+\|\s+(.+)", line)
        if m and m.group(1) in dict(DOOR_CHECKS):
            words[m.group(1)] = m.group(2).strip()
    bad = [n for n, _ in DOOR_CHECKS if n not in words]
    print("MUTANTS | check | clean | mutant | mutant result")
    for lang in ("en", "fr"):         # the stand-in speaks each language: a check that pins English words is red on the French one
        for name, fn in DOOR_CHECKS:
            def go(mutant):
                c = PalCtl(mutant, lang)
                try:
                    return fn(c, mutated[name]()) if name in mutated and mutant else fn(c)
                except Unreadable as e:
                    return False, "not judged: %s" % e
            ok, d = go(None)
            red, why = go(name)
            print("MUTANTS | %s | %s | %s | %s | %s" % (name, "green" if ok else "RED " + d, words.get(name, "?"), "red" if not red else "STILL GREEN", lang))
            bad += ["%s (%s)" % (name, lang)] if (not ok or red) else []
    print("doors-nav: %d checks x 2 languages, %d problems" % (len(DOOR_CHECKS), len(bad)))
    return 1 if bad else 0


CHECKS = [("open-by-key", c_open), ("type-to-filter", c_filter), ("nonsense-finds-nothing", c_nothing),
          ("lines-show-the-preset-keys", c_preset_keys), ("down-up-move-the-line", c_updown), ("enter-runs-the-command", c_enter), ("escape-closes", c_escape),
          ("account-jump-filters", c_account), ("split-tab-jump", c_tab), ("label-at-depth-3", c_label3),
          ("titles-are-french-in-french", c_french_titles), ("keys-only-while-focused", c_focus), ("empty-state-words", c_empty), ("move-to-another-account", c_move), ("pick-moves-the-row", c_pick), ("not-spam-command", c_not_spam), ("vip-mark-command", c_vip_mark)] + DOOR_CHECKS


def run(c):
    out = []
    for name, fn in CHECKS:
        try:
            ok, d = fn(c)
            out.append((name, "pass" if ok else "fail", d))
        except Unreadable as e:
            out.append((name, "unjudged", str(e)))
        except (KeyError, IndexError, TypeError) as e:
            out.append((name, "fail", "reply shape: %s" % e))
    return out


def report(res):
    for n, st, d in res:
        print("%-28s %-8s %s" % (n, st, d))
    f, u = sum(r[1] == "fail" for r in res), sum(r[1] == "unjudged" for r in res)
    print("%d checks, %d failed, %d not judged" % (len(res), f, u))
    return 1 if f or u else 0


class ModelCtl:
    """A model of the palette and the Junk view, only as far as the not-spam case reads it. `mutant`: no-line | has-key | does-nothing."""

    def __init__(self, mutant=None, lang="en"):
        self.mutant, self.spam, self.mailbox, self.open_, self.query, self.focus = mutant, set(), "inbox", False, "", FIRST
        self.lang, self.title = lang, catalogue_value("Not spam", lang)

    def uistate(self):
        return {"surface": "mac-palette" if self.open_ else "mac-main", "focus_index": 0, "tab": FX["tabs"][0]["id"], "focus_message": self.focus}

    def key(self, k, *a):
        if k == "⌘P": self.open_, self.query = True, ""
        elif k == "Escape": self.open_ = False
        elif k == "⇧!" and not self.open_: self.spam.add(self.focus); self.focus = "next-message"
        elif k == "Enter" and self.open_:
            self.open_ = False
            if self.query == self.title.lower() and self.mutant != "does-nothing" and self.mailbox == "junk": self.spam.discard(self.focus)

    def type(self, q): self.query += q

    def rows(self, ax):
        if ax == "mac-settings-general.settings": return [{"language": self.lang}]
        if ax != "mac-palette.results" or not self.open_: return []
        lines = [] if self.mutant == "no-line" else [{"title": self.title, "key": "⇧N" if self.mutant == "has-key" else ""}]
        return [l for l in lines if self.query in l["title"].lower()]

    def send(self, verb, ax, *a):
        if verb == "click" and ax.endswith("go-spam"): self.mailbox, self.focus = "junk", sorted(self.spam)[0] if self.spam else self.focus
        if verb == "click" and ax.endswith(("go-inbox", "accounts.all")): self.mailbox = "inbox"

    def click(self, ax): self.send("click", ax)

    def core(self, path): return str(path.split("/")[1] in self.spam).lower()


class ModelVip:
    """A model of the palette, the inbox and the VIPs mailbox, only as far as the vip-mark case reads it. The palette's one VIP line is titled by
    whether the focused mail's sender is a VIP. `mutant`: no-line | has-key | does-nothing (Enter marks nothing) | never-flips (the line always
    reads "Mark as VIP")."""

    def __init__(self, mutant=None, lang="en"):
        self.mutant, self.lang = mutant, lang
        self.mark, self.remove = catalogue_value("Mark as VIP", lang), catalogue_value("Remove from VIPs", lang)
        self.vip, self.open_, self.query, self.box = set(), False, "", "inbox"
        self.inbox = [FIRST, "msg-other-sender"]

    def focus(self):
        rows = [m for m in self.inbox if m not in self.vip] if self.box == "inbox" else sorted(self.vip)   # a VIP's mail leaves the inbox for VIPs
        return rows[0] if rows else None

    def uistate(self):
        return {"surface": "mac-palette" if self.open_ else "mac-main", "focus_index": 0, "tab": FX["tabs"][0]["id"], "focus_message": self.focus()}

    def lines(self):
        if self.mutant == "no-line":
            return []
        vip = self.focus() in self.vip and self.mutant != "never-flips"
        return [{"title": self.remove if vip else self.mark, "key": "⇧V" if self.mutant == "has-key" else ""}]

    def key(self, k, *a):
        if k == "⌘P": self.open_, self.query = True, ""
        elif k == "Escape": self.open_ = False
        elif k == "Enter" and self.open_:
            hit = [l for l in self.lines() if self.query in l["title"].lower()]
            self.open_ = False
            if hit and self.mutant != "does-nothing":
                (self.vip.add if hit[0]["title"] == self.mark else self.vip.discard)(self.focus())

    def type(self, q): self.query += q

    def rows(self, ax):
        if ax == "mac-settings-general.settings": return [{"language": self.lang}]
        if ax != "mac-palette.results" or not self.open_: return []
        return [l for l in self.lines() if self.query in l["title"].lower()]

    def send(self, verb, ax, *a):
        if verb == "click" and ax == "mac-main.sidebar.mailbox.vips": self.box = "vips"
        if verb == "click" and ax.endswith(("go-inbox", "accounts.all")): self.box = "inbox"

    def click(self, ax): self.send("click", ax)

    def core(self, path): return len(self.vip) if path == "mailbox/vips/rows" else None


class ModelLang:
    """A stand-in for the palette's list in the language the Settings door picked. `mutant` english-titles: the lines stay English whatever the language."""
    KEYS = ["Archive", "Previous split", "Next split", "Send now", "Focus left", "Focus right", "Scroll down", "Scroll up", "Jump to top", "Jump to bottom",
            "Next message", "Previous message", "Expand message", "Outdent list", "Switch colours", "Write a reply draft", "Pop out reply", "Pop out forward",
            "Pop out new message", "Move contacts to bcc", "Palette", "Cycle through links and dates", "Open links and attachments", "Hide or show the sidebar"]

    def __init__(self, mutant=None):
        self.mutant, self.lang, self.open_ = mutant, "en", False

    def uistate(self): return {"surface": "mac-palette" if self.open_ else "mac-main", "focus_index": 0}
    def key(self, k, *a): self.open_ = k == "\u2318P"
    def type(self, q): pass
    def send(self, verb, ax, *a):
        if verb == "click" and ax.startswith("mac-settings-general.appearance.language."): self.lang = ax.rsplit(".", 1)[1]
    def click(self, ax): self.send("click", ax)

    def rows(self, ax):
        if ax == "mac-settings-general.settings": return [{"language": self.lang}]
        if ax != "mac-palette.results" or not self.open_: return []
        return [{"title": k if self.mutant == "english-titles" else catalogue_value(k, self.lang), "key": ""} for k in self.KEYS]


def selftest():
    bad = []
    ok, d = c_french_titles(ModelLang())
    print("clean model, French titles: %s %s" % ("pass" if ok else "FAIL", d))
    bad += [] if ok else ["clean model is red on titles-are-french-in-french"]
    ok, d = c_french_titles(ModelLang("english-titles"))
    print("mutant english-titles: %s %s" % ("red" if not ok else "STILL GREEN", d))
    bad += [] if not ok else ["mutant english-titles stayed green"]
    for lang in ("en", "fr"):
        ok, d = c_not_spam(ModelCtl(lang=lang))
        print("clean model (%s): %s %s" % (lang, "pass" if ok else "FAIL", d))
        bad += [] if ok else ["clean model is red in %s" % lang]
        for m in ("no-line", "has-key", "does-nothing"):
            try:
                ok, d = c_not_spam(ModelCtl(m, lang))
            except Unreadable as e:
                ok, d = False, str(e)
            print("mutant %-13s (%s): %s %s" % (m, lang, "still green" if ok else "red", d))
            bad += ["mutant %s stayed green in %s" % (m, lang)] if ok else []
        ok, d = c_vip_mark(ModelVip(lang=lang))
        print("clean model, vip-mark (%s): %s %s" % (lang, "pass" if ok else "FAIL", d))
        bad += [] if ok else ["clean model is red on vip-mark in %s" % lang]
        for m in ("no-line", "has-key", "does-nothing", "never-flips"):
            try:
                ok, d = c_vip_mark(ModelVip(m, lang))
            except Unreadable as e:
                ok, d = False, str(e)
            print("mutant vip-mark %-12s (%s): %s %s" % (m, lang, "still green" if ok else "red", d))
            bad += ["vip-mark mutant %s stayed green in %s" % (m, lang)] if ok else []
    fr, en = catalogue_value("Not spam", "fr"), catalogue_value("Not spam", "en")
    print("%s the French line differs from the English one (%r, %r): pinning English is red on a French copy" % ("ok  " if fr != en else "FAIL", fr, en))
    bad += [] if fr != en else ["the French and English lines are the same"]
    print("par-palettemac selftest: %s (%d problems)" % ("green" if not bad else "RED", len(bad)))
    return (1 if bad else 0) | selftest_doors()


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        sys.exit(selftest())
    try:
        ctl = Ctl().connect()
    except CouldNotDrive as e:
        sys.exit(report([(n, "unjudged", str(e)) for n, _ in CHECKS]))
    LANG_DOOR = "mac-settings-general.appearance.language.%s"   # the pick of Settings > General, a click door that needs no window (as journeys/J3.py:127)
    pinned = False
    try:
        if ctl.rows("mac-settings-general.settings")[0].get("language") in (None, "", "system"):
            ctl.click(LANG_DOOR % "en")      # a copy on 'system' names no catalogue column: pin English, put it back at the end
            pinned = True
        sys.exit(report(run(ctl)))
    finally:
        if pinned:
            ctl.click(LANG_DOOR % "system")
        ctl.close()
