#!/usr/bin/env python3
"""par-labelscontactsmac: what the owner does on the labels and contacts screen, through the core's delta-6 calls.

    python3 checks/parity/par-labelscontactsmac.py             drive the running rig copy; exit 1 when F > 0 or U > 0
    python3 checks/parity/par-labelscontactsmac.py --selftest  each check on a stand-in app, clean and with its mutant

What the owner sees, per check:
  drop-nests-a-label       a label dropped on another sits under it ("Work > Supplier ..."): the move reached the core.
                           Uses the socket `drag` verb; when a tap on a row does nothing (synthetic input inert) it
                           uses the screen's own click ids and says so in the detail.
  depth-refusal-in-words   a sixth level is refused, and the screen shows the core's sentence ("... at most 5 levels deep ...").
  duplicate-folder-refused a second smart folder with a name in use is refused with "the name exists already".

Round 2, the LC-S1 inspector (region mac-labels.inspector; every id below is `mac-labels.inspector.<id>`):
  inspector-opens-from-plus  the sidebar's "+" opens it on a NEW label; the reader leaves pane 3 (open=true, mode=new).
  rule-typed-live-count      a typed domain rule gives a live count, a per-rule count equal to it, and every matched row
                             carries the rule's tag; a subject rule adds the Subject tag; Matches | Mailbox switches pane 2.
  create-reads-in-tree       Create writes through createLabelFull: the label is in the core's tree and the inspector
                             now shows it as a modify of that label.
  modify-preview-and-rerun   Modify a saved label's rule: the preview names join, leave and stay (each row carries its edge;
                             delta 12: a label with a meaning has no "leave", what no rule matches stays),
                             Apply asks "Re-run on N messages?", R re-files, and the core's decided_by is read back.
  esc-returns-the-reader     Esc closes the inspector; the same key and the offer's Not now are two different doors.
  every-control-clicked      each control of the view is clicked once and its visible outcome asserted.

Round 3, the label keys through the core (contract delta 10), on the main list of the fixture core:
  key-y-removes-the-label    Y takes the label in view off the focused thread: the core's assignment loses it, focus stays.
  key-open-bracket-...       [ does the same and focus moves to the next thread; ] to the previous one.
  key-shift-y-removes-all    ⇧Y leaves the thread with no label in the core.
  key-v-moves-to-a-label     V opens the move picker (drawn: `measure mac-labels.move.picker`), typed words narrow it, its row
                             moves the thread: labelled and archived in the core, one undo (Z restores it).
  key-shift-p-pins           ⇧P flips the core's pin, and again flips it back.

Round 3, the inspector's layout (regions mac-labels.inspector.geometry and .rowfit), on the create and the modify path, at
1280x820 and 960x632, light and dark, English and French:
  inspector-layout-holds     no two control frames intersect; every row and control sits inside its card; the label column
                             starts at one x per column of cards; no label is narrower than its longest word (the Settings
                             round's rule). WHOOSH_R3_SHOTS=<dir> also shoots each state there.

Round 4, the labelling flow (contract delta 17; three cards: Entry rules, AI sorting, Exit filter; every id `mac-labels.inspector.<id>`;
regions .flow, .foot, .why; chains are named entry, other1.., exit):
  flow-cards-in-order        the board's example (À répondre), SPEC A140 (three steps: step 1 picks, the AI rules on each picked mail, step 3 filters
                             the accepted): the three cards in order, each on, "picks 37", "32 accepted · 5 waiting for the AI", "keeps 24 of 32",
                             the entry chain (ONE chain: the body contains "Hi Assem"), the exit chain (unanswered yes), Primary.
  flow-foot-provenance       the foot says "24 messages · 37 picked · 32 accepted · 8 to Primary": with the exit filter on the label holds what step 3 kept
                             (24); "picked" is step 1's count, "accepted" the AI's (A140: step 1 no longer files a mail the AI reads).
  flow-switches              each card's switch flips its card, and flips back.
  flow-exit-needs-fallback   the exit switch on with no destination is refused in words (the sentence, on the card, Save closed);
                             picking Primary lifts it.
  flow-fact-states           each of the eight facts goes to yes, no and back to any, and no other fact moves.
  flow-rows-and-chains       add and remove a rule row, set its kind and value; « Autre règle » opens a second chain with its own
                             grid, mode and rows, and its remove button closes it.
  flow-every-rule-row-has-its-remove   three rows added: each draws its remove control at rest; the second and third are removed by their own doors; the saved rule set keeps the first.
  flow-ai-reads              each of the four ticks flips (Objet, Corps, Reste du fil, Pièces jointes); the three that read the message off is refused
                             in words on the AI card and closes Save, and the thread tick on its own does not lift it.
  flow-ai-refusal-shown-and-opens-decisions
                             while the AI cannot sort, the AI card and the empty list say why in the core's words, and the control opens Decisions.
  flow-ai-thread-saved       « Reste du fil » on, Create: the core reads the tick back (label/<id>/flow/ai_reads) and a Save that unticks it writes it too.
  flow-fallback-picker       Primary, Spam and a label each become « Les autres vont dans », shown by name.
  flow-why-here              « Pourquoi ici ? » opens over the foot and says, for the message in focus, which card placed it.
  flow-save-reads-back       Create writes the cards and the chains: the core reads back the switch, the fallback, the exit chain and
                             both entry chains; a flow change then asks the Re-run question, and Re-run answers it.
"""
import json
import os
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable  # noqa: E402
from lib.fixture import MSG, FX  # noqa: E402
sys.path.insert(0, os.path.join(ROOT, "checks", "checklib"))
import resetguard as RG  # noqa: E402

AX = "mac-labels-contacts"


def labels(c):
    return c.rows(AX + ".labels")


def c_nest(c):
    c.send("click", AX + ".open")
    top = [r for r in labels(c) if r["depth"] == 1]
    if len(top) < 2:
        raise Unreadable("fewer than two top-level labels to nest")
    a, b = top[-1], top[0]
    path = "inplace"
    try:
        c.send("drag", AX + ".labels." + a["id"], AX + ".labels." + b["id"])
        if not [r for r in labels(c) if r["id"] == a["id"] and r["parent"] == b["id"]]:
            raise Unreadable("drag delivered but the row did not move")
    except (Unreadable, Exception):
        path = "click ids"
        c.send("click", AX + ".labels.select." + a["id"])
        c.send("click", AX + ".labels.drop." + b["id"])
    now = [r for r in labels(c) if r["id"] == a["id"]]
    ok = bool(now) and now[0]["parent"] == b["id"]
    c.send("click", AX + ".labels.select." + a["id"])
    c.send("click", AX + ".labels.drop.top")
    return ok, "%s -> %s by %s, path %r" % (a["name"], b["name"], path, now[0]["path"] if now else None)


def c_depth(c):
    c.send("click", AX + ".open")
    deep = max(labels(c), key=lambda r: r["depth"])
    row = deep
    for n in range(row["depth"], 5):
        c.send("click", AX + ".labels.select." + row["id"])
        c.send("type", "L%d" % n)
        c.send("click", AX + ".labels.create")
        row = max(labels(c), key=lambda r: r["depth"])
    c.send("click", AX + ".labels.select." + row["id"])
    c.send("type", "Sixth")
    try:
        c.send("click", AX + ".labels.create")
    except Exception:
        pass
    w = [r["words"] for r in c.rows(AX + ".labels.refusal")]
    return any("levels" in x for x in w) and not [r for r in labels(c) if r["name"] == "Sixth"], "words %r" % w


def c_duplicate(c):
    c.send("click", AX + ".open")
    for _ in range(2):
        c.send("click", AX + ".folder.new")
        c.send("type", "Parity folder")
        c.rows(AX + ".rule.matches")
        c.send("type", "linea")
        c.rows(AX + ".rule.matches")
        try:
            c.send("click", AX + ".rule.save")
        except Exception:
            pass
    w = [r["words"] for r in c.rows(AX + ".rule.refusal")]
    c.send("click", AX + ".rule.cancel")
    return any("exists already" in x for x in w), "words %r" % w



def c_typing_after_editor(c):
    """typing-lost: the smart-folder editor's window stays alive after the folder is saved; text typed in the add-account sheet
    afterwards must land in its address field, not in the closed editor's rule."""
    c.send("click", AX + ".open")
    c.send("click", AX + ".folder.new")
    c.send("type", "Typing folder")
    c.send("type", "supplier")
    c.send("click", AX + ".rule.save")
    c.send("key", "Escape")
    for ax in ("dock.whoosh", "mac-main.sidebar.accounts-menu.up", "mac-main.sidebar.accounts-menu.add", "mac-add-account.address"):
        c.send("click", ax)
    c.send("type", "probe@nothing.example.test")
    rows = c.rows("mac-add-account.address")
    text = rows[0].get("text") if rows else None
    c.send("key", "Escape")
    return text == "probe@nothing.example.test", "the address field holds %r" % (text,)


# ---------------------------------------------------------------- round 2: the label inspector
INS = "mac-labels.inspector"
PLUS = "mac-main.sidebar.labels.add"


def summary(c):
    rows = c.rows(INS)
    if not rows:
        raise Unreadable("no row in region %s" % INS)
    return rows[0]


def settle(c, tries=60):
    """The count is read off the main thread: wait until the inspector says it is not counting."""
    for _ in range(tries):
        s = summary(c)
        if not s.get("counting"):
            return s
        time.sleep(0.1)
    raise Unreadable("the count never settled")


def say(c, ax, text=None):
    if text is not None:
        c.send("type", text)
    return c.send("click", ax)


def reset(c):
    try:
        c.send("key", "Escape")
        if summary(c).get("open"):
            c.send("key", "Escape")
    except Unreadable:
        pass


def i_open(c):
    reset(c)
    say(c, PLUS)
    s = summary(c)
    return bool(s.get("open")) and s.get("mode") == "new", "open=%s mode=%s" % (s.get("open"), s.get("mode"))


def i_live(c):
    reset(c)
    say(c, INS + ".open.new")
    say(c, INS + ".name", "Parity suppliers")
    say(c, INS + ".rule.0.kind.domain")
    say(c, INS + ".rule.0.value", "linea-supply-example.com")
    s = settle(c)
    rules = c.rows(INS + ".rules")
    rows = c.rows(INS + ".matches")
    domain_ok = s["count"] > 0 and rules and rules[0]["count"] == s["count"] and len(rows) == s["matches"] and \
        all("Domain" in r["tag"] for r in rows)
    first = s["count"]
    say(c, INS + ".rule.add")
    say(c, INS + ".rule.1.kind.subject")
    say(c, INS + ".rule.1.value", "quote")
    s2 = settle(c)
    rows2 = c.rows(INS + ".matches")
    subj_ok = any("Subject" in r["tag"] for r in rows2) and s2["count"] >= first
    say(c, INS + ".switch.mailbox")
    mailbox = summary(c).get("showing") == "mailbox"
    say(c, INS + ".switch.matches")
    back = summary(c).get("showing") == "matches"
    return domain_ok and subj_ok and mailbox and back, \
        "count %s then %s, tags %s, mailbox %s, back %s" % (first, s2["count"], sorted({r["tag"] for r in rows2}), mailbox, back)


def i_fact_tag(c):
    """A message caught only by a fact of the grid (Favori) is listed with the fact's name as its tag, and the rows are the core's count."""
    reset(c)
    ids = [r["id"] for r in c.rows(LIST)]
    starred = None
    if ids:   # the fixture core holds no starred mail and its corpus has no star field: `fixture star <id> on` stars the first listed one (reset clears it; the real core refuses the verb)
        try:
            c.send("fixture", "star", ids[0], "on")
            starred = ids[0]
        except Unreadable:
            starred = None
    try:
        say(c, INS + ".open.new")
        say(c, INS + ".name", "Parity starred")
        say(c, INS + ".entry.fact.starred.yes")
        s = settle(c)
        rows = c.rows(INS + ".matches")
    finally:
        if starred:
            c.send("fixture", "star", starred, "off")
    if not rows:
        raise Unreadable("no starred message in this corpus, so no row is caught by Favori | the count the inspector says: %r, rows %d; staged: %s (`fixture star <id> on` on the first of %d listed mails)"
                         % (s.get("count"), len(rows), starred or "no, the verb was refused or no mail is listed", len(ids)))
    tagged = all("starred" in r["tag"].lower() for r in rows)
    return s["count"] > 0 and len(rows) == s["matches"] and tagged, \
        "starred %s, count %s, rows %d, tags %s" % (starred, s["count"], len(rows), sorted({r["tag"] for r in rows}))


def i_create(c):
    tree_before = c.core("labels/tree")
    s = summary(c)
    if s.get("mode") != "new" or not s.get("can_write"):
        raise Unreadable("no new label ready to create (can_write=%s)" % s.get("can_write"))
    say(c, INS + ".create")
    s = summary(c)
    tree = c.core("labels/tree")
    inside = "Parity suppliers" in json.dumps(tree) and "Parity suppliers" not in json.dumps(tree_before)
    return inside and s.get("mode") == "modify" and bool(s.get("label_id")), \
        "in tree %s, mode %s, id %s" % (inside, s.get("mode"), s.get("label_id"))


def i_modify(c):
    s = summary(c)
    lid = s.get("label_id")
    if not lid:
        raise Unreadable("no saved label open")
    reset(c)
    say(c, INS + ".open.modify." + lid)
    s = settle(c)
    say(c, INS + ".match.any")   # the saved rule is a domain; with "all" a second domain rule matches nothing, so the preview would hold no row
    n = len(c.rows(INS + ".rules"))
    say(c, INS + ".rule.add")
    say(c, INS + ".rule.%d.kind.domain" % n)
    say(c, INS + ".rule.%d.value" % n, "provanta-example.be")
    s = settle(c)
    rows = c.rows(INS + ".matches")
    edges = [r["edge"] for r in rows]
    preview = s.get("preview") and all(e in ("join", "leave", "stay") for e in edges)
    # contract delta 12: a filed message no rule matches STAYS when the label has a meaning; it leaves only without one
    if s.get("meaning"):
        preview = preview and "leave" not in edges
    say(c, INS + ".apply")
    s = summary(c)
    offer = s.get("offer")
    asked = bool(offer) and offer["filed"] >= 0
    joins = [r["id"] for r in rows if r["edge"] == "join"]
    say(c, INS + ".rerun")
    res = until(lambda: c.rows(INS + ".result"), tries=60)      # the core judges in the background: the result lands when it ends
    done = bool(res) and summary(c).get("offer") is None
    by = {r["id"]: r.get("decided_by") for r in res if r.get("id")}
    decided = bool(joins) and all(by.get(j) for j in joins)
    try:
        state_ok = bool(joins) and bool(c.core("message/%s/label/%s/decided_by" % (joins[0], lid)))
    except Unreadable:
        state_ok = False   # the core holds no decided_by for a message the re-run should have filed
    return bool(preview) and asked and done and decided and state_ok, \
        "edges %s, offer %s, joined %s, decided_by %s, core state %s" % (sorted(set(edges)), offer, joins, [by.get(j) for j in joins][:3], state_ok)


LIST = "mac-main.list"


def in_board(c, mid):
    """True when the message is filed under the board's label. `label_assignment/<m>` lists the labels' NAMES on the test engine (FixtureCore.swift:1928), not their ids:
    the label's own name is read, and the id is accepted too (the stand-in lists ids)."""
    try:
        name = c.core("label/%s/name" % BOARD)
    except Unreadable:
        name = None
    got = assigned(c, mid)
    if BOARD in got or (name is not None and name in got):
        return True
    # the board's label is a BUILT-IN category on the test engine: a message under it is held in `category` and `filedBy` (FixtureLabels.swift:275-280), never in the
    # `assigned` list that `label_assignment` reads, so the filing's own read is the proof (main95: the kept mail's label_assignment was [] while its counts said kept)
    try:
        return bool(c.core("message/%s/label/%s/decided_by" % (mid, BOARD)))
    except Unreadable:
        return False


def assigned(c, mid):
    return c.core("label_assignment/%s" % mid) or []


def fixture_reset(c):
    """A key case starts from the launch state: `fixture reset`, then the digest must equal the launch digest (else the case is NOT JUDGED)."""
    RG.reset(c)


def focus_on(c, mid):
    """Focus the row that says what the fixture message `mid` says (subject, sender) in the served list; the returned ids are the served ids, in order.
    The id the served row carries replaces the literal: `mid` is the fixture's name for the message, not a promise about the list."""
    rows = c.rows(LIST)
    ids = [r["id"] for r in rows]
    at = RG.row_index(rows, MSG[mid]) if mid in MSG else None
    if at is None and mid in ids:
        at = ids.index(mid)
    if at is None:
        raise Unreadable("%s (%s) is not in the list" % (mid, MSG.get(mid, {}).get("subject")))
    c.send("click", "%s.row.%d" % (LIST, at))
    return ids


def give(c, mid, *names):
    """Labels a thread through the list's own L picker, so the keys have something to take off."""
    for n in names:
        focus_on(c, mid)
        c.send("key", "L")
        c.send("type", n)
        c.send("key", "Enter")
    have = assigned(c, mid)
    if not all(n in have for n in names):
        raise Unreadable("L did not label %s with %s (has %s)" % (mid, names, have))


def k_remove(chord, step):
    def run(c):
        fixture_reset(c)
        give(c, "msg-04", "Suppliers")
        ids = focus_on(c, "msg-04")
        c.send("key", chord)
        now, focus = assigned(c, "msg-04"), c.uistate()["focus_message"]
        want = ids[ids.index("msg-04") + step] if 0 <= ids.index("msg-04") + step < len(ids) else "msg-04"
        return "Suppliers" not in now and focus == want, "%s: labels now %s, focus %s (want %s)" % (chord, now, focus, want)
    return run


def k_all(c):
    fixture_reset(c)
    give(c, "msg-04", "Suppliers", "Tenders")
    focus_on(c, "msg-04")
    c.send("key", "⇧Y")
    now = assigned(c, "msg-04")
    return now == [], "labels now %s" % now


def k_move(c):
    fixture_reset(c)
    focus_on(c, "msg-08")
    c.send("key", "V")
    overlay = c.uistate().get("overlay")
    c.send("type", "Supp")
    c.send("click", "mac-labels.move.filter")
    targets = c.rows("mac-labels.move.targets")
    narrowed = bool(targets) and all("supp" in (t["name"] + " " + t.get("path", "")).lower() for t in targets)
    drawn = False
    try:
        drawn = c.send("measure", "mac-labels.move.picker").get("frame", {}).get("h", 0) > 0
    except Unreadable:
        pass
    pick = next((t for t in targets if t["name"] == "Suppliers"), None)
    if pick:
        c.send("click", "mac-labels.move." + pick["id"])
    moved = "Suppliers" in assigned(c, "msg-08") and c.core("message/msg-08/archived") is True
    if moved:
        c.send("key", "Z")
    return overlay == "move" and drawn and narrowed and moved, \
        "overlay %s, picker drawn %s, typed Supp narrows to %s, labelled+archived %s" % (overlay, drawn, [t["name"] for t in targets], moved)


def k_pin(c):
    fixture_reset(c)
    focus_on(c, "msg-04")
    before = bool(c.core("pin/msg-04"))
    c.send("key", "⇧P")
    flipped = bool(c.core("pin/msg-04"))
    c.send("key", "⇧P")
    back = bool(c.core("pin/msg-04"))
    return flipped != before and back == before, "pin %s -> %s -> %s" % (before, flipped, back)


KEYS = [("key-y-removes-the-label", k_remove("Y", 0), "y-not-sent"), ("key-open-bracket-removes-and-steps-next", k_remove("[", 1), "next-no-step"),
        ("key-close-bracket-removes-and-steps-back", k_remove("]", -1), "prev-no-step"), ("key-shift-y-removes-all", k_all, "all-keeps-one"),
        ("key-v-moves-to-a-label", k_move, "picker-not-drawn"), ("key-v-picker-narrows-by-words", k_move, "filter-ignored"), ("key-shift-p-pins", k_pin, "pin-ignored")]


class FakeKeys:
    """The list on the wire for the key checks: focus, the core's assignments, archive and pin; a mutant breaks one key."""
    def __init__(self, mutant=None):
        RG.LAUNCH.clear()
        self.m, self.ids, self.f, self.typed, self.overlay = mutant, ["msg-03", "msg-04", "msg-05", "msg-08"], 0, "", None
        self.lab, self.arch, self.pin, self.undo, self.query = {}, set(), set(), [], ""

    def uistate(self):
        return {"focus_message": self.ids[self.f], "overlay": self.overlay}

    def core(self, path):
        k, mid = path.split("/")[0], path.split("/")[1]
        return {"label_assignment": list(self.lab.get(mid, [])), "message": mid in self.arch, "pin": mid in self.pin}[k]

    def rows(self, ax):
        if ax == LIST:
            return [{"id": i} for i in self.ids]
        if ax == "mac-labels.move.targets" and self.overlay == "move":
            every = [{"id": "lbl-sup", "name": "Suppliers", "path": "Work"}, {"id": "lbl-fam", "name": "Family", "path": "Personal"}]
            q = "" if self.m == "filter-ignored" else self.query.lower()
            return [t for t in every if q in t["name"].lower()]
        return []

    def send(self, verb, *a):
        mid = self.ids[self.f]
        if verb == "fixture" and a == ("digest",):
            st = (sorted((k, tuple(v)) for k, v in self.lab.items()), sorted(self.arch), sorted(self.pin))
            return {"digest": repr(st), "parts": {"labels": repr(st[0]), "archived": repr(st[1]), "pinned": repr(st[2])}, "view": "v"}
        if verb == "fixture" and a == ("reset",):
            self.lab, self.arch, self.pin, self.undo, self.f, self.query, self.typed, self.overlay = {}, set(), set(), [], 0, "", "", None
            return {}
        if verb == "click" and a[0].startswith(LIST + ".row."):
            self.f = int(a[0].split(".")[-1])
        elif verb == "click" and a[0] == "mac-labels.move.lbl-sup" and self.overlay == "move":
            self.lab.setdefault(mid, []).append("Suppliers"); self.arch.add(mid); self.overlay = None; self.undo.append(mid)
        elif verb == "click" and a[0] == "mac-labels.move.filter":
            self.query, self.typed = self.typed, ""
        elif verb == "click" and a[0] == "mac-labels.move.close" and self.overlay == "move":
            if self.m != "close-door-dead":
                self.overlay = None
        elif verb == "measure":
            return {"frame": {"h": 0 if self.m == "picker-not-drawn" else 240}}
        elif verb == "type":
            self.typed = a[0]
        elif verb == "key":
            k = a[0]
            if k == "Enter" and self.typed:
                self.lab.setdefault(mid, []).append(self.typed); self.typed = ""
            elif k in ("Y", "[", "]") and self.lab.get(mid):
                if not (self.m == "y-not-sent" and k == "Y"):
                    self.lab[mid].pop(0)
                step = {"Y": 0, "[": 1, "]": -1}[k]
                if not ((self.m == "next-no-step" and k == "[") or (self.m == "prev-no-step" and k == "]")):
                    self.f = min(max(0, self.f + step), len(self.ids) - 1)
            elif k == "⇧Y":
                self.lab[mid] = self.lab.get(mid, [])[-1:] if self.m == "all-keeps-one" else []
            elif k == "V":
                self.overlay = "move"
            elif k == "Z" and self.undo:
                u = self.undo.pop(); self.lab[u].remove("Suppliers"); self.arch.discard(u)
            elif k == "⇧P" and self.m != "pin-ignored":
                self.pin.symmetric_difference_update({mid})
        return {}


SIZES = ((1280, 820), (960, 632))
EPS = 0.5


def _inside(a, b):
    return a["x"] >= b["x"] - EPS and a["y"] >= b["y"] - EPS and a["x"] + a["w"] <= b["x"] + b["w"] + EPS and a["y"] + a["h"] <= b["y"] + b["h"] + EPS


def _cross(a, b):
    return a["x"] + EPS < b["x"] + b["w"] and b["x"] + EPS < a["x"] + a["w"] and a["y"] + EPS < b["y"] + b["h"] and b["y"] + EPS < a["y"] + a["h"]


SCROLLED = ("free.drop", "free.entry.switch", "free.ai.switch", "free.exit.switch", "free.ai.refusal", "free.exit.refusal")


def layout_faults(geo, fit):
    """Every way the measured inspector breaks the layout rule; [] when it holds. Pure: the selftest feeds it too.
    The app sends the token it promises (row kind "tokens": the smallest inset a control keeps from its card's edge)."""
    short = lambda i: i.split("geo.", 1)[-1]
    tok = next((g for g in geo if g["kind"] == "tokens"), None)
    cards = {g["card"]: g for g in geo if g["kind"] == "card"}
    ctls = [g for g in geo if g["kind"] in ("ctl", "free")]
    rows = [g for g in geo if g["kind"] == "row"]
    clip = next((g for g in geo if g["kind"] == "clip"), None)
    bad = []
    if tok is None or len(cards) < 4 or len(ctls) < 8 or not fit:
        return ["too little measured: tokens %s, %d cards, %d controls, %d fitted rows" % (bool(tok), len(cards), len(ctls), len(fit))]
    inset = tok["inset"]
    # no two DRAWN controls intersect: a scrolled frame (in a card, a switch, the drop line, a refusal) counts only where the scroll
    # view shows it; what scrolls under the foot is clipped by the scroll view, never drawn over it
    def drawn(g):
        if clip is None or not (g.get("card") or short(g["id"]) in SCROLLED):
            return g
        x0, y0 = max(g["x"], clip["x"]), max(g["y"], clip["y"])
        x1, y1 = min(g["x"] + g["w"], clip["x"] + clip["w"]), min(g["y"] + g["h"], clip["y"] + clip["h"])
        return dict(g, x=x0, y=y0, w=x1 - x0, h=y1 - y0) if x1 > x0 and y1 > y0 else None
    seen = [d for d in (drawn(g) for g in ctls) if d]
    for i, a in enumerate(seen):
        for b in seen[i + 1:]:
            if _cross(a, b):
                bad.append("overlap %s / %s" % (short(a["id"]), short(b["id"])))
    for g in rows:                                                        # a row lies inside its card
        if g["card"] not in cards or not _inside(g, cards[g["card"]]):
            bad.append("row outside its card: %s" % short(g["id"]))
    for g in ctls:                                                        # a control keeps the card's inset on both sides
        k = cards.get(g.get("card"))
        if g.get("card") and (k is None or g["x"] + EPS < k["x"] + inset or g["x"] + g["w"] > k["x"] + k["w"] - inset + EPS
                              or not _inside(g, k)):
            bad.append("control outside its card's inset: %s" % short(g["id"]))
    names = sorted(cards)
    for i, a in enumerate(names):                                         # cards of one column: one x, one width
        for b in names[i + 1:]:
            p, q = cards[a], cards[b]
            if p["x"] < q["x"] + q["w"] and q["x"] < p["x"] + p["w"] and (abs(p["x"] - q["x"]) > EPS or abs(p["w"] - q["w"]) > EPS):
                bad.append("cards %s and %s share a column but not its edges (x %.1f/%.1f, w %.1f/%.1f)" % (a, b, p["x"], q["x"], p["w"], q["w"]))
    ed = next((g for g in ctls if short(g["id"]) == "ctl.ai.editor"), None)
    if ed is None or "ai" not in cards or ed["w"] + EPS < cards["ai"]["w"] - 2 * (inset + 8):
        bad.append("description narrower than its column")
    closes = [short(g["id"]) for g in ctls if ".free.close." in g["id"]]
    if len(closes) != 1:
        bad.append("%d Close controls: %s" % (len(closes), closes))
    columns = {}
    for r in rows:
        if r["card"] in cards:
            columns.setdefault(round(cards[r["card"]]["x"]), set()).add(round(r["x"], 1))
    bad += ["label column not aligned at card x %s: %s" % (x, sorted(xs)) for x, xs in columns.items() if len(xs) > 1]
    bad += ["label narrower than its longest word: %s %.0f < %.0f" % (f["title"], f["label_width"], f["longest_word"])
            for f in fit if f["label_width"] + EPS < f["longest_word"]]
    return bad


def i_geometry(c):
    """The modify path (the label i_create made, with its rules) and the create path, both sizes, both themes."""
    lid = summary(c).get("label_id")
    if not lid:
        raise Unreadable("no saved label to open on the modify path")
    shots = os.environ.get("WHOOSH_R3_SHOTS")
    faults, states = [], 0
    for lang, theme in (("en", "light"), ("en", "dark"), ("fr", "light"), ("fr", "dark")):   # the owner saw it in French
        c.send("click", "mac-settings-general.appearance.language." + lang)
        c.send("theme", theme)
        for w, h in SIZES:
            c.send("resize", str(w), str(h))
            for path in ("modify", "new"):
                reset(c)
                if path == "modify":
                    say(c, INS + ".open.modify." + lid)
                else:
                    say(c, INS + ".open.new")
                    say(c, INS + ".rule.0.value", "linea-supply-example.com")
                settle(c)
                time.sleep(0.5)
                found = layout_faults(c.rows(INS + ".geometry"), c.rows(INS + ".rowfit"))
                faults += ["%s %s %dx%d %s: %s" % (lang, theme, w, h, path, f) for f in found]
                states += 1
                if shots:
                    r = c.send("shot", "labels-r3-%s-%s-%s-%dx%d" % (path, lang, theme, w, h))
                    if isinstance(r, dict) and r.get("path"):
                        import shutil
                        shutil.copy(r["path"], os.path.join(shots, os.path.basename(r["path"])))
    c.send("click", "mac-settings-general.appearance.language.en")
    c.send("theme", "light")
    reset(c)
    say(c, INS + ".open.modify." + lid)
    kinds = {}
    for f in faults:
        k = f.split(": ", 1)[-1]
        kinds[k] = kinds.get(k, 0) + 1
    return not faults and states == 16, "%d states, %d faults; %s" % (states, len(faults), "; ".join("%s (x%d)" % kv for kv in sorted(kinds.items())) or "no fault")


def i_esc(c):
    reset(c)
    c.send("key", "⌘I")
    opened = summary(c).get("open")
    c.send("key", "⌘I")
    closed = not summary(c).get("open")
    c.send("key", "⌘I")
    c.send("key", "Escape")
    esc = not summary(c).get("open")
    return bool(opened) and closed and esc, "cmd-I open %s, cmd-I close %s, Esc close %s" % (opened, closed, esc)


def i_controls(c):
    """Every control once, each with the outcome it must show. A control with no outcome is a finding."""
    reset(c)
    say(c, INS + ".open.new")
    seen, bad = [], []

    def step(name, ax, text, test):
        try:
            say(c, ax, text)
            ok = bool(test(settle(c)))
        except Unreadable as e:
            ok = False
            name = "%s (%s)" % (name, e)
        (seen if ok else bad).append(name)

    labels = [r for r in c.rows("mac-labels-contacts.labels") if r.get("depth") == 1 and str(r.get("id", "")).startswith("lbl-")]
    step("name", INS + ".name", "Controls", lambda s: s["name"] == "Controls")
    step("meaning", INS + ".meaning", "supplier quotes", lambda s: s["meaning"] == "supplier quotes")
    step("pinned", INS + ".pinned", None, lambda s: s["pinned"] is True)
    for i in (0, 3, 5):
        step("colour.%d" % i, "%s.colour.%d" % (INS, i), None, lambda s, i=i: s["colour"] is not None)
    step("colour.none", INS + ".colour.none", None, lambda s: s["colour"] is None)
    if labels:
        step("inside", "%s.inside.%s" % (INS, labels[0]["id"]), None, lambda s: s["parent"] == labels[0]["id"])
    step("inside.top", INS + ".inside.top", None, lambda s: s["parent"] is None)
    step("match.all", INS + ".match.all", None, lambda s: s["match"] == "all")
    step("match.any", INS + ".match.any", None, lambda s: s["match"] == "any")
    step("rule.0.kind.domain", INS + ".rule.0.kind.domain", None, lambda s: c.rows(INS + ".rules")[0]["kind"] == "domain")
    step("rule.0.value", INS + ".rule.0.value", "linea-supply-example.com", lambda s: s["count"] > 0)
    step("rule.add", INS + ".rule.add", None, lambda s: s["rules"] == 3)
    for k in ("from", "subject", "body", "account", "to", "cc", "listHeader", "domain"):
        step("rule.2.kind." + k, "%s.rule.2.kind.%s" % (INS, k), None,
             lambda s, k=k: c.rows(INS + ".rules")[2]["kind"] == k)
    step("rule.0.commit", INS + ".rule.0.commit", None, lambda s: c.rows(INS + ".rules")[0]["values"] == ["linea-supply-example.com"])
    step("rule.2.remove", INS + ".rule.2.remove", None, lambda s: s["rules"] == 2)
    step("drop", INS + ".drop", None, lambda s: s["rules"] >= 1)
    say(c, INS + ".proof.hold")
    held = summary(c).get("counting") is True
    say(c, INS + ".proof.hold")
    (seen if held and not settle(c).get("counting") else bad).append("proof.hold")
    step("switch.mailbox", INS + ".switch.mailbox", None, lambda s: s["showing"] == "mailbox")
    step("switch.matches", INS + ".switch.matches", None, lambda s: s["showing"] == "matches")
    step("toolbar", INS + ".toolbar", None, lambda s: s["open"] is False)
    say(c, INS + ".open.new")
    step("cancel", INS + ".cancel", None, lambda s: s["open"] is False)
    return not bad, "clicked %d controls, %d with no outcome%s" % (len(seen) + len(bad), len(bad), (": " + ", ".join(bad)) if bad else "")


INSPECTOR = [("inspector-opens-from-plus", i_open, "opens-nothing"), ("rule-typed-live-count", i_live, "count-stuck"),
             ("create-reads-in-tree", i_create, "create-silent"), ("modify-preview-and-rerun", i_modify, "rerun-no-decided-by"),
             ("inspector-layout-holds", i_geometry, "stacking-removed"),
             ("esc-returns-the-reader", i_esc, "esc-ignored"), ("every-control-clicked", i_controls, "dead-control"),
             ("fact-only-row-carries-the-fact-tag", i_fact_tag, "fact-tag-empty")]

CHECKS = [("drop-nests-a-label", c_nest, "move-dropped"), ("depth-refusal-in-words", c_depth, "refusal-silent"),
          ("duplicate-folder-refused", c_duplicate, "duplicate-accepted"),
          ("typing-after-the-editor-closes-reaches-the-next-field", c_typing_after_editor, "closed-editor-keeps-taking-the-typed-text")]


class Fake:
    """The screen on the wire, for --selftest. A mutant breaks exactly the behaviour its check names."""
    def __init__(self, mutant=None):
        self.m = mutant
        self.l = [{"id": "a", "name": "Work", "path": "Work", "depth": 1, "parent": None},
                  {"id": "b", "name": "Home", "path": "Home", "depth": 1, "parent": None}]
        self.sel, self.refusal, self.folders, self.saves = None, [], 0, 0
        self.editor, self.rule, self.sheet, self.addr_focus, self.addr = False, "", False, False, ""

    def send(self, verb, *a):
        if verb == "click" and a[0].endswith(".folder.new"):
            self.editor = True
        elif verb == "click" and a[0] == "mac-main.sidebar.accounts-menu.add":
            self.sheet = True
        elif verb == "click" and a[0] == "mac-add-account.address":
            self.addr_focus = self.sheet
        elif verb == "type" and self.sheet and self.addr_focus:
            if self.editor and self.m == "closed-editor-keeps-taking-the-typed-text":
                self.rule += a[0]        # the closed editor's view still takes the shared buffer
            else:
                self.addr += a[0]
            return {}
        elif verb == "key" and a[0] == "Escape" and self.sheet:
            self.sheet = self.addr_focus = False
        if verb == "drag":
            if self.m != "move-dropped":
                for r in self.l:
                    if r["id"] == a[0].split(".")[-1]:
                        r["parent"] = a[1].split(".")[-1]; r["path"] = "Work > Home"; r["depth"] = 2
        elif verb == "click" and ".labels.select." in a[0]:
            self.sel = a[0].split(".")[-1]
        elif verb == "click" and a[0].endswith(".labels.create"):
            d = max(r["depth"] for r in self.l if r["id"] == self.sel)
            if d >= 5:
                self.refusal = [] if self.m == "refusal-silent" else ["a label can be at most 5 levels deep"]
                if self.m == "refusal-silent":
                    self.l.append({"id": "x", "name": "Sixth", "path": "", "depth": 6, "parent": self.sel})
            else:
                self.l.append({"id": "n%d" % len(self.l), "name": "L%d" % d, "path": "", "depth": d + 1, "parent": self.sel})
        elif verb == "click" and a[0].endswith(".rule.save"):
            self.saves += 1
            if self.saves > 1:
                self.refusal = [] if self.m == "duplicate-accepted" else ["the name exists already"]
        return {}

    def rows(self, ax):
        if ax == "mac-add-account.address":
            return [{"ax": ax, "focused": self.addr_focus, "text": self.addr}] if self.sheet else []
        if ax.endswith(".labels.refusal") or ax.endswith(".rule.refusal"):
            return [{"words": w} for w in self.refusal]
        if ax.endswith(".labels"):
            return list(self.l)
        return []

class FakeInspector:
    """The inspector on the wire, for --selftest: a three-message corpus and the label model. A mutant breaks exactly the
    behaviour its check names (INSPECTOR rows)."""
    CORPUS = [("m1", "linea-supply-example.com", "Quote for the Q1 print run"), ("m2", "linea-supply-example.com", "Invoice 2026-11"),
              ("m3", "other.example", "Re: Quote"), ("m4", "provanta-example.be", "Clarification")]

    def __init__(self, mutant=None):
        self.m = mutant
        self.typed = ""
        self.tree = [{"id": "work", "name": "Work", "depth": 1, "parent": None}]
        self.reset_draft()
        self.open, self.mode, self.lid, self.offer, self.result, self.showing = False, "closed", None, None, [], "matches"
        self.saved_rules, self.filed = [], {"m1"}
        self.starred_ids = set()

    def reset_draft(self):
        self.d = {"name": "", "parent": None, "colour": None, "pinned": False, "meaning": "", "match": "any",
                  "facts": {}, "rules": [{"kind": "from", "values": [], "pending": ""}, {"kind": "subject", "values": [], "pending": ""}]}

    # the messages the stand-in's core knows to be starred: none at launch (the fixture core holds none), `fixture star <id> on` adds one; a row does not carry the star
    @property
    def STARRED(self):
        return self.starred_ids

    def conds(self, rules):
        return [(r["kind"], v) for r in rules for v in r["values"] + ([r["pending"]] if r["pending"] else [])]

    def hit(self, c, msg):
        k, v = c
        if k == "fact:starred":
            return msg[0] in self.STARRED
        return (k == "domain" and msg[1] == v) or (k == "subject" and v.lower() in msg[2].lower())

    def matches(self, rules):
        cs = self.conds(rules) + self.fact_conds()
        return [m for m in self.CORPUS if any(self.hit(c, m) for c in cs)]

    def fact_conds(self):
        return [("fact:" + k, "yes") for k, st in self.d["facts"].items() if st == "yes"]

    def send(self, verb, *a):
        if verb == "fixture" and a[:1] == ("star",):
            if len(a) != 3 or a[2] not in ("on", "off") or a[1] not in [m[0] for m in self.CORPUS]:
                raise Unreadable("fixture star: refused %r" % (a,))
            (self.starred_ids.add if a[2] == "on" else self.starred_ids.discard)(a[1])
            return {}
        if verb == "type":
            self.typed = a[0]
        elif verb == "key":
            if a[0] == "⌘I":
                self.open, self.mode = (not self.open), ("new" if not self.open else "closed")
                if self.open:
                    self.reset_draft()
            elif a[0] == "Escape" and self.m != "esc-ignored":
                if self.offer:
                    self.offer = None
                else:
                    self.open, self.mode = False, "closed"
        elif verb == "click":
            self.click(a[0])
        return {}

    def geometry(self):
        """A clean one-column inspector of four cards; the mutant removes the stacking, so the Applies-to control runs into its label."""
        g = [{"id": "geo.tokens", "kind": "tokens", "card": None, "inset": 12},
             {"id": "geo.clip.scroll", "kind": "clip", "card": None, "x": 0, "y": 80, "w": 700, "h": 520},
             {"id": "geo.free.toggle", "kind": "free", "card": None, "x": 640, "y": 20, "w": 32, "h": 28}]
        g += [{"id": "geo.card." + n, "kind": "card", "card": n, "x": 32, "y": y, "w": 636, "h": hh}
              for n, y, hh in (("entry", 100, 220), ("ai", 340, 100), ("exit", 460, 140), ("label", 620, 200))]
        def ctl(card, n, x, y, w=120):
            return {"id": "geo.ctl.%s.%s" % (card, n), "kind": "ctl", "card": card, "x": x, "y": y, "w": w, "h": 24}
        for i, n in enumerate(("name", "inside", "colour", "scope", "pinned")):
            g.append({"id": "geo.row.label." + n, "kind": "row", "card": "label", "x": 32, "y": 620 + 36 * i, "w": 636, "h": 36})
            g.append(ctl("label", n, 520, 626 + 36 * i))
        g += [ctl("ai", "editor", 52, 372, 596), ctl("entry", "match", 150, 108), ctl("entry", "add", 60, 296), ctl("exit", "fallback", 200, 560),
              ctl("entry", "fact.unread", 60, 150, 200), ctl("exit", "fact.unanswered", 60, 480, 200),
              {"id": "geo.row.ai.reads", "kind": "row", "card": "ai", "x": 32, "y": 340, "w": 636, "h": 36},
              {"id": "geo.free.entry.switch", "kind": "free", "card": None, "x": 600, "y": 72, "w": 40, "h": 20},
              {"id": "geo.free.drop", "kind": "free", "card": None, "x": 32, "y": 322, "w": 400, "h": 16},
              {"id": "geo.free.close.cancel", "kind": "free", "card": None, "x": 500, "y": 610, "w": 80, "h": 28},
              {"id": "geo.ctl.entry.r9.kind", "kind": "ctl", "card": "entry", "x": 400, "y": 250, "w": 120, "h": 40}]
        if self.m == "stacking-removed":
            g.append(ctl("label", "scope-pill", 400, 698, 140))
        return g

    def rowfit(self):
        narrow = self.m == "stacking-removed"
        return [{"title": t, "label_width": 40 if narrow and t == "Applies to" else 128, "longest_word": 60} for t in ("Name", "Applies to")]

    def click(self, ax):
        t, self.typed = self.typed, ""
        tail = ax.split(INS + ".")[-1] if INS in ax else ax
        d = self.d
        if ax == PLUS or tail == "open.new":
            if not (ax == PLUS and self.m == "opens-nothing"):
                self.open, self.mode, self.lid = True, "new", None
                self.reset_draft()
        elif tail.startswith("open.modify."):
            self.open, self.mode, self.lid = True, "modify", tail.split(".")[-1]
            d.update(name="Work", rules=[{"kind": "domain", "values": ["linea-supply-example.com"], "pending": ""}])
            self.saved_rules = [dict(r) for r in d["rules"]]
        elif tail == "name" and t:
            d["name"] = t
        elif tail == "meaning":
            d["meaning"] = t
        elif tail == "pinned" and self.m != "dead-control":
            d["pinned"] = not d["pinned"]
        elif tail.startswith("colour."):
            n = tail.split(".")[-1]
            d["colour"] = None if n == "none" else "#00000%s" % n
        elif tail == "inside.top":
            d["parent"] = None
        elif tail.startswith("inside."):
            d["parent"] = tail.split(".")[-1]
        elif tail.startswith("match."):
            d["match"] = tail.split(".")[-1]
        elif tail.startswith("entry.fact."):
            _, _, key, st = tail.split(".")
            d["facts"][key] = st
        elif tail == "rule.add":
            d["rules"].append({"kind": "domain", "values": [], "pending": ""})
        elif tail.startswith("rule.") and tail.endswith(".value"):
            i = int(tail.split(".")[1])
            if t and self.m != "count-stuck":
                d["rules"][i]["pending"] = t
        elif tail.startswith("rule.") and ".kind." in tail:
            i = int(tail.split(".")[1])
            d["rules"][i]["kind"] = tail.split(".")[-1]
        elif tail.startswith("rule.") and tail.endswith(".commit"):
            r = d["rules"][int(tail.split(".")[1])]
            if r["pending"]:
                r["values"].append(r["pending"]); r["pending"] = ""
        elif tail.startswith("rule.") and tail.endswith(".remove"):
            del d["rules"][int(tail.split(".")[1])]
        elif tail == "drop":
            d["rules"].append({"kind": "domain", "values": ["linea-supply-example.com"], "pending": ""})
        elif tail == "create" and d["name"]:
            if self.m != "create-silent":
                self.tree.append({"id": "n1", "name": d["name"], "depth": 1, "parent": d["parent"]})
            self.mode, self.lid = "modify", "n1"
        elif tail == "apply":
            self.offer = {"filed": len(self.filed), "join": 0, "leave": 0, "stay": 0}
            self.saved_rules = [dict(r) for r in d["rules"]]
        elif tail == "rerun" and self.offer:
            joined = [m[0] for m in self.matches(d["rules"]) if m[0] not in self.filed]
            self.result = [{"ax": "r.%s" % j, "id": j, "decided_by": "" if self.m == "rerun-no-decided-by" else "rule", "refiled": len(joined)} for j in joined]
            self.filed |= set(joined); self.offer = None
        elif tail == "proof.hold":
            self.hold = not getattr(self, "hold", False)
        elif tail.startswith("switch."):
            self.showing = tail.split(".")[-1]
        elif tail in ("toolbar", "close", "cancel", "done"):
            self.open, self.mode = False, "closed"

    def preview(self):
        if self.mode != "modify" or [r["values"] + [r["pending"]] for r in self.d["rules"]] == [r["values"] + [r["pending"]] for r in self.saved_rules]:
            return None
        now = {m[0] for m in self.matches(self.d["rules"])}
        gone = self.filed - now
        if self.d["meaning"]:                     # delta 12: with a meaning, what no rule matches stays filed
            return {"join": sorted(now - self.filed), "leave": [], "stay": sorted((now & self.filed) | gone)}
        return {"join": sorted(now - self.filed), "leave": sorted(gone), "stay": sorted(now & self.filed)}

    def rows(self, ax):
        d = self.d
        if ax == LIST:
            return [{"id": m[0]} for m in self.CORPUS]
        if ax == "mac-labels-contacts.labels":
            return list(self.tree)
        pv = self.preview()
        if ax == INS:
            n = len(self.matches(d["rules"]))
            return [{"open": self.open, "mode": self.mode, "label_id": self.lid, "name": d["name"], "parent": d["parent"], "colour": d["colour"],
                     "pinned": d["pinned"], "meaning": d["meaning"], "match": d["match"], "rules": len(d["rules"]), "count": n, "matches": n,
                     "counting": getattr(self, "hold", False), "preview": bool(pv), "offer": self.offer, "showing": self.showing, "can_write": bool(d["name"])}]
        if ax == INS + ".rules":
            return [{"index": i, "kind": r["kind"], "values": r["values"] + ([r["pending"]] if r["pending"] else []),
                     "count": len(self.matches([r]))} for i, r in enumerate(d["rules"])]
        if ax == INS + ".matches":
            if pv:
                return [{"id": i, "tag": "x", "edge": e} for e in ("join", "leave", "stay") for i in pv[e]]
            cs = self.conds(d["rules"])
            facts = [] if self.m == "fact-tag-empty" else self.fact_conds()
            word = lambda k: "Is starred" if k == "fact:starred" else k.capitalize()
            return [{"id": m[0], "edge": None, "tag": " · ".join(word(k) for k in dict.fromkeys(c[0] for c in cs + facts if self.hit(c, m)))}
                    for m in self.matches(d["rules"])]
        if ax == INS + ".result":
            return self.result
        if ax == INS + ".geometry":
            return self.geometry()
        if ax == INS + ".rowfit":
            return self.rowfit()
        return []

    def core(self, path):
        if path == "labels/tree":
            return [t["name"] for t in self.tree]
        return "rule" if "decided_by" in path else None



# ---------------------------------------------------------------- round 4: the labelling flow (contract delta 17)
FACTS = ["unread", "unanswered", "has_attachment", "starred", "vip_sender", "cc_me", "mailing_list", "replied_by_me"]
EXIT_WORDS = "Say where the other messages go before you turn the exit filter on"
READS_WORDS = "the AI has to read at least one of: subject, body, attachments. To stop the AI sorting here, switch the card off"
FL = INS + ".flow"
BOARD = "needs-reply"          # À répondre: the label the board draws, served with the board's own figures
BODY_WORDS = "Hi Assem"        # the example's ONE entry chain: a body rule (FixtureFlow.seedFlowExampleLocked, A140); every flow_example row opens so


def example_figures():
    """The example's A140 figures, derived from the design fixture file (docs/design/fixtures/mac-main-sample.json), never from a screen: step 1 picks every
    flow_example row plus the corpus mails whose body holds "Hi Assem" (those wait for the AI until a Re-run); the AI accepts the kept and rejected rows;
    step 3 keeps the kept rows and sends the rejected rows to the fallback."""
    rows = FX["flow_example"]
    waiting = sum(1 for m in FX["messages"] if BODY_WORDS in json.dumps(m.get("body", ""), ensure_ascii=False))
    kept, rej = sum(r["step"] == "kept" for r in rows), sum(r["step"] == "rejected" for r in rows)
    return {"entry": len(rows) + waiting, "ai": kept + rej, "kept": kept, "sent": rej, "waiting": waiting}


EX = example_figures()
EX_PINNED = {"entry": 37, "ai": 32, "kept": 24, "sent": 8, "waiting": 5}   # the owner's example after A140 (docs/lanes/flow3-swift.md); the selftest holds the two equal

def flow_rows(c):
    rows = c.rows(FL)
    if not rows:
        raise Unreadable("no row in region %s" % FL)
    return ({r["card"]: r for r in rows if "card" in r}, {r["chain"]: r for r in rows if "chain" in r})


def new_label(c, name):
    reset(c)
    say(c, INS + ".open.new")
    say(c, INS + ".name", name)
    return settle(c)


def f_cards(c):
    reset(c)
    say(c, "%s.open.modify.%s" % (INS, BOARD))
    settle(c)
    rows = c.rows(FL)
    cards, chains = flow_rows(c)
    order = [r["card"] for r in rows if "card" in r]
    e, a, x = cards.get("entry", {}), cards.get("ai", {}), cards.get("exit", {})
    # A140: step 1 "picks" (not "catches"), the AI card counts what it "accepted" and the picked mails "waiting for the AI", step 3 keeps "of" the accepted.
    want = ("picks %d" % EX["entry"], "%d accepted · %d waiting for the AI" % (EX["ai"], EX["waiting"]), "keeps %d of %d" % (EX["kept"], EX["ai"]))
    # the entry chain is ONE chain (a body rule "Hi Assem", no fact set): the old example's attachment fact and its from rule are gone, and no second chain is read
    entry_ok = chains["entry"]["match"] == "all" and chains["entry"]["set"] == 0 and any(BODY_WORDS in r["values"] and r["kind"] == "body" for r in chains["entry"]["rows"]) \
        and not [k for k in chains if k.startswith("other")]
    ok = order == ["entry", "ai", "exit"] and all(r.get("on") for r in cards.values()) \
        and (e.get("count"), a.get("count"), x.get("count")) == want and entry_ok \
        and chains["exit"]["facts"]["unanswered"] == "yes" and x.get("fallback") == "primary" \
        and a.get("reads") == {"subject": True, "body": True, "rest_of_thread": False, "attachments": False}
    return ok, "order %s, counts %s (want %s), entry chain match %s, facts set %s, rows %s, chains %s, exit fallback %s" % (
        order, [e.get("count"), a.get("count"), x.get("count")], list(want), chains["entry"]["match"], chains["entry"]["set"],
        [(r["kind"], r["values"]) for r in chains["entry"]["rows"]], sorted(chains), x.get("fallback"))


def f_set_aside(c):
    """fix-aisort round 11 (finding F11): the picked mails the label AI's judge refused for themselves are set aside (`label/<id>/flow/ai_set_aside`) and the card SAYS it:
    "N accepted · M waiting for the AI · 1 that Apple's model would not read", in the app's language, with no failure sentence on the card. The test engine serves the
    number only when `fixture ai-set-aside <label> <n>` armed it (`fixture reset` clears it), so the case arms 1 and resets."""
    try:
        c.send("fixture", "ai-set-aside", BOARD, "1")
        reset(c)
        say(c, "%s.open.modify.%s" % (INS, BOARD))
        settle(c)
        a = flow_rows(c)[0].get("ai", {})
        want = "%d accepted · %d waiting for the AI · 1 that Apple's model would not read" % (EX["ai"], EX["waiting"])
        ok = a.get("count") == want and not a.get("ai_refusal")
        return ok, "the AI card says %r (want %r), its refusal sentence is %r (want none)" % (a.get("count"), want, a.get("ai_refusal"))
    finally:
        try:
            fixture_reset(c)
        except Unreadable:
            pass      # said by the next case's own reset


def f_foot(c):
    reset(c)
    say(c, "%s.open.modify.%s" % (INS, BOARD))
    settle(c)
    f = c.rows(INS + ".foot")[0]
    # A140: with the exit filter on the label holds what step 3 kept; step 1's count is "picked" (it files nothing the AI reads), the AI's is "accepted"
    want = ["%d messages" % EX["kept"], "%d picked" % EX["entry"], "%d accepted" % EX["ai"], "%d to Primary" % EX["sent"]]
    return f.get("parts") == want and f.get("held") == EX["kept"] and f.get("text") == " · ".join(want), \
        "foot %r (want %r), held %s (want %s; picked %s, accepted %s, kept %s)" % (f.get("text"), " · ".join(want), f.get("held"), EX["kept"],
                                                                                f.get("caught_by_entry"), f.get("sorted_by_ai"), f.get("kept_by_exit"))


def f_switches(c):
    new_label(c, "Parity switches")
    seen = []
    for card in ("entry", "ai", "exit"):
        before = flow_rows(c)[0][card]["on"]
        say(c, "%s.%s.switch" % (FL, card))
        flipped = flow_rows(c)[0][card]["on"] != before
        say(c, "%s.%s.switch" % (FL, card))
        seen.append((card, flipped, flow_rows(c)[0][card]["on"] == before))
    return all(f and b for _, f, b in seen), "flips and flips back: %s" % seen


def f_exit_refusal(c):
    new_label(c, "Parity exit")
    say(c, FL + ".exit.switch")
    s, x = summary(c), flow_rows(c)[0]["exit"]
    refused = s.get("refusal") == EXIT_WORDS and x.get("error") == EXIT_WORDS and s.get("can_write") is False
    say(c, FL + ".exit.fallback.primary")
    s2, x2 = summary(c), flow_rows(c)[0]["exit"]
    lifted = s2.get("refusal") is None and x2.get("error") is None and s2.get("can_write") is True and x2.get("fallback_name") == "Primary"
    return refused and lifted, "on with no destination: refusal %r, Save open %s; after Primary: refusal %r, Save open %s" % (
        s.get("refusal"), s.get("can_write"), s2.get("refusal"), s2.get("can_write"))


def f_facts(c):
    new_label(c, "Parity facts")
    bad = []
    for key in FACTS:
        for st in ("yes", "no", "any"):
            say(c, "%s.entry.fact.%s.%s" % (INS, key, st))
            f = flow_rows(c)[1]["entry"]["facts"]
            if f[key] != st or any(f[k] != "any" for k in FACTS if k != key):
                bad.append("%s %s" % (key, st))
    return not bad, "%d of %d states landed%s" % (len(FACTS) * 3 - len(bad), len(FACTS) * 3, (": red " + ", ".join(bad[:4])) if bad else "")


def drawn_at(c, ax):
    """True when `measure <ax>` answers a frame with a size (the control is on screen, whatever the pointer does)."""
    try:
        f = (c.send("measure", ax) or {}).get("frame") or {}
    except Unreadable:
        return False
    return f.get("w", 0) > 0 and f.get("h", 0) > 0


def f_rows_removable(c):
    """Owner, 2026-11-12: "impossible to delete extra rules". Every text rule row of the Entry card draws its remove control at rest (it was opacity 0 until the
    pointer was over the row), and removing a row takes it out of the edited rule set at once and out of the saved one at Create. Three rows are added, the
    controls are measured, the second and the third are removed by their own doors; the first row's value stays and is the only one saved."""
    new_label(c, uname("Parity removable"))
    ch = lambda: flow_rows(c)[1].get("entry")
    n0 = ch()["rows_n"]
    for _ in range(3):
        say(c, INS + ".entry.rule.add")
    rows = ch()["rows"]
    added = len(rows) == n0 + 3
    for k, (kind, word) in enumerate((("subject", "alpha"), ("from", "beta"), ("domain", "gamma.example"))):
        say(c, INS + ".entry.rule.%d.kind.%s" % (n0 + k, kind))
        say(c, INS + ".entry.rule.%d.value" % (n0 + k), word)
        say(c, INS + ".entry.rule.%d.commit" % (n0 + k))
    geo = lambda i: "%s.geo.ctl.entry.r%d.remove" % (INS, i)
    shown = [drawn_at(c, geo(n0 + k)) for k in range(3)]
    facts = dict(ch()["facts"])
    say(c, INS + ".entry.rule.%d.remove" % (n0 + 2))
    say(c, INS + ".entry.rule.%d.remove" % (n0 + 1))
    left = ch()["rows"]
    now = len(left) == n0 + 1 and left[n0]["values"] == ["alpha"]
    say(c, INS + ".create")
    lid = summary(c).get("label_id")
    saved = json.dumps(c.core("label/%s/chains/entry" % lid) if lid else None)
    kept = "alpha" in saved and "beta" not in saved and "gamma" not in saved
    same = dict(ch()["facts"]) == facts
    return added and all(shown) and now and kept and same, \
        "three rows added %s, a remove control drawn at rest on each row %s, rows left after removing the second and third %d (want %d, the first holding alpha: %s), saved rule set %s (alpha kept, beta and gamma gone: %s), yes/no facts untouched %s" % (
            added, shown, len(left), n0 + 1, now, saved[:120], kept, same)


FIELDS = [("domain", "Domain", "is"), ("from", "From", "contains"), ("subject", "Subject", "contains"), ("body", "Body", "contains"),
          ("account", "Account", "is"), ("to", "To", "contains"), ("cc", "Cc", "contains"), ("listHeader", "List header", "contains")]


def f_field_condition(c):
    """Owner, 13 Nov: the rule's FIELD (Domain, From, ...) and its CONDITION (is, contains) are two menus. Each of the eight fields is clicked once and the row shows its
    field word and its condition word (a domain and an account are `is`, the text fields `contains`: the forms the core matches); a typed value survives a move between
    text fields and goes when the row crosses to Account (its values are ids); the condition door answers for the form the field offers. Negative forms are not offered."""
    new_label(c, uname("Parity field"))
    ch = lambda: flow_rows(c)[1].get("entry")
    n = ch()["rows_n"]
    say(c, INS + ".entry.rule.add")
    seen = []
    for kind, field, op in FIELDS:
        say(c, INS + ".entry.rule.%d.kind.%s" % (n, kind))
        r = ch()["rows"][n]
        seen.append((kind, r.get("field"), r.get("op")) == (kind, field, op))
    say(c, INS + ".entry.rule.%d.kind.subject" % n)
    say(c, INS + ".entry.rule.%d.value" % n, "invoice")
    say(c, INS + ".entry.rule.%d.commit" % n)
    say(c, INS + ".entry.rule.%d.kind.from" % n)
    kept = ch()["rows"][n]["values"] == ["invoice"]
    say(c, INS + ".entry.rule.%d.kind.account" % n)
    cleared = ch()["rows"][n]["values"] == []
    answered = hit(c, INS + ".entry.rule.%d.op.is" % n)
    refused_other = not hit(c, INS + ".entry.rule.%d.op.contains" % n)
    # the negated twins (round 18): each field's second condition, `is not` for Domain and Account, `does not contain` for the text fields; the row shows it and a move to
    # another field keeps the negation; a form the field lacks is refused
    neg = []
    for kind, field, op in FIELDS:
        say(c, INS + ".entry.rule.%d.kind.%s" % (n, kind))
        want = "is_not" if op == "is" else "does_not_contain"
        neg.append(hit(c, INS + ".entry.rule.%d.op.%s" % (n, want)) and ch()["rows"][n].get("op") == want)
    say(c, INS + ".entry.rule.%d.kind.subject" % n)
    say(c, INS + ".entry.rule.%d.op.does_not_contain" % n)
    say(c, INS + ".entry.rule.%d.kind.from" % n)
    kept_neg = ch()["rows"][n].get("op") == "does_not_contain"
    wrong_form = not hit(c, INS + ".entry.rule.%d.op.is_not" % n)
    return all(seen) and kept and cleared and answered and refused_other and all(neg) and kept_neg and wrong_form, \
        "the eight fields show their field and condition words %s; a value survives subject -> from %s; it goes at Account %s; the `is` door of an account row answers %s and its `contains` door is refused %s; each field's negated form is taken and shown %s; the negation survives a move to another text field %s; `is not` on a text field is refused %s" % (
            seen, kept, cleared, answered, refused_other, neg, kept_neg, wrong_form)


def f_rows_real_click(c):
    """Owner, 13 Nov 07:32: "still cannot delete rules". A REAL pointer press and release on the drawn remove control (the `drag` verb from the control's measured
    frame to itself, never the socket door) removes a filled row, an empty row and the last row; each leaves the row gone, the filled one moves the header's change count, and
    after Create the removed rows do not come back (the old `readDraft` put two empty starter rows back whenever a saved label held none)."""
    new_label(c, uname("Parity real click"))
    ch = lambda: flow_rows(c)[1].get("entry")
    geo = lambda i: "%s.geo.ctl.entry.r%d.remove" % (INS, i)

    def press(i):
        """A real press and release on row i's Remove, once the control is at rest: after a removal the rows slide for a moment, and a place read mid-way is not
        where the control ends (diagnostic on main140: the same click sent after the slide removes the row; sent during it, it fell beside the control)."""
        last = None
        for _ in range(30):
            try:
                f = c.send("measure", geo(i)).get("frame")
            except Unreadable:
                f = None
            if f is not None and f == last:
                break
            last = f
            time.sleep(0.15)
        return c.send("drag", geo(i), geo(i))
    n0 = ch()["rows_n"]
    say(c, INS + ".entry.rule.add")
    say(c, INS + ".entry.rule.%d.kind.subject" % n0)
    say(c, INS + ".entry.rule.%d.value" % n0, "alpha")
    say(c, INS + ".entry.rule.%d.commit" % n0)
    steps, notes = [], []
    changes = lambda: summary(c).get("changes")
    for what, i in (("filled row", n0), ("empty row", 0)):
        before, ch_before = ch()["rows_n"], changes()
        press(i)
        after, ch_after = ch()["rows_n"], changes()
        # a filled row is a change to save, so the header's count moves; an empty row was never part of what Create saves, so its count stays (main136 read 1 -> 1)
        moved = ch_before is None or ch_after != ch_before
        ok = after == before - 1 and (moved if what == "filled row" else True)
        steps.append(ok); notes.append("%s: rows %d -> %d, changes %s -> %s" % (what, before, after, ch_before, ch_after))
    for _ in range(20):          # bounded: a click that misses the control must end the case red, not loop
        if ch()["rows_n"] <= 1:
            break
        press(0)
    before = ch()["rows_n"]
    press(0)
    last = ch()["rows_n"] == before - 1 == 0
    steps.append(last); notes.append("last row: rows %d -> %d" % (before, ch()["rows_n"]))
    say(c, INS + ".name", uname("Parity real click saved"))
    say(c, INS + ".create")
    stays = ch()["rows_n"] == 0
    steps.append(stays); notes.append("after Create the removed rows stay removed: %d rows" % ch()["rows_n"])
    return all(steps), "; ".join(notes)


def f_rows_chains(c):
    new_label(c, "Parity rows")
    ch = lambda k: flow_rows(c)[1].get(k)
    n0 = ch("entry")["rows_n"]
    say(c, INS + ".entry.rule.add")
    added = ch("entry")["rows_n"] == n0 + 1
    say(c, INS + ".entry.rule.%d.kind.subject" % n0)
    say(c, INS + ".entry.rule.%d.value" % n0, "invoice")
    say(c, INS + ".entry.rule.%d.commit" % n0)
    row = ch("entry")["rows"][n0]
    typed = row["kind"] == "subject" and row["values"] == ["invoice"]
    say(c, INS + ".entry.rule.%d.remove" % n0)
    removed = ch("entry")["rows_n"] == n0
    say(c, INS + ".other.add")
    opened = ch("other1") is not None
    if not opened:
        return False, "« Autre règle » opened no second chain"
    say(c, INS + ".other1.fact.starred.yes")
    say(c, INS + ".other1.match.any")
    say(c, INS + ".other1.rule.add")
    o = ch("other1")
    own = o["facts"]["starred"] == "yes" and o["match"] == "any" and o["rows_n"] == 3 and ch("entry")["facts"]["starred"] == "any"
    say(c, INS + ".other1.remove")
    closed = ch("other1") is None
    return added and typed and removed and own and closed, \
        "row added %s, kind and value %s, removed %s, second chain own grid/mode/rows %s, closed %s" % (added, typed, removed, own, closed)


def f_reads(c):
    new_label(c, "Parity reads")
    ai = lambda: flow_rows(c)[0]["ai"]
    start = ai()["reads"] == {"subject": True, "body": True, "rest_of_thread": False, "attachments": False}
    flips = []
    for k in ("attachments", "attachments", "rest_of_thread", "rest_of_thread", "subject", "subject", "body", "body"):
        before = ai()["reads"][k]
        say(c, "%s.ai.reads.%s" % (FL, k))
        flips.append(ai()["reads"][k] != before)
    for k in ("subject", "body"):
        say(c, "%s.ai.reads.%s" % (FL, k))
    s, a = summary(c), ai()
    refused = not any(a["reads"][k] for k in ("subject", "body", "attachments")) and s.get("refusal") == READS_WORDS and a.get("error") == READS_WORDS and s.get("can_write") is False
    say(c, FL + ".ai.reads.rest_of_thread")           # the thread on its own is no reading: the refusal stays
    s_t, a_t = summary(c), ai()
    thread_alone = a_t["reads"]["rest_of_thread"] is True and s_t.get("refusal") == READS_WORDS and s_t.get("can_write") is False
    say(c, FL + ".ai.reads.rest_of_thread")
    say(c, FL + ".ai.reads.subject")
    s2 = summary(c)
    lifted = s2.get("refusal") is None and ai().get("error") is None and s2.get("can_write") is True
    return start and all(flips) and refused and thread_alone and lifted, \
        "default %s, each of the four ticks flips %s, none of the three refused %s, the thread alone still refused %s, one again lifts %s" % (start, all(flips), refused, thread_alone, lifted)


def evidence(c, **reads):
    """` | evidence: ...` for a case that cannot tell a check defect from an app defect: each value the case compared, as the app answered it, then the
    inspector row and the shell's focus (`key_evidence`). Never raises, never hides the verdict it explains."""
    shown = "; ".join("%s=%r" % (k, v) for k, v in reads.items())
    return " | evidence: %s%s" % (shown + "; " if shown else "", key_evidence(c))


def windows_of(c):
    """The windows the app lists (id, kind), or the words of the refusal; never raises."""
    try:
        return [(f.get("id"), f.get("kind")) for f in c.send("winframes").get("frames", [])]
    except (Unreadable, AttributeError, KeyError, TypeError, ValueError) as e:
        return "unread (%s)" % e


def f_thread_saved(c):
    new_label(c, "Parity thread")
    say(c, FL + ".ai.reads.rest_of_thread")
    say(c, INS + ".create")
    lid = summary(c).get("label_id")
    if not lid:
        return False, "Create did not open the label"
    p = "label/%s/flow/ai_reads" % lid
    core_on = c.core(p)
    on = (core_on or {}).get("rest_of_thread") is True
    card_on = ((flow_rows(c) or [{}])[0].get("ai") or {}).get("reads")
    say(c, FL + ".ai.reads.rest_of_thread")
    card_off = ((flow_rows(c) or [{}])[0].get("ai") or {}).get("reads")
    say(c, INS + ".apply")
    core_off = c.core(p)
    off = (core_off or {}).get("rest_of_thread", False) is False      # the key is served only while it is on (FixtureFlow.swift `ai_reads`): missing reads false
    detail = "the core reads back rest_of_thread %s after Create with the tick, %s after Save without it" % (on, off)
    if on and off:
        return True, detail
    return False, detail + evidence(c, path=p, core_after_create=core_on, card_reads_after_create=card_on, card_reads_after_untick=card_off, core_after_save=core_off)


AI_WHY = "No model is set for sorting mail: choose one in Settings, Decisions."


def f_ai_refusal(c):
    """While the AI cannot run, the AI card says why (the core's `label/<id>/flow/ai_refusal` sentence, or the shell's when the on-device weights are not on
    this Mac), the empty list says it too, and the control beside it is the door that fixes it: Decisions opens its pane; the weights' download is only
    named here (a check never starts an 840 MB download).

    The fixture core serves the refusal only on a NEW installation with no decision model (FixtureFirstRun.swift `firstRunRefusalLocked`), so the case stages one
    (`fixture install-state new`, which keeps the built-in labels only: it makes its own label) and brings the sample back (`fixture reset`) when it ends."""
    c.send("fixture", "install-state", "new")
    try:
        return f_ai_refusal_run(c)
    finally:
        try:
            fixture_reset(c)
        except Unreadable:
            pass      # said by the next case's own reset


UNDER_TRUST = "The AI judged 42 mails and was sure of none: its best was 58 %, and it files from 80 %."      # the engine's ai_under_trust words with its three numbers filled (English: the run sets it)


def f_ai_under_trust(c):
    """The engine's `ai_under_trust` (label/<id>/flow/ai_refusal {code, judged, highest_percent, trust_percent, words}): the AI judged mails and was sure of none. The test engine
    serves it for the built-in Social (FixtureFlow.swift `ai_refusal`: 42 judged, best 58 %, files from 80 %). The AI card says the sentence with the three numbers filled, in
    the app's language, never the raw `%d` format; the control under it is the same Open Decisions door (`decisions`), which opens Settings > AI on the Decisions section
    (AISettingsStore `openDecisions`; the trust row itself is not aimed at)."""
    reset(c)
    say(c, "%s.open.modify.social" % INS)
    settle(c)
    card = flow_rows(c)[0]["ai"]
    why, door = card.get("ai_refusal"), card.get("door")
    if not why:
        raise Unreadable("the AI card shows no refusal for Social although the test engine serves ai_under_trust for it | evidence: card_ai=%s" % card)
    opened = surface = None
    if door and door.endswith(".decisions"):
        opened = hit(c, door)
        surface = c.uistate().get("surface")
    ok = why == UNDER_TRUST and bool(door) and door.endswith(".decisions") and opened and surface == "mac-settings-ai"
    return ok, "the card says %r (want %r), door %r answered %s and landed on %r (want mac-settings-ai)" % (why, UNDER_TRUST, door, opened, surface)


NEEDS_APPLE = "Sorting mail into labels needs Apple Intelligence: turn it on in System Settings, then Apple Intelligence and Siri."
APPLE_PANE = "x-apple.systempreferences:com.apple.Siri-Settings.extension"


def f_ai_needs_apple(c):
    """The engine's `ai_needs_apple` (label/<id>/flow/ai_refusal {code, words}): Apple's on-device model judges the label's AI step and Apple Intelligence is off. The test engine
    serves it for the built-in Updates. The AI card says the sentence in the app's language, and its door is NOT "Open Decisions" but the one to System Settings at the
    Apple Intelligence pane (`apple.settings`); on a test copy the door opens nothing and the `apple-settings` row holds the URL it would open."""
    reset(c)
    say(c, "%s.open.modify.updates" % INS)
    settle(c)
    card = flow_rows(c)[0]["ai"]
    why, door = card.get("ai_refusal"), card.get("door")
    if not why:
        raise Unreadable("the AI card shows no refusal for Updates although the test engine serves ai_needs_apple for it | evidence: card_ai=%s" % card)
    before = (c.rows(INS + ".apple-settings") or [{}])[0].get("opened", [])
    opened = hit(c, door) if door and door.endswith(".apple.settings") else None
    after = (c.rows(INS + ".apple-settings") or [{}])[0].get("opened", [])
    ok = why == NEEDS_APPLE and bool(door) and door.endswith(".apple.settings") and opened and before == [] and after == [APPLE_PANE]
    return ok, "the card says %r (want %r), door %r answered %s; URLs opened before the press %s, after it %s (want [%r])" % (why, NEEDS_APPLE, door, opened, before, after, APPLE_PANE)


def f_ai_refusal_run(c):
    fresh(c, uname("Parity AI refusal"))
    say(c, INS + ".rule.0.kind.domain")
    commit(c, 0, "linea-supply-example.com")
    settle(c)
    say(c, INS + ".create")
    settle(c)
    lid = summary(c).get("label_id")
    try:
        core_words = ((c.core("label/%s/flow/ai_refusal" % lid) or {}) if lid else {}).get("words")
    except Unreadable:
        core_words = None       # no value is the core's answer "nothing refuses": the fixture core serves a refusal only on a new installation with no decision model (FixtureFirstRun.swift:73-74)
    card = flow_rows(c)[0]["ai"]
    why, door = card.get("ai_refusal"), card.get("door")
    if not why:
        raise Unreadable("the AI card shows no refusal and the core serves none for label %r (a model can sort on this installation, or the card is off): the refusal is judged on a new installation (`fixture install-state new`)%s" % (
            lid, evidence(c, core_path="label/%s/flow/ai_refusal" % lid, core_words=core_words, card_ai=card)))
    empty = c.rows(INS + ".empty")      # only while the label has no rule and no row: skipped otherwise
    said = (core_words is None or why == core_words) and (not empty or empty[0].get("words") == why)
    if door and door.endswith(".decisions"):
        opened = hit(c, door)
        surface = c.uistate().get("surface")
        fixed = opened and surface == "mac-settings-ai"
    else:
        fixed = bool(door) and door.endswith(".laya.download")
        opened, surface = fixed, "(download door named, not clicked)"
    detail = "core says %r, the card says %r, the empty list %s, door %r: %s, surface %r" % (core_words, why, "says it" if empty else "is not shown", door, fixed, surface)
    if said and fixed:
        return True, detail
    return False, detail + evidence(c, core_words=core_words, card_refusal=why, empty_rows=empty, door=door, door_click_answered=opened, surface_after=surface)


def f_fallback(c):
    new_label(c, "Parity fallback")
    labels = [r for r in c.rows("mac-labels-contacts.labels") if str(r.get("id", "")).startswith("lbl-")]
    if not labels:
        raise Unreadable("no owner label to send the others to")
    seen = []
    for ident, want in (("primary", "Primary"), ("junk", "Spam"), (labels[0]["id"], labels[0]["name"])):
        say(c, "%s.exit.fallback.%s" % (FL, ident))
        x = flow_rows(c)[0]["exit"]
        seen.append(x.get("fallback") == ident and x.get("fallback_name") == want)
    return all(seen), "Primary, Spam and %r each named: %s" % (labels[0]["name"], seen)


def f_why(c):
    reset(c)
    say(c, "%s.open.modify.%s" % (INS, BOARD))
    settle(c)
    tabs = c.rows("mac-main.splits")                           # the label's own rows are listed on its tab, not on Primary
    at = next((i for i, r in enumerate(tabs) if r.get("id") == BOARD), None)
    if at is not None:
        c.send("click", "mac-main.splits.%d" % at)
    ids = [r["id"] for r in c.rows(LIST)]
    def has_why(i):
        try:                                                     # the core answers "no value" for a message the label does not hold: that row is skipped, not the check
            return bool(c.core("message/%s/why/%s" % (i, BOARD)))
        except Unreadable:
            return False
    mid = next((i for i in ids if has_why(i)), None)
    if mid is None:
        raise Unreadable("no message of %s has a why-here read" % BOARD)
    read = c.core("message/%s/why/%s" % (mid, BOARD))
    card = read["card"]
    word = {"entry": "Entry rules", "ai": "AI sorting", "exit": "Exit filter", "fallback": "Sent here by the exit filter", "hand": "Placed by hand"}[card]
    focus_on(c, mid)
    say(c, INS + ".why")
    opened = c.rows(INS + ".foot")[0].get("why_open") is True
    lines = c.rows(INS + ".why")
    says = len(lines) == 2 and word in lines[1]["text"]
    # A140: a mail the AI accepted is placed by step 1, the AI and (when it kept it) step 3: the line names the AI's answer ("AI: yes") whenever the read says ai yes
    ai_named = read.get("ai") != "yes" or (len(lines) == 2 and "AI: yes" in lines[1]["text"])
    say(c, INS + ".why")
    closed = c.rows(INS + ".foot")[0].get("why_open") is False
    return opened and says and ai_named and closed, "message %s placed by %s (read ai %r): panel open %s, line names the card %s and the AI's answer %s, lines %s, closes %s" % (
        mid, card, read.get("ai"), opened, says, ai_named, [l["text"] for l in lines], closed)


def f_save(c):
    new_label(c, "Parity flow")
    say(c, FL + ".exit.fallback.junk")
    say(c, FL + ".exit.switch")
    say(c, INS + ".exit.fact.unanswered.yes")
    say(c, INS + ".entry.fact.starred.yes")
    say(c, INS + ".other.add")
    say(c, INS + ".other1.fact.vip_sender.yes")
    say(c, INS + ".create")
    s = summary(c)
    lid = s.get("label_id")
    if s.get("mode") != "modify" or not lid:
        return False, "Create did not open the label (mode %s, error %r)" % (s.get("mode"), s.get("failure") or s.get("refusal"))
    p = "label/%s/" % lid
    exit_chain = c.core(p + "chains/exit") or []
    entry = c.core(p + "chains/entry") or []
    read = c.core(p + "flow/exit_enabled") is True and c.core(p + "flow/exit_fallback") == "junk" \
        and len(exit_chain) == 1 and exit_chain[0]["facts"]["unanswered"] == "yes" \
        and len(entry) == 2 and entry[0]["facts"]["starred"] == "yes" and entry[1]["facts"]["vip_sender"] == "yes"
    say(c, FL + ".exit.switch")
    say(c, INS + ".other1.remove")
    say(c, INS + ".apply")
    asked = bool(summary(c).get("offer"))
    say(c, INS + ".rerun")
    answered = summary(c).get("offer") is None
    after = c.core(p + "flow/exit_enabled") is False and len(c.core(p + "chains/entry") or []) == 1
    return read and asked and answered and after, "core reads back: exit on+junk+chain, two entry chains %s; then flow change asks %s, Re-run answers %s, saved %s" % (read, asked, answered, after)


FLOW = [("flow-cards-in-order", f_cards, "cards-out-of-order"), ("flow-cards-in-order", f_cards, "cards-old-words"), ("flow-cards-in-order", f_cards, "entry-two-chains"),
        ("flow-foot-provenance", f_foot, "foot-counts-ai-as-held"), ("flow-foot-provenance", f_foot, "foot-old-words"),
        ("flow-switches", f_switches, "switch-dead"), ("flow-exit-needs-fallback", f_exit_refusal, "exit-without-fallback-accepted"),
        ("flow-fact-states", f_facts, "fact-no-ignored"), ("flow-rows-and-chains", f_rows_chains, "other-chain-missing"),
        ("flow-ai-reads", f_reads, "last-tick-allowed"), ("flow-ai-reads", f_reads, "thread-alone-allowed"),
        ("flow-ai-thread-saved", f_thread_saved, "thread-tick-not-written"),
        ("flow-ai-refusal-shown-and-opens-decisions", f_ai_refusal, "ai-refusal-hidden"), ("flow-ai-refusal-shown-and-opens-decisions", f_ai_refusal, "decisions-dead"), ("flow-fallback-picker", f_fallback, "fallback-ignored"),
        ("flow-every-rule-row-has-its-remove", f_rows_removable, "only-first-row-removable"),
        ("flow-rule-field-and-condition-menus", f_field_condition, "op-not-shown"),
        ("flow-real-click-removes-a-rule-row", f_rows_real_click, "click-misses-the-control"), ("flow-real-click-removes-a-rule-row", f_rows_real_click, "starter-rows-come-back"),
        ("flow-ai-under-trust-says-its-three-numbers", f_ai_under_trust, "trust-numbers-dropped"),
        ("flow-ai-needs-apple-says-it-and-opens-system-settings", f_ai_needs_apple, "needs-apple-door-decisions"),
        ("flow-ai-card-counts-the-mails-apple-would-not-read", f_set_aside, "aside-not-said"),
        ("flow-why-here", f_why, "why-ignores-focus"), ("flow-why-here", f_why, "why-drops-the-ai-answer"), ("flow-save-reads-back", f_save, "save-drops-flow")]


class FakeFlow:
    """The three cards on the wire, for --selftest: a draft (switches, AI ticks, fallback, chains), a core that keeps what Create and Save
    write, the board's example label, and a why-here read. A mutant breaks exactly the behaviour its check names (FLOW rows)."""
    def __init__(self, mutant=None):
        self.m, self.typed, self.name = mutant, "", ""
        self.tree = [{"id": "lbl-sup", "name": "Suppliers", "depth": 1, "parent": None}]
        self.open, self.mode, self.lid, self.offer, self.why_open, self.f = False, "closed", None, None, False, 0
        self.ids = ["m0", "m1", "m2"]
        self.store = {BOARD: self.example()}
        self.d, self.saved = self.blank(), None
        self.surface = "mac-main"
        self.apple = []          # the URLs the Apple Intelligence door recorded
        self.aside = 0           # `fixture ai-set-aside`: the picked mails the judge set aside

    @staticmethod
    def chain(seed=True):
        return {"facts": {k: "any" for k in FACTS}, "match": "all",
                "rows": [{"kind": "from", "values": []}, {"kind": "subject", "values": []}] if seed else []}

    def blank(self):
        return {"on": {"entry": True, "ai": True, "exit": False}, "reads": {"subject": True, "body": True, "rest_of_thread": False, "attachments": False},
                "fallback": None, "entry": self.chain(), "exit": self.chain(False), "others": []}

    def example(self):
        d = self.blank()
        d["on"]["exit"], d["fallback"] = True, "primary"
        d["entry"]["rows"] = [{"kind": "body", "values": [BODY_WORDS]}]      # A140: ONE entry chain, a body rule; no fact set
        if self.m == "entry-two-chains":
            d["others"].append(self.chain())
        d["exit"]["facts"]["unanswered"] = "yes"
        return d

    def refusal(self):
        # the thread is read WITH the message: on its own it is no reading (mutant thread-alone-allowed counts it)
        counted = self.d["reads"].values() if self.m == "thread-alone-allowed" else [self.d["reads"][k] for k in ("subject", "body", "attachments")]
        if not any(counted) and self.m != "last-tick-allowed":
            return READS_WORDS
        if self.d["on"]["exit"] and not self.d["fallback"] and self.m != "exit-without-fallback-accepted":
            return EXIT_WORDS
        return None

    def tree_ids(self):
        return [n["id"] for n in self.tree]

    def chain_of(self, key):
        d = self.d
        if key == "entry":
            return d["entry"]
        if key == "exit":
            return d["exit"]
        n = int(key[5:]) - 1
        return d["others"][n] if 0 <= n < len(d["others"]) else None

    def send(self, verb, *a):
        if verb == "type":
            self.typed = a[0]
        elif verb == "click":
            self.click(a[0])
        elif verb == "drag" and a[0] == a[1]:
            m = re.search(r"\.geo\.ctl\.entry\.r(\d+)\.remove$", a[0])
            if m and self.m != "click-misses-the-control":
                self.chain_click(["entry", "rule", m.group(1), "remove"], "")
        elif verb == "fixture" and a[:1] == ("ai-set-aside",):
            self.aside = int(a[2])
        elif verb == "measure":
            m = re.search(r"\.geo\.ctl\.entry\.r(\d+)\.remove$", a[0])
            if not m:
                raise Unreadable("no measurement at %s" % a[0])
            n0 = len(self.chain()["rows"])
            if self.m == "only-first-row-removable" and int(m.group(1)) != 0:
                raise Unreadable("no measurement at %s: nothing on screen carries that id" % a[0])
            return {"frame": {"w": 16, "h": 16}}
        return {}

    def click(self, ax):
        t, self.typed = self.typed, ""
        d = self.d
        if ax.startswith(LIST + ".row."):
            self.f = int(ax.split(".")[-1])
            return
        tail = ax[len(INS) + 1:] if ax.startswith(INS + ".") else ax
        parts = tail.split(".")
        if tail == "open.new":
            self.open, self.mode, self.lid, self.d, self.name, self.offer = True, "new", None, self.blank(), "", None
        elif tail.startswith("open.modify."):
            lid = tail.split(".")[-1]
            if lid == "social" and lid not in self.tree_ids():
                self.tree.append({"id": "social", "name": "Social", "depth": 1, "parent": None})
            self.open, self.mode, self.lid, self.offer = True, "modify", lid, None
            self.d = json.loads(json.dumps(self.store.get(lid) or self.blank()))
            self.saved = json.loads(json.dumps(self.d))
        elif tail == "name":
            self.name = t
        elif tail.startswith("flow.") and tail.endswith(".switch"):
            card = parts[1]
            if not (card == "entry" and self.m == "switch-dead"):
                d["on"][card] = not d["on"][card]
        elif tail.startswith("flow.ai.reads."):
            d["reads"][parts[-1]] = not d["reads"][parts[-1]]
        elif tail.startswith("flow.exit.fallback."):
            if self.m != "fallback-ignored":
                d["fallback"] = parts[-1]
        elif tail == "apple.settings":
            self.apple.append(APPLE_PANE)
        elif tail == "decisions":
            if self.m == "decisions-dead":
                return
            self.surface = "mac-settings-ai"
        elif tail == "other.add":
            if self.m != "other-chain-missing":
                d["others"].append(self.chain())
        elif tail == "why":
            self.why_open = not self.why_open
        elif tail == "create" and self.name and not self.refusal():
            if self.m == "starter-rows-come-back" and not d["entry"]["rows"]:
                d["entry"]["rows"] = [{"kind": "from", "values": []}, {"kind": "subject", "values": []}]
            self.mode, self.lid = "modify", "n1"
            self.tree.append({"id": "n1", "name": self.name, "depth": 1, "parent": None})
            self.store["n1"] = self.blank() if self.m == "save-drops-flow" else json.loads(json.dumps(d))
            self.saved = json.loads(json.dumps(d))
        elif tail == "apply" and not self.refusal():
            changed = d != self.saved
            self.store[self.lid] = json.loads(json.dumps(d))
            self.saved = json.loads(json.dumps(d))
            self.offer = {"filed": 1, "join": 0, "leave": 0, "stay": 0} if changed else None
        elif tail == "rerun" and self.offer:
            self.offer = None
        else:
            self.chain_click(parts, t)

    def chain_click(self, parts, t):
        key = parts[0] if parts[0] in ("entry", "exit") or parts[0].startswith("other") else "entry"
        if parts[0] != key:
            parts = [key] + parts           # the round 2 ids: rule.0.value, match.all
        ch = self.chain_of(key)
        if ch is None:
            return
        rest = parts[1:]
        if rest[0] == "fact":
            if not (rest[2] == "no" and self.m == "fact-no-ignored"):
                ch["facts"][rest[1]] = rest[2]
        elif rest[0] == "match":
            ch["match"] = rest[1]
        elif rest[:2] == ["rule", "add"]:
            ch["rows"].append({"kind": "domain", "values": []})
        elif rest[0] == "rule":
            i, what = int(rest[1]), rest[2]
            if i >= len(ch["rows"]):
                return
            if what == "kind":
                if (ch["rows"][i]["kind"] == "account") != (rest[3] == "account"):
                    ch["rows"][i]["values"] = []; ch["rows"][i].pop("pending", None)
                ch["rows"][i]["kind"] = rest[3]
            elif what == "op":
                base = dict((k, o) for k, _, o in FIELDS)[ch["rows"][i]["kind"]]
                negative = "is_not" if base == "is" else "does_not_contain"
                if rest[3] not in (base, negative):
                    raise Unreadable("click %s: refused" % rest[3])
                ch["rows"][i]["negated"] = rest[3] == negative
            elif what == "value":
                ch["rows"][i]["pending"] = t
            elif what == "commit":
                ch["rows"][i]["values"] += [ch["rows"][i].pop("pending", "")]
            elif what == "remove":
                del ch["rows"][i]
        elif rest[0] == "remove" and key.startswith("other"):
            self.d["others"].remove(ch)

    def counts(self):
        return {k: EX[k] for k in ("entry", "ai", "kept", "sent")} if self.lid == BOARD else {"entry": 0, "ai": 0, "kept": 0, "sent": 0}

    def card_rows(self):
        d, c = self.d, self.counts()
        if self.m == "cards-old-words":      # the words before A140: step 1 "catches", the AI card a bare message count
            text = {"entry": "catches %d" % c["entry"] if self.lid else "", "ai": "%d messages" % c["ai"] if self.lid else "",
                    "exit": "keeps %d of %d" % (c["kept"], c["ai"]) if self.lid else ""}
        else:
            wait = EX["waiting"] if self.lid == BOARD else 0
            text = {"entry": "picks %d" % c["entry"] if self.lid else "",
                    "ai": (("%d accepted · %d waiting for the AI" % (c["ai"], wait) if wait else "%d accepted" % c["ai"])
                           + (" · %d that Apple's model would not read" % self.aside if self.aside and self.m != "aside-not-said" else "")) if self.lid else "",
                    "exit": "keeps %d of %d" % (c["kept"], c["ai"]) if self.lid else ""}
        err, names = self.refusal(), {"primary": "Primary", "junk": "Spam", "lbl-sup": "Suppliers"}
        rows = [{"card": "entry", "on": d["on"]["entry"], "count": text["entry"]},
                {"card": "ai", "on": d["on"]["ai"], "count": text["ai"], "reads": dict(d["reads"]), "error": err if err == READS_WORDS else None,
                 "ai_refusal": None if (self.lid == BOARD and self.aside) else NEEDS_APPLE if self.lid == "updates" else None if self.m == "ai-refusal-hidden" else (
                     ("The AI judged %d mails and was sure of none: its best was %d %%, and it files from %d %%." if self.m == "trust-numbers-dropped" else UNDER_TRUST)
                     if self.lid == "social" else AI_WHY), "door": INS + (".decisions" if self.m == "needs-apple-door-decisions" or self.lid != "updates" else ".apple.settings")},
                {"card": "exit", "on": d["on"]["exit"], "count": text["exit"], "fallback": d["fallback"], "fallback_name": names.get(d["fallback"]),
                 "error": err if err == EXIT_WORDS else None}]
        return [rows[1], rows[0], rows[2]] if self.m == "cards-out-of-order" else rows

    def chain_rows(self):
        out = []
        for key, ch in [("entry", self.d["entry"])] + [("other%d" % (i + 1), o) for i, o in enumerate(self.d["others"])] + [("exit", self.d["exit"])]:
            out.append({"chain": key, "match": ch["match"], "facts": dict(ch["facts"]), "set": sum(v != "any" for v in ch["facts"].values()),
                        "rows": [dict(kind=r["kind"], field=dict((k, f) for k, f, _ in FIELDS)[r["kind"]], op=(None if self.m == "op-not-shown" else (lambda b: ("is_not" if b == "is" else "does_not_contain") if r.get("negated") else b)(dict((k, o) for k, _, o in FIELDS)[r["kind"]])),
                                 values=r["values"] + ([r["pending"]] if r.get("pending") else [])) for r in ch["rows"]], "rows_n": len(ch["rows"])})
        return out

    def rows(self, ax):
        d = self.d
        if ax == INS:
            r = self.refusal()
            return [{"open": self.open, "mode": self.mode, "label_id": self.lid, "name": self.name, "counting": False, "refusal": r,
                     "can_write": bool(self.name) and r is None and (self.mode == "new" or d != self.saved), "offer": self.offer}]
        if ax == FL:
            return self.card_rows() + self.chain_rows()
        if ax == INS + ".foot":
            c = self.counts()
            held = c["ai"] if self.m == "foot-counts-ai-as-held" else c["kept"]          # A140: the exit filter is on, so the label holds what step 3 kept
            names = ("%d by rule", "%d by the AI") if self.m == "foot-old-words" else ("%d picked", "%d accepted")
            parts = ["%d messages" % held, names[0] % c["entry"], names[1] % c["ai"], "%d to Primary" % c["sent"]]
            return [{"text": " · ".join(parts), "parts": parts, "held": held, "why_open": self.why_open,
                     "caught_by_entry": c["entry"], "sorted_by_ai": c["ai"], "kept_by_exit": c["kept"]}]
        if ax == INS + ".apple-settings":
            return [{"opened": list(self.apple)}]
        if ax == INS + ".empty":
            return [{"words": AI_WHY, "door": INS + ".decisions"}] if self.m != "ai-refusal-hidden" else []
        if ax == INS + ".why":
            if not self.why_open:
                return []
            seen = self.ids[0 if self.m == "why-ignores-focus" else self.f]
            line = "Body: %s · %s · Exit filter · chain 1 · Unanswered: yes" % (BODY_WORDS, "AI: no" if self.m == "why-drops-the-ai-answer" else "AI: yes")
            return [{"text": "Subject of %s" % seen}, {"text": line if seen == "m1" else "This label does not hold this message."}]
        if ax == "mac-labels-contacts.labels":
            return list(self.tree)
        if ax == LIST:
            return [{"id": i} for i in self.ids]
        return []

    def uistate(self):
        return {"focus_message": self.ids[self.f], "surface": self.surface}

    def core(self, path):
        parts = path.split("/")
        if parts[0] == "label" and path.endswith("/flow/ai_refusal"):
            return {"code": "ai_no_model", "words": AI_WHY}
        if parts[0] == "message":
            # A140: step 1 picked the mail, the AI accepted it, step 3 kept it: the card is "exit", with the AI's answer and step 1's own read
            return {"card": "exit", "chain_id": 0, "ai": "yes", "entry": {"chain_id": 0, "matched": [{"kind": "body", "words": BODY_WORDS}]}} \
                if parts[1] == "m1" and parts[3] == BOARD else None
        flow = self.store.get(parts[1])
        if not flow:
            return None
        if parts[2] == "flow":
            if parts[3] == "ai_reads":
                return dict(flow["reads"]) if self.m != "thread-tick-not-written" else dict(flow["reads"], rest_of_thread=False)
            return {"exit_enabled": flow["on"]["exit"], "exit_fallback": flow["fallback"]}[parts[3]]
        if parts[2] == "chains":
            chains = [flow["entry"]] + flow["others"] if parts[3] == "entry" else ([flow["exit"]] if any(v != "any" for v in flow["exit"]["facts"].values()) else [])
            return [{"chain_id": i, "facts": dict(c["facts"]), "conditions": [{"kind": r["kind"], "words": v} for r in c["rows"] for v in r["values"]]} for i, c in enumerate(chains)]
        return None



# ---------------------------------------------------------------- doors-labels: every control of the labels window and the inspector is clicked once
# Round doors-labels (docs/lanes/doors-labels.md): the controls no case clicked. Each case clicks the control's door and reads a visible
# outcome off the rows; each has a stand-in-app mutant that disables that one control (--selftest). Names: w_ = the window, d_ = the inspector,
# kd_ = a key, the move picker's Esc door.
import re

W = "mac-labels-contacts"
STAMP = [int(time.time()) % 100000, 0]


def uname(stem):
    """A label or folder name no earlier run of a rig left behind."""
    STAMP[1] += 1
    return "%s %d-%d" % (stem, STAMP[0], STAMP[1])


def hit(c, ax):
    """A click that did nothing (the door answers ok:false) is a RED outcome of its case, not an unjudged one."""
    try:
        c.send("click", ax)
        return True
    except Unreadable:
        return False


def until(fn, tries=12):
    """A door that moves the keys is answered on the view's next frame: look again for a moment before saying no."""
    v = fn()
    for _ in range(tries):
        if v:
            return v
        time.sleep(0.1)
        v = fn()
    return v


def slug(name):
    return "-".join(p for p in re.split(r"[^a-z0-9]+", name.lower()) if p)


def lrows(c):
    return c.rows(W + ".labels")


def window(c):
    reset(c)
    c.send("click", W + ".open")
    c.send("click", W + ".show.labels")


def make_label(c, name):
    """A top-level label of this name, made through the window's own door; the window then has it selected."""
    c.send("type", name)
    c.send("click", W + ".labels.create.top")
    r = next((r for r in lrows(c) if r["name"] == name), None)
    if r is None:
        raise Unreadable("the label %r was not created" % name)
    return r


def label(c, lid):
    return next((r for r in lrows(c) if r["id"] == lid), None)


def w_select(c):
    window(c)
    a, b = make_label(c, uname("Pick A")), make_label(c, uname("Pick B"))
    c.send("click", W + ".folder.new")
    in_editor = lrows(c)[0]["pane"] == "folder"
    clicked = hit(c, "%s.labels.select.%s" % (W, a["id"]))
    ra, rb = label(c, a["id"]), label(c, b["id"])
    return in_editor and clicked and ra["selected"] and not rb["selected"] and ra["pane"] == "labels", \
        "editor open %s, click answered %s, A selected %s, B selected %s, pane %s" % (in_editor, clicked, ra["selected"], rb["selected"], ra["pane"])


def w_rename(c):
    window(c)
    a = make_label(c, uname("Rename me"))
    new = uname("Renamed")
    c.send("type", new)
    clicked = hit(c, W + ".labels.rename")
    r = label(c, a["id"])
    return clicked and r["name"] == new and r["path"].endswith(new), "after rename: %r (path %r), click answered %s" % (r["name"], r["path"], clicked)


def w_inside(c):
    window(c)
    a = make_label(c, uname("Holder"))
    kid = uname("Kid")
    c.send("type", kid)
    clicked = hit(c, W + ".labels.create")
    r = next((r for r in lrows(c) if r["name"] == kid), None)
    return clicked and r is not None and r["parent"] == a["id"] and r["depth"] == a["depth"] + 1, \
        "kid %r parent %r depth %r (holder %s depth %s)" % (kid, r and r["parent"], r and r["depth"], a["id"], a["depth"])


def w_drop_top(c):
    window(c)
    a = make_label(c, uname("Holder"))
    kid = uname("Kid")
    c.send("type", kid)
    c.send("click", W + ".labels.create")
    k = next(r for r in lrows(c) if r["name"] == kid)
    nested = k["parent"] == a["id"]
    clicked = hit(c, W + ".labels.drop.top")
    r = label(c, k["id"])
    return nested and clicked and r["parent"] is None and r["depth"] == 1, "nested %s, after the top drop: parent %r depth %r" % (nested, r["parent"], r["depth"])


def w_drop_onto(c):
    window(c)
    a, b = make_label(c, uname("Moved")), make_label(c, uname("Target"))
    c.send("click", "%s.labels.select.%s" % (W, a["id"]))
    clicked = hit(c, "%s.labels.drop.%s" % (W, b["id"]))
    r = label(c, a["id"])
    return clicked and r["parent"] == b["id"] and r["depth"] == b["depth"] + 1, "after the drop door: parent %r depth %r (target %s)" % (r["parent"], r["depth"], b["id"])


def new_folder(c, name, rule="linea"):
    c.send("click", W + ".folder.new")
    c.send("type", name)
    c.rows(W + ".rule.matches")
    c.send("type", rule)
    return c.rows(W + ".rule.matches")[0]


def w_folder_save_cancel(c):
    window(c)
    name = uname("Parity folder")
    draft = new_folder(c, name)
    saved = hit(c, W + ".rule.save")
    side = [r for r in c.rows(W + ".sidebar") if r["title"] == name]
    other = name + " b"
    c.send("click", W + ".folder.new")
    c.send("type", other)
    c.rows(W + ".rule.matches")
    cancelled = hit(c, W + ".rule.cancel")
    after = c.rows(W + ".rule.matches")
    left = [r for r in c.rows(W + ".sidebar") if r["title"] == other]
    return draft["name"] == name and draft["rule"] == "linea" and saved and len(side) == 1 and cancelled and after == [] and not left, \
        "typed name %r rule %r; Save answered %s, sidebar rows %d; Cancel answered %s, editor rows after %d, folder left %d" % (
            draft["name"], draft["rule"], saved, len(side), cancelled, len(after), len(left))


def w_folder_cancel_saved(c):
    """F1 of the control sweep: Cancel on a SAVED smart folder reverts the draft and closes the editor too."""
    window(c)
    name = uname("Cancel saved")
    new_folder(c, name)
    c.send("click", W + ".rule.save")
    side = next((r for r in c.rows(W + ".sidebar") if r["title"] == name), None)
    if side is None:
        raise Unreadable("the folder %r was not saved" % name)
    c.send("click", W + ".show.labels")
    c.send("click", "%s.edit.%s" % (W, side["id"]))
    c.send("type", " extra")
    opened = c.rows(W + ".rule.matches") != []
    cancelled = hit(c, W + ".rule.cancel")
    closed = c.rows(W + ".rule.matches") == []
    c.send("click", "%s.edit.%s" % (W, side["id"]))
    reverted = (c.rows(W + ".rule.matches") or [{}])[0].get("rule")
    return opened and cancelled and closed and reverted == "linea", \
        "editor open %s, Cancel answered %s, editor closed after %s, reopened rule %r" % (opened, cancelled, closed, reverted)


def w_sender_row_opens_thread(c):
    """F3 of the control sweep: a click on a message row of the sender card opens that thread in the reader."""
    window(c)
    people = c.rows(W + ".people")
    if not people:
        raise Unreadable("the window lists no person")
    c.send("click", "%s.person.%s" % (W, people[0]["id"]))
    msgs = c.rows(W + ".contact.messages")
    if not msgs:
        raise Unreadable("the sender card lists no message")
    clicked = hit(c, msgs[0]["ax"])
    st = c.uistate()
    ok = clicked and st.get("surface") == "mac-reader" and st.get("focus_message") == msgs[0]["id"]
    detail = "row click answered %s, surface %r, focus %r (row %r)" % (clicked, st.get("surface"), st.get("focus_message"), msgs[0]["id"])
    if ok:
        return True, detail
    return False, detail + evidence(c, row=msgs[0], click_answered=clicked, uistate={k: st.get(k, "absent") for k in ("surface", "overlay", "focus_message", "first_responder")},
                                    app_windows=windows_of(c))


def w_folder_edit(c):
    window(c)
    name = uname("Edit me")
    new_folder(c, name)
    c.send("click", W + ".rule.save")
    side = next((r for r in c.rows(W + ".sidebar") if r["title"] == name), None)
    if side is None:
        raise Unreadable("the folder %r was not saved" % name)
    c.send("click", W + ".show.labels")
    away = c.rows(W + ".rule.matches") == []
    clicked = hit(c, "%s.edit.%s" % (W, side["id"]))
    rows = c.rows(W + ".rule.matches")
    r = rows[0] if rows else {}
    return away and clicked and r.get("new") is False and r.get("folder_id") == side["id"] and r.get("name") == name and r.get("rule") == "linea", \
        "editor away %s, click answered %s, editor on %r (new %s, id %s, rule %r)" % (away, clicked, r.get("name"), r.get("new"), r.get("folder_id"), r.get("rule"))


def w_parent_chip(c):
    window(c)
    a = make_label(c, uname("Chip home"))
    c.send("click", W + ".folder.new")
    one = hit(c, "%s.parent.%s" % (W, slug(a["name"])))
    got = (c.rows(W + ".rule.matches") or [{}])[0].get("parent")
    none = hit(c, W + ".parent.none")
    back = (c.rows(W + ".rule.matches") or [{}])[0].get("parent", "?")
    return one and none and got == a["name"] and back is None, "chip answered %s, parent %r; None answered %s, parent %r" % (one, got, none, back)


def w_person(c):
    window(c)
    people = c.rows(W + ".people")
    if not people:
        raise Unreadable("the window lists no person")
    p = people[0]
    clicked = hit(c, "%s.person.%s" % (W, p["id"]))
    rows = c.rows(W + ".contact.header")
    h = rows[0] if rows else {}
    return clicked and h.get("name") == p["name"] and h.get("address") in p["addresses"], \
        "click answered %s, card %r (%r), person %r %s" % (clicked, h.get("name"), h.get("address"), p["name"], p["addresses"])


LOOKALIKE = os.path.join(ROOT, "checks", "corpora", "lookalike-people.json")   # its own corpus: two people at one domain whose names look alike, one stranger


def w_merge(c):
    """The case loads a corpus of its own; the launch sample comes back when it ends (`fixture reset` restores the file the app started on), because the
    cases that follow (the labelling flow's example, the tree menu) read the sample's labels and its 37 picked mails, which the look-alike corpus has none of."""
    try:
        return w_merge_run(c)
    finally:
        try:
            fixture_reset(c)
        except Unreadable:
            pass      # a reset that cannot be confirmed is said by the next case's own reset; it never hides this case's verdict


def w_merge_run(c):
    """The Merge button joins the look-alike. The case loads its OWN corpus (main63: the shared fixture holds no two look-alike people, so the case was
    never judged): two people at linea-supply-example.com plus a stranger. The corpus must show up in the people list first (the control: with the
    corpus unloaded, Merge-did-nothing would read red for the wrong reason); then a card that offers the merge must merge."""
    fixture_reset(c)
    c.send("fixture", "corpus", LOOKALIKE)
    window(c)
    people = c.rows(W + ".people")
    near = [p for p in people if any(a.endswith("@linea-supply-example.com") for a in p["addresses"])]
    if len(near) < 2:
        raise Unreadable("the look-alike corpus is not in the people list (%d of the two look-alikes shown): not judged, not red" % len(near))
    for p in near:
        c.send("click", "%s.person.%s" % (W, p["id"]))
        before = (c.rows(W + ".contact.header") or [{}])[0].get("addresses", [])
        if not hit(c, W + ".contact.merge"):
            continue
        after = (c.rows(W + ".contact.header") or [{}])[0].get("addresses", [])
        left = len(c.rows(W + ".people"))
        return len(after) > len(before) and left == len(people) - 1, \
            "%s: addresses %d -> %d, people %d -> %d" % (p["name"], len(before), len(after), len(people), left)
    return False, "no card of the two look-alikes offered or ran Merge (%d people shown)" % len(people)


WINDOW = [("w-label-row-click-selects-it-and-shows-its-pane", w_select, "select-dead"),
          ("w-rename-door-renames-the-selected-label", w_rename, "rename-dead"),
          ("w-new-inside-door-makes-a-child-of-the-selected-label", w_inside, "inside-at-top"),
          ("w-top-drop-door-lifts-a-nested-label", w_drop_top, "drop-top-dead"),
          ("w-row-drop-door-nests-the-selected-label", w_drop_onto, "drop-onto-dead"),
          ("w-folder-save-adds-it-and-cancel-closes-a-new-one", w_folder_save_cancel, "save-dead"),
          ("w-folder-save-adds-it-and-cancel-closes-a-new-one", w_folder_save_cancel, "cancel-dead"),
          ("w-folder-row-door-opens-the-editor-on-its-rule", w_folder_edit, "edit-dead"),
          ("w-parent-chip-sets-and-clears-the-parent", w_parent_chip, "parent-chip-dead"),
          ("w-cancel-on-a-saved-folder-closes-the-editor", w_folder_cancel_saved, "cancel-saved-keeps-open"),
          ("w-sender-card-row-opens-its-thread", w_sender_row_opens_thread, "row-open-dead"),
          ("w-person-row-opens-the-sender-card", w_person, "person-dead"),
          ("w-merge-button-joins-the-look-alike", w_merge, "merge-dead")]


class FakeWin:
    """The labels window on the wire, for --selftest: the label tree, the folder editor, the people and the sender card. A mutant disables one control."""
    def __init__(self, mutant=None):
        RG.LAUNCH.clear()
        self.m, self.typed, self.pane, self.sel = mutant, "", "labels", None
        self.labels, self.folders, self.card = [], [], None
        self.dn, self.dr, self.new, self.fid = "", "", False, None
        self.surface, self.focus = "mac-main", None
        self.people = [{"id": "p1", "name": "Linea", "addresses": ["orders@linea-supply-example.com"]},
                       {"id": "p2", "name": "Linea Billing", "addresses": ["billing@linea-supply-example.com"]}]
        self.msgs = [{"id": "msg-l1", "thread": "thr-l1", "subject": "Order 1"}, {"id": "msg-l2", "thread": "thr-l2", "subject": "Order 2"}]

    def item(self, i):
        return next((l for l in self.labels if l["id"] == i), None)

    def put(self, name, parent):
        d = 1 if parent is None else self.item(parent)["depth"] + 1
        row = {"id": "lbl-%d" % (len(self.labels) + 1), "name": name, "depth": d, "parent": parent}
        self.labels.append(row)
        self.sel, self.pane = row["id"], "labels"
        return True

    def path(self, l):
        return l["name"] if l["parent"] is None else self.path(self.item(l["parent"])) + " > " + l["name"]

    def send(self, verb, *a):
        if verb == "fixture":      # reset keeps this stand-in's windows as they are; the digest is constant; a corpus changes nothing (its people are in self.people)
            return {"digest": "win", "parts": {}, "view": "v"} if a == ("digest",) else {}
        if verb == "type":
            self.typed = a[0]
            if self.pane == "folder" and self.typed:
                if self.new and not self.dn:
                    self.dn = self.typed
                else:
                    self.dr += self.typed
                self.typed = ""
            return {}
        if verb == "click":
            if not self.click(a[0]):
                raise Unreadable("click %s: did nothing" % a[0])
        return {}

    def click(self, ax):
        tail = ax[len(W) + 1:]
        t, self.typed = self.typed, ""
        mm = self.m
        if tail in ("open", "show.labels"):
            self.pane = "labels" if tail == "show.labels" else self.pane
            return True
        if tail == "labels.create.top":
            return bool(t) and self.put(t, None)
        if tail == "labels.create":
            return bool(t) and self.sel is not None and self.put(t, None if mm == "inside-at-top" else self.sel)
        if tail.startswith("labels.select."):
            if mm == "select-dead" or not self.item(tail.split(".")[-1]):
                return False
            self.sel, self.pane = tail.split(".")[-1], "labels"
            return True
        if tail == "labels.rename":
            if mm == "rename-dead" or not t or self.sel is None:
                return False
            self.item(self.sel)["name"] = t
            return True
        if tail == "labels.drop.top" or tail.startswith("labels.drop."):
            dead = "drop-top-dead" if tail.endswith(".top") else "drop-onto-dead"
            if mm == dead or self.sel is None:
                return False
            tgt = None if tail.endswith(".top") else tail.split(".")[-1]
            me = self.item(self.sel)
            me["parent"], me["depth"] = tgt, 1 if tgt is None else self.item(tgt)["depth"] + 1
            return True
        if tail == "folder.new":
            self.pane, self.new, self.fid, self.dn, self.dr, self.dp = "folder", True, None, "", "", None
            return True
        if tail == "rule.save":
            if mm == "save-dead" or self.pane != "folder":
                return False
            if self.new:
                self.folders.append({"id": "f%d" % (len(self.folders) + 1), "name": self.dn, "rule": self.dr, "parent": self.dp})
                self.new, self.fid = False, self.folders[-1]["id"]
            return True
        if tail == "rule.cancel":
            if mm == "cancel-dead" or self.pane != "folder":
                return False
            if self.new:
                self.pane, self.dn, self.dr = "labels", "", ""
            else:
                f = next((f for f in self.folders if f["id"] == self.fid), None)
                self.dr = f["rule"] if f else self.dr          # a saved folder is reverted to what it was saved as
                if mm != "cancel-saved-keeps-open":
                    self.pane = "labels"
            return True
        if tail.startswith("edit."):
            f = next((f for f in self.folders if f["id"] == tail.split(".")[-1]), None)
            if mm == "edit-dead" or f is None:
                return False
            self.pane, self.new, self.fid, self.dn, self.dr, self.dp = "folder", False, f["id"], f["name"], f["rule"], f["parent"]
            return True
        if tail.startswith("parent."):
            if mm == "parent-chip-dead" or self.pane != "folder":
                return False
            s = tail.split(".", 1)[1]
            l = next((l for l in self.labels if l["parent"] is None and slug(l["name"]) == s), None)
            if s != "none" and l is None:
                return False
            self.dp = None if s == "none" else l["name"]
            return True
        if tail.startswith("person."):
            p = next((p for p in self.people if p["id"] == tail.split(".")[-1]), None)
            if mm == "person-dead" or p is None:
                return False
            self.card, self.pane = p, "contact"
            return True
        if tail.startswith("contact.messages."):
            i = int(tail.split(".")[-1])
            if mm == "row-open-dead" or self.pane != "contact" or i >= len(self.msgs):
                return False
            self.surface, self.focus = "mac-reader", self.msgs[i]["id"]
            return True
        if tail == "contact.merge":
            if mm == "merge-dead" or self.pane != "contact" or len(self.people) < 2:
                return False
            keep, other = self.card, next(p for p in self.people if p is not self.card)
            keep["addresses"] = keep["addresses"] + other["addresses"]
            self.people.remove(other)
            return True
        return False

    def uistate(self):
        return {"surface": self.surface, "focus_message": self.focus}

    def rows(self, ax):
        tail = ax[len(W) + 1:] if ax.startswith(W) else ax
        if tail == "labels":
            return [{"id": l["id"], "name": l["name"], "path": self.path(l), "depth": l["depth"], "parent": l["parent"],
                     "selected": l["id"] == self.sel, "pane": self.pane} for l in self.labels]
        if tail == "rule.matches":
            return [{"matches": 1, "rule": self.dr, "name": self.dn, "parent": self.dp, "new": self.new, "folder_id": self.fid}] if self.pane == "folder" else []
        if tail == "sidebar":
            return [{"id": f["id"], "title": f["name"], "parent": f["parent"], "count": 1} for f in self.folders]
        if tail == "people":
            return [dict(p) for p in self.people]
        if tail == "contact.messages":
            return [{"ax": "%s.contact.messages.%d" % (W, i), "id": m["id"], "subject": m["subject"]} for i, m in enumerate(self.msgs)] if self.pane == "contact" else []
        if tail == "contact.header":
            return [{"name": self.card["name"], "address": self.card["addresses"][0], "addresses": list(self.card["addresses"])}] if self.pane == "contact" and self.card else []
        return []


# ---- the inspector: the controls no case clicked
def fresh(c, name=None):
    reset(c)
    say(c, INS + ".open.new")
    say(c, INS + ".name", name or uname("Parity door"))
    return settle(c)


def accounts(c):
    reset(c)
    ids = sorted({r["account"] for r in c.rows(LIST) if r.get("account")})
    if not ids:
        raise Unreadable("the list shows no account to scope a label to")
    return ids


def commit(c, i, value):
    say(c, "%s.rule.%d.value" % (INS, i), value)
    say(c, "%s.rule.%d.commit" % (INS, i))


def d_scope(c):
    ids = accounts(c)
    fresh(c)
    one = hit(c, "%s.scope.%s" % (INS, ids[0]))
    named = summary(c).get("scope")
    back = hit(c, INS + ".scope.all")
    allm = summary(c).get("scope")
    return one and back and named not in (None, "", "all") and allm == "all", "Applies to %r after an account, %r after All mailboxes" % (named, allm)


def d_account_rule(c):
    ids = accounts(c)
    fresh(c)
    say(c, INS + ".rule.0.kind.account")
    clicked = hit(c, "%s.rule.0.account.%s" % (INS, ids[0]))
    r = c.rows(INS + ".rules")[0]
    return clicked and r["kind"] == "account" and r["values"] == [ids[0]], "account rule values %s after the menu item (click answered %s)" % (r["values"], clicked)


def d_chip_x(c):
    fresh(c)
    say(c, INS + ".rule.0.kind.domain")
    commit(c, 0, "one-example.com")
    commit(c, 0, "two-example.com")
    before = c.rows(INS + ".rules")[0]["values"]
    clicked = hit(c, INS + ".rule.0.chip.0.remove")
    after = c.rows(INS + ".rules")[0]["values"]
    return clicked and before == ["one-example.com", "two-example.com"] and after == ["two-example.com"], "values %s then %s (click answered %s)" % (before, after, clicked)


def d_more(c):
    fresh(c)
    say(c, INS + ".rule.0.kind.domain")
    for i in range(5):
        commit(c, 0, "v%d-example.com" % i)
    r0 = c.rows(INS + ".rules")[0]
    clicked = hit(c, INS + ".rule.0.more")
    r1 = c.rows(INS + ".rules")[0]
    return clicked and (r0["shown"], r0["expanded"]) == (3, False) and (r1["shown"], r1["expanded"]) == (5, True), \
        "shown %s -> %s of 5 (click answered %s)" % (r0["shown"], r1["shown"], clicked)


def d_addvalue(c):
    fresh(c)
    say(c, INS + ".rule.0.kind.domain")
    commit(c, 0, "one-example.com")
    r0 = c.rows(INS + ".rules")[0]
    clicked = hit(c, INS + ".rule.0.addvalue")
    r1 = until(lambda: next((r for r in c.rows(INS + ".rules") if r["field_open"] and r["focused"]), None)) or c.rows(INS + ".rules")[0]
    ok = clicked and r0["field_open"] is False and r1["field_open"] is True and r1["focused"] is True
    detail = "value field open %s -> %s, keys in it %s (click answered %s)" % (r0["field_open"], r1["field_open"], r1["focused"], clicked)
    if ok:
        return True, detail
    return False, detail + evidence(c, click_answered=clicked, rule_before=r0, rule_after_waited=r1)


def d_closest(c):
    fresh(c)
    say(c, INS + ".rule.0.kind.domain")
    say(c, INS + ".rule.0.value", "linea-supply-exmple.com")
    near = settle(c).get("closest")
    clicked = hit(c, INS + ".useclosest")
    s = settle(c)
    r = c.rows(INS + ".rules")[0]
    return clicked and near == "linea-supply-example.com" and r["values"] == [near] and s["count"] > 0 and s.get("closest") is None, \
        "offered %r, rule now %s, %s matches (click answered %s)" % (near, r["values"], s["count"], clicked)


def d_done(c):
    reset(c)
    say(c, "%s.open.modify.%s" % (INS, BOARD))
    s = settle(c)
    foot = s.get("foot")
    clicked = hit(c, INS + ".done")
    return foot == "clean" and clicked and summary(c).get("open") is False, "foot %r, Close answered %s, open %s" % (foot, clicked, summary(c).get("open"))


def offer_for(c):
    """A saved label whose rules moved and Save pressed: the Re-run question is on the foot."""
    fresh(c, uname("Parity offer"))
    say(c, INS + ".rule.0.kind.domain")
    commit(c, 0, "linea-supply-example.com")
    settle(c)
    say(c, INS + ".create")
    if summary(c).get("mode") != "modify":
        raise Unreadable("the label was not created (can_write=%s)" % summary(c).get("can_write"))
    n = len(c.rows(INS + ".rules"))
    say(c, INS + ".rule.add")
    say(c, "%s.rule.%d.kind.domain" % (INS, n))
    say(c, "%s.rule.%d.value" % (INS, n), "provanta-example.be")
    settle(c)
    say(c, INS + ".apply")
    s = summary(c)
    if not s.get("offer"):
        raise Unreadable("Save asked no Re-run question")
    return s


def d_notnow(c):
    s = offer_for(c)
    clicked = hit(c, INS + ".notnow")
    s2 = summary(c)
    return s.get("foot") == "offer" and clicked and s2.get("offer") is None and s2.get("open") is True and c.rows(INS + ".result") == [], \
        "foot %r then offer %s, still open %s, re-run result rows %d (click answered %s)" % (s.get("foot"), s2.get("offer"), s2.get("open"), len(c.rows(INS + ".result")), clicked)


def d_swatches(c):
    fresh(c)
    got = []
    for i in range(6):
        say(c, "%s.colour.%d" % (INS, i))
        got.append(summary(c).get("colour"))
    return all(got) and len(set(got)) == 6, "six swatches gave %s" % got


def key_evidence(c):
    """What decides a key the inspector ignores: the inspector's own row (open, mode, id, can_write, foot, failure, refusal, offer, and `editing` when the
    build serves it) and the shell's focus. `editing` is the model's flag that a text field holds the keys (LabelsContactsMacFeature.swift:33-38 refuse
    R, ?, A and Return while it is true); the row does not serve it, so it is also derived from the focused element. Never raises."""
    out = []
    try:
        r = summary(c)
        out.append("inspector %s" % {f: r.get(f, "absent") for f in ("open", "mode", "label_id", "can_write", "foot", "failure", "refusal", "offer", "editing")})
    except (Unreadable, KeyError, IndexError, TypeError, AttributeError, ValueError) as e:
        out.append("inspector unreadable (%s)" % e)
    try:
        u = c.uistate()
        fr = u.get("first_responder", "absent")
        derived = "absent" if fr == "absent" else ("yes" if str(fr) in ("search-field", "composer", "quick-reply") or str(fr).startswith("other:") else "no")
        out.append("focused field %r, a text field holds the keys: %s (derived from it), surface %r overlay %r" % (fr, derived, u.get("surface", "absent"), u.get("overlay", "absent")))
    except (Unreadable, KeyError, TypeError, AttributeError, ValueError) as e:
        out.append("shell focus unreadable (%s)" % e)
    return "; ".join(out)


def press_key(c, key):
    """Send the key; a refusal ("does nothing") carries the state it met. Returns the state before the key, for a failure line."""
    before = key_evidence(c)
    try:
        c.send("click", INS + ".blur")      # the name field takes the keys of a new label (LabelInspectorView `.onAppear`): letters and Return are its own until it lets go
    except Unreadable:
        pass
    try:
        c.send("key", key)
    except Unreadable as e:
        raise Unreadable("%s | before the key %s: %s" % (e, key, before))
    return before


def after_key(c, ok, detail, before):
    """A failed key case says what the app was in before the key and after it."""
    return ok, detail if ok else "%s | before the key: %s | after the key: %s" % (detail, before, key_evidence(c))


def kd_a(c):
    fresh(c)
    before = press_key(c, "A")
    chains = [r["chain"] for r in c.rows(INS + ".flow") if "chain" in r]
    return after_key(c, "other1" in chains, "chains after A: %s" % chains, before)


def kd_question(c):
    fresh(c)
    before = press_key(c, "?")
    on = c.rows(INS + ".foot")[0].get("why_open")
    press_key(c, "?")
    off = c.rows(INS + ".foot")[0].get("why_open")
    return after_key(c, on is True and off is False, "? opens Why here %s, again closes it %s" % (on, off is False), before)


def kd_return(c):
    fresh(c)
    say(c, INS + ".rule.0.kind.domain")
    commit(c, 0, "linea-supply-example.com")
    settle(c)
    before = press_key(c, "Enter")
    s = summary(c)
    return after_key(c, s.get("mode") == "modify" and bool(s.get("label_id")), "Return on a new label: mode %s, id %s" % (s.get("mode"), s.get("label_id")), before)


def kd_r(c):
    offer_for(c)
    before = press_key(c, "R")
    for _ in range(30):         # the re-run answers off the main thread: read until its rows are there, bounded
        s = summary(c)
        if s.get("offer") is None and c.rows(INS + ".result"):
            break
        time.sleep(0.1)
    n = len(c.rows(INS + ".result"))
    return after_key(c, s.get("offer") is None and n > 0, "R: offer %s, re-run result rows %d" % (s.get("offer"), n), before)


def kd_esc_offer(c):
    offer_for(c)
    c.send("key", "Escape")
    s = summary(c)
    return s.get("offer") is None and s.get("open") is True and c.rows(INS + ".result") == [], "Esc on the question: offer %s, inspector open %s" % (s.get("offer"), s.get("open"))


DOORS_IN = [("d-applies-to-menu-items-set-the-scope", d_scope, "scope-dead"),
            ("d-account-rule-menu-adds-the-account", d_account_rule, "account-adder-dead"),
            ("d-value-chip-x-takes-the-value-off", d_chip_x, "chip-x-dead"),
            ("d-plus-n-more-shows-every-value", d_more, "more-dead"),
            ("d-plus-gives-the-value-field-back", d_addvalue, "addvalue-dead"),
            ("d-did-you-mean-use-it-swaps-in-the-near-domain", d_closest, "useclosest-dead"),
            ("d-close-button-closes-a-saved-label", d_done, "done-dead"),
            ("d-not-now-keeps-todays-filing", d_notnow, "notnow-dead"),
            ("d-six-swatches-set-six-colours", d_swatches, "swatch-4-dead"),
            ("kd-key-a-adds-the-other-rule", kd_a, "a-key-dead"),
            ("kd-key-question-mark-opens-and-closes-why-here", kd_question, "q-key-dead"),
            ("kd-key-return-creates-the-new-label", kd_return, "return-dead"),
            ("kd-key-r-reruns-the-offer", kd_r, "r-key-dead"),
            ("kd-esc-on-the-offer-is-not-now", kd_esc_offer, "esc-offer-closes")]


def lev(a, b):
    p = list(range(len(b) + 1))
    for i, x in enumerate(a, 1):
        q = [i]
        for j, y in enumerate(b, 1):
            q.append(min(p[j] + 1, q[j - 1] + 1, p[j - 1] + (x != y)))
        p = q
    return p[-1]


class FakeDoors(FakeInspector):
    """The inspector's small controls on the wire, on top of FakeInspector: the Applies-to scope, the rule row's chips, "+N more" and "+", the
    nearest-domain offer, the foot's Close and Not now, the swatches and the keys A ? Return R Esc. A mutant disables one control."""
    ACCOUNTS = ["acc-a", "acc-b"]

    def __init__(self, mutant=None):
        super().__init__(mutant)
        self.scope, self.focus, self.why, self.others = None, None, False, 0

    def reset_draft(self):
        super().reset_draft()
        self.scope, self.focus, self.why, self.others = None, None, False, 0
        for r in self.d["rules"]:
            r["expanded"] = False

    def conds(self, rules):
        keep = (lambda v: True) if self.m == "blank-matches-all" else (lambda v: bool(v.strip()))
        return [(r["kind"], v) for r in rules for v in r["values"] + ([r["pending"]] if r["pending"] else []) if keep(v)]

    def hit(self, c, msg):
        if self.m == "blank-matches-all" and not c[1].strip():
            return True   # the contract's `has` with an empty needle
        return super().hit(c, msg)

    def engine_preview(self):
        if not self.offer:
            return None
        joins = [m for m in self.matches(self.d["rules"]) if m[0] not in self.filed]
        by_rule = len(self.CORPUS) if self.m == "preview-counts-draft" else len(joins)
        return {"by_rule": by_rule, "by_ai": 0, "leave": 0, "stay": len(self.filed), "ai_words": "" if self.m == "preview-counts-draft" else "The AI cannot sort yet: no model on this Mac."}

    def dirty(self):
        sig = lambda rs: [[r["kind"]] + r["values"] + ([r["pending"]] if r["pending"] else []) for r in rs]
        return self.mode == "modify" and sig(self.d["rules"]) != sig(self.saved_rules)

    def foot(self):
        return "offer" if self.offer else ("create" if self.mode == "new" else ("apply" if self.dirty() else "clean"))

    def shown(self, r):
        return len(r["values"]) if r.get("expanded") or len(r["values"]) <= 3 else 3

    def field_open(self, i):
        r = self.d["rules"][i]
        return r["kind"] != "account" and (not r["values"] or self.focus == i or bool(r["pending"]))

    def closest(self):
        r = next((r for r in self.d["rules"] if r["kind"] == "domain" and (r["values"] or r["pending"])), None)
        if r is None or self.matches(self.d["rules"]):
            return None
        typed = (r["values"] + ([r["pending"]] if r["pending"] else []))[-1]
        near = [m[1] for m in self.CORPUS if m[1] != typed and lev(m[1], typed) <= 2]
        return near[0] if near else None

    def uistate(self):
        # mutant editing-stuck: the model's `editing` stays true after Save/Create, so the keys refuse (the reader's candidate), and a field keeps the focus
        return {"surface": "mac-main", "overlay": None, "first_responder": "other:mac-labels.inspector.name" if self.m == "editing-stuck" else "list"}

    def send(self, verb, *a):
        if verb == "key" and self.open and self.m == "editing-stuck" and a[0] in ("A", "?", "Enter", "R"):
            raise Unreadable("key %s: the key %s does nothing on mac-main" % (a[0], a[0]))
        if verb == "key" and self.open:
            k = a[0]
            if k == "A" and self.m != "a-key-dead":
                self.others += 1
                return {}
            if k == "?" and self.m != "q-key-dead":
                self.why = not self.why
                return {}
            if k == "Enter" and self.m != "return-dead" and self.mode == "new" and self.d["name"]:
                return super().send("click", INS + ".create")
            if k == "R" and self.offer and self.m != "r-key-dead":
                return super().send("click", INS + ".rerun")
            if k == "Escape" and self.offer and self.m == "esc-offer-closes":
                self.open, self.mode, self.offer = False, "closed", None
                return {}
        return super().send(verb, *a)

    def click(self, ax):
        tail = ax.split(INS + ".")[-1] if INS in ax else ax
        d = self.d
        parts = tail.split(".") + ["", "", "", "", ""]
        if tail.startswith("scope."):
            self.typed = ""
            if self.m != "scope-dead":
                self.scope = None if parts[1] == "all" else parts[1]
        elif tail.startswith("rule.") and parts[2] == "account" and parts[3] and not parts[4]:
            self.typed = ""
            r = d["rules"][int(parts[1])]
            if self.m != "account-adder-dead" and r["kind"] == "account" and parts[3] not in r["values"]:
                r["values"].append(parts[3])
        elif tail.startswith("rule.") and parts[2] == "more":
            self.typed = ""
            r = d["rules"][int(parts[1])]
            if self.m != "more-dead" and not r.get("expanded") and len(r["values"]) > 3:
                r["expanded"] = True
        elif tail.startswith("rule.") and parts[2] == "addvalue":
            self.typed = ""
            i = int(parts[1])
            if self.m != "addvalue-dead" and d["rules"][i]["kind"] != "account" and not self.field_open(i):
                self.focus = i
        elif tail.startswith("rule.") and parts[2] == "chip" and parts[4] == "remove":
            self.typed = ""
            r = d["rules"][int(parts[1])]
            j = int(parts[3])
            if self.m != "chip-x-dead" and j < self.shown(r):
                v = r["values"][j]
                r["values"] = [x for x in r["values"] if x != v]
        elif tail == "useclosest":
            self.typed = ""
            near = self.closest()
            if self.m != "useclosest-dead" and near:
                r = next(r for r in d["rules"] if r["kind"] == "domain" and (r["values"] or r["pending"]))
                r["values"], r["pending"] = [near], ""
        elif tail == "done":
            self.typed = ""
            if self.m != "done-dead" and self.foot() == "clean":
                self.open, self.mode = False, "closed"
        elif tail == "notnow":
            self.typed = ""
            if self.m != "notnow-dead" and self.offer:
                self.offer = None
        elif tail.startswith("colour.") and tail.endswith(".4") and self.m == "swatch-4-dead":
            self.typed = ""
        elif tail == "create" and d["name"]:
            super().click(ax)
            self.saved_rules = [dict(r, values=list(r["values"])) for r in d["rules"]]
        else:
            return super().click(ax)

    def rows(self, ax):
        out = super().rows(ax)
        if ax == INS and out:
            ep = self.engine_preview()
            if out[0].get("offer") and ep:
                out[0]["offer"] = dict(out[0]["offer"], join=ep["by_rule"] + ep["by_ai"] + (1 if self.m == "preview-disagrees" else 0))
            # the Re-run preview's head line: with no model it counts 0 joins by the AI (never "the AI rules on each after Save"); the broken copy keeps the promise
            out[0]["preview_line"] = (("%d picked · the AI rules on each after Save" % ep["by_rule"]) if self.m == "preview-promises-the-ai"
                                      else ("%d picked · %d by the AI" % (ep["by_rule"], ep["by_ai"]))) if ep else None
            out[0].update(engine_preview=ep, foot=self.foot(), scope=("all" if self.scope is None else "Account " + self.scope), closest=self.closest())
        elif ax == INS + ".rules":
            for i, r in enumerate(out):
                src = self.d["rules"][i]
                if self.m != "blank-matches-all":
                    r["values"] = [v for v in r["values"] if v.strip()]
                r.update(expanded=bool(src.get("expanded")), shown=self.shown(src), field_open=self.field_open(i), focused=self.focus == i)
        elif ax == INS + ".flow":
            return [{"chain": "entry"}] + [{"chain": "other%d" % (n + 1)} for n in range(self.others)] + [{"chain": "exit"}]
        elif ax == INS + ".foot":
            return [{"why_open": self.why}]
        elif ax == LIST:
            return [{"id": "m1", "account": self.ACCOUNTS[0]}, {"id": "m2", "account": self.ACCOUNTS[1]}]
        return out


def kd_close(c):
    focus_on(c, "msg-08")
    c.send("key", "V")
    opened = c.uistate().get("overlay") == "move"
    before = assigned(c, "msg-08")
    clicked = hit(c, "mac-labels.move.close")
    return opened and clicked and c.uistate().get("overlay") is None and assigned(c, "msg-08") == before, \
        "picker open %s, Esc door answered %s, overlay now %s, labels unchanged %s" % (opened, clicked, c.uistate().get("overlay"), assigned(c, "msg-08") == before)


KEYS.append(("kd-move-picker-esc-door-closes-it-and-moves-nothing", kd_close, "close-door-dead"))


# ---- A125: the Re-run preview and the foot are one computation (state label/<l>/rerun/preview), and a blank rule value matches nothing
NO_MODEL = "The AI cannot sort yet: no model on this Mac."


def engine(c):
    r = settle(c)       # the proof (and the core's preview in it) is computed off the main thread after Save: read it when the inspector says it is done
    e = r.get("engine_preview")
    if not e:
        raise Unreadable("the core serves no label/<l>/rerun/preview (an older core) | the row says engine_preview %r for label %r (mode %r, offer %r); "
                         "not staged: the Rust engine serves that path (core/crates/engine/src/flow_ctx.rs:259) and the fixture core mirrors it (rerunPreviewLocked, Sources/CoreBridge/FixtureAgents.swift) for a SAVED label; "
                         "this row held none, so the label is not saved or this build's core is older: the missing piece is a core that serves `label/<l>/rerun/preview`"
                         % (e, r.get("label_id", "absent"), r.get("mode", "absent"), r.get("offer", "absent")))
    return e


def a_no_model(c):
    """A saved label with a description, a rule that matches nothing, no model on this Mac: Save, and the question says 0 join with the engine's line."""
    fresh(c, uname("Parity A125"))
    say(c, INS + ".meaning", "supplier quotes")
    say(c, INS + ".rule.0.kind.domain")
    commit(c, 0, "nothing-matches-this.invalid")
    settle(c)
    say(c, INS + ".create")
    say(c, INS + ".meaning", "supplier quotes and invoices")
    say(c, INS + ".apply")
    e = engine(c)
    s = settle(c)       # the row is read AFTER the proof landed: the first read (before it) holds no preview, so no head line (main111: "the preview line says None")
    offer = s.get("offer") or {}
    line = s.get("preview_line")
    # the preview line follows the refusal: it counts the picks and 0 joins by the AI and promises nothing the card denies ("the AI rules on each after Save" is for an AI that can rule)
    return (e["by_rule"], e["by_ai"], e["ai_words"], offer.get("join"), line) == (0, 0, NO_MODEL, 0, "0 picked · 0 by the AI"), \
        "engine preview %s, the question says %s join, the preview line says %r (want '0 picked · 0 by the AI')" % (e, offer.get("join"), line)


def a_blank(c):
    fresh(c, uname("Parity blank"))
    say(c, INS + ".rule.0.kind.subject")
    say(c, INS + ".rule.0.value", "   ")
    s = settle(c)
    r = c.rows(INS + ".rules")[0]
    return s["count"] == 0 and s["matches"] == 0 and r["values"] == [] and r["count"] in (0, None), \
        "a blank Subject value: %s matches, %s rows, rule values %s, rule count %s" % (s["count"], s["matches"], r["values"], r["count"])


def a_agree(c):
    s = offer_for(c)
    e = engine(c)
    offer = s["offer"]
    joined_before = e["by_rule"] + e["by_ai"]
    say(c, INS + ".rerun")
    res = [r for r in c.rows(INS + ".result") if r.get("id")]
    return offer["join"] == joined_before and (e["by_ai"] != 0 or len(res) == e["by_rule"]), \
        "offer says %s join, engine %s by rule + %s by the AI; the re-run then filed %d" % (offer["join"], e["by_rule"], e["by_ai"], len(res))


DOORS_IN += [("a125-no-model-and-no-matching-rule-says-0-join-and-the-line", a_no_model, "preview-counts-draft"),
             ("a125-no-model-and-no-matching-rule-says-0-join-and-the-line", a_no_model, "preview-promises-the-ai"),
             ("a125-a-blank-rule-value-matches-nothing", a_blank, "blank-matches-all"),
             ("a125-the-question-and-the-run-agree", a_agree, "preview-disagrees")]


# ---- doors-labels: every control of the window and the inspector, its click door and the control it stands for, both read from the Swift sources.
# (label, door file, door text, control file, control text); the label is the control's name in docs/COVERAGE.md "Labels, contacts and Add account (lane doors-labels)".
LC = "Sources/Features/LabelsContactsMac/"
F_FEAT, F_SOCK, F_FLOW, F_RULE, F_MODEL = "LabelsContactsMacFeature.swift", "LabelInspectorSocket.swift", "LabelFlowSocket.swift", "LabelRuleDoors.swift", "LabelsContactsModel.swift"
DOORS = [
    ("Window: label row click", F_FEAT, r'"\(ax).labels.select.\(l.id)"', "LabelsSidebar.swift", r".onTapGesture { _ = model.selectLabel(l.id) }"),
    ("Window: label row drag onto a row", F_FEAT, r'"\(ax).labels.drop.\(l.id)"', "LabelsSidebar.swift", "DragGesture(minimumDistance: 6"),
    ("Window: Labels header drop (top)", F_FEAT, r'"\(ax).labels.drop.top"', "LabelsSidebar.swift", r'.whooshAx("mac-labels-contacts.labels.top")'),
    ("Window: rename field", F_FEAT, r'"\(ax).labels.rename"', "LabelDetail.swift", 'ax: "rename"'),
    ("Window: new-inside field", F_FEAT, r'"\(ax).labels.create"] = { m in', "LabelDetail.swift", 'ax: "create"'),
    ("Window: smart folder +", F_FEAT, r'"\(ax).folder.new"', "LabelsSidebar.swift", 'accessibilityIdentifier("mac-labels-contacts.folder.new")'),
    ("Window: smart folder row", F_FEAT, r't["\(ax).edit.\(f.id)"]', "LabelsSidebar.swift", "_ = model.edit(f.id)"),
    ("Window: person row", F_FEAT, r't["\(ax).person.\(p.id)"]', "LabelsSidebar.swift", r'accessibilityIdentifier("mac-labels-contacts.person.\(p.id)")'),
    ("Window: editor name field", F_MODEL, "draftName = m.typed", "SmartFolderEditor.swift", 'accessibilityIdentifier("mac-labels-contacts.folder.name")'),
    ("Window: editor rule field", F_MODEL, "draftRule += m.typed", "SmartFolderEditor.swift", 'accessibilityIdentifier("mac-labels-contacts.rule.field")'),
    ("Window: parent chips", F_FEAT, r't["\(ax).parent.\(LabelsContactsSource.slug(n.name))"]', "SmartFolderEditor.swift", r'accessibilityIdentifier("mac-labels-contacts.parent.\(value'),
    ("Window: editor Cancel", F_FEAT, r'"\(ax).rule.cancel"', "SmartFolderEditor.swift", 'accessibilityIdentifier("mac-labels-contacts.rule.cancel")'),
    ("Window: editor Save", F_FEAT, r'"\(ax).rule.save"', "SmartFolderEditor.swift", 'accessibilityIdentifier("mac-labels-contacts.rule.save")'),
    ("Window: Merge", F_FEAT, r'"\(ax).contact.merge"', "ContactCardView.swift", 'accessibilityIdentifier("mac-labels-contacts.contact.merge")'),
    ("Window: Esc and Return", F_FEAT, '"Escape": { m in', "SmartFolderEditor.swift", ".keyboardShortcut(.cancelAction)"),
    ("Inspector: toolbar button", F_SOCK, r'"\(ax).toolbar"', "LabelInspectorView.swift", 'accessibilityIdentifier("mac-labels.inspector.toolbar")'),
    ("Inspector: key I (command)", F_FEAT, '"⌘I": { _ in LabelInspector.toggle() }', "LabelInspectorView.swift", '" · ⌘I · Esc"'),
    ("Inspector: name field", F_SOCK, r'"\(ax).name"', "LabelInspectorBlocks.swift", 'accessibilityIdentifier("mac-labels.inspector.name")'),
    ("Inspector: Inside menu items", F_SOCK, r'"\(ax).inside.top"', "LabelInspectorBlocks.swift", 'accessibilityIdentifier("mac-labels.inspector.inside.top")'),
    ("Inspector: Colour swatches", F_SOCK, r't["\(ax).colour.\(i)"]', "LabelInspectorParts.swift", r'accessibilityIdentifier("mac-labels.inspector.colour.\(hex ?? "none")")'),
    ("Inspector: Applies to items", F_SOCK, r't["\(ax).scope.\(a.id)"]', "LabelInspectorBlocks.swift", r'accessibilityIdentifier("mac-labels.inspector.scope.\(a.id)")'),
    ("Inspector: Pinned switch", F_SOCK, r'"\(ax).pinned"', "LabelInspectorBlocks.swift", 'accessibilityIdentifier("mac-labels.inspector.pinned")'),
    ("Inspector: description editor", F_SOCK, r'"\(ax).meaning"', "LabelFlowAi.swift", 'accessibilityIdentifier("mac-labels.inspector.meaning")'),
    ("Inspector: card switches", F_FLOW, r'"\(ax).flow.entry.switch"', "LabelFlowCards.swift", r'accessibilityIdentifier("\(LabelGeo.base).flow.\(key).switch")'),
    ("Inspector: AI reads ticks", F_FLOW, r'"\(ax).flow.ai.reads.subject"', "LabelFlowAi.swift", r'accessibilityIdentifier("\(LabelGeo.base).flow.ai.reads.\(id)")'),
    ("Inspector: AI reads tick, rest of the thread", F_FLOW, r'"\(ax).flow.ai.reads.rest_of_thread"', "LabelFlowAi.swift", 'ReadTick(title: Words.text("Rest of the thread")'),
    ("Window: sender card message row", F_FEAT, r't["\(ax).contact.messages.\(i)"]', "ContactCardView.swift", ".onTapGesture { onOpen?() }"),
    ("Inspector: Open Decisions", F_SOCK, r'"\(ax).decisions"', "LabelProofRow.swift", r'accessibilityIdentifier("mac-labels.inspector.\(door.id)")'),
    ("Inspector: Download the decision model", F_SOCK, r'"\(ax).laya.download"', "LabelProofRow.swift", r'model.runRefusalDoor(door.id)'),
    ("Inspector: Open System Settings (Apple Intelligence)", F_SOCK, r'"\(ax).apple.settings"', "LabelProofRow.swift", r'model.runRefusalDoor(door.id)'),
    ("Inspector: Exit fallback items", F_FLOW, r'"\(ax).flow.exit.fallback.primary"', "LabelFlowCards.swift", r'accessibilityIdentifier("\(LabelGeo.base).flow.exit.fallback.primary")'),
    ("Inspector: All / Any menu", F_FLOW, r't["\(prefix).match.all"]', "LabelChainView.swift", r'accessibilityIdentifier("\(ids).match.all")'),
    ("Inspector: fact states", F_FLOW, r'"\(prefix).fact.\(messageFactKey(fact: fact)).\(word)"', "LabelFactsGrid.swift", r'accessibilityIdentifier("\(id).\(key(s))")'),
    ("Inspector: Add rule", F_FLOW, r't["\(prefix).rule.add"]', "LabelChainView.swift", r'accessibilityIdentifier("\(ids).rule.add")'),
    ("Inspector: Other rule (and key A)", F_FLOW, r'"\(ax).other.add"', "LabelChainView.swift", r'accessibilityIdentifier("\(LabelGeo.base).other.add")'),
    ("Inspector: remove an other rule", F_FLOW, r'"\(ax).other\(n).remove"', "LabelChainView.swift", r'accessibilityIdentifier("\(ids).remove")'),
    ("Inspector: rule condition menu", F_FLOW, r't["\(prefix).rule.\(i).op.\(o.rawValue)"]', "LabelInspectorRules.swift", r'accessibilityIdentifier("\(ids).op.\(o.rawValue)")'),
    ("Inspector: rule kind menu", F_FLOW, r'"\(prefix).rule.\(i).kind.\(k.rawValue)"', "LabelInspectorRules.swift", r'accessibilityIdentifier("\(ids).kind.\(k.rawValue)")'),
    ("Inspector: rule value field", F_FLOW, r'"\(prefix).rule.\(i).value"', "LabelInspectorRules.swift", r'accessibilityIdentifier("\(ids).value")'),
    ("Inspector: rule value Return", F_FLOW, r'"\(prefix).rule.\(i).commit"', "LabelInspectorRules.swift", "model.commitPending(ref, rule.id)"),
    ("Inspector: rule remove", F_FLOW, r'"\(prefix).rule.\(i).remove"', "LabelInspectorRules.swift", r'accessibilityIdentifier("\(ids).remove")'),
    ("Inspector: rule account menu", F_FLOW, r'"\(prefix).rule.\(i).account.\(a.id)"', "LabelInspectorRules.swift", r'accessibilityIdentifier("\(ids).account.\(a.id)")'),
    ("Inspector: value chip x", F_RULE, r'.chip.\(j).remove"', "LabelInspectorRules.swift", r'.help(Words.text("Remove %@", v))'),
    ("Inspector: +N more", F_RULE, r'.rule.\(i).more"', "LabelInspectorRules.swift", '.help(Words.text("Show every value"))'),
    ("Inspector: + add a value", F_RULE, r'.rule.\(i).addvalue"', "LabelInspectorRules.swift", r'accessibilityIdentifier("\(ids).addvalue")'),
    ("Inspector: drop zone", F_SOCK, r'"\(ax).drop"', "LabelInspectorLive.swift", 'accessibilityIdentifier("mac-labels.inspector.drop")'),
    ("Inspector: Did you mean, Use it", F_SOCK, r'"\(ax).useclosest"', "LabelProofRow.swift", 'accessibilityIdentifier("mac-labels.inspector.useclosest")'),
    ("Inspector: Matches / Mailbox", F_SOCK, r'"\(ax).switch.matches"', "LabelProofViews.swift", r'accessibilityIdentifier("mac-labels.inspector.switch.\(id)")'),
    ("Inspector: Why here (and key ?)", F_FLOW, r'"\(ax).why"', "LabelInspectorFoot.swift", 'accessibilityIdentifier("mac-labels.inspector.why")'),
    ("Inspector: Close", F_SOCK, r'"\(ax).done"', "LabelInspectorFoot.swift", 'accessibilityIdentifier("mac-labels.inspector.done")'),
    ("Inspector: Cancel", F_SOCK, r'"\(ax).cancel"', "LabelInspectorFoot.swift", 'accessibilityIdentifier("mac-labels.inspector.cancel")'),
    ("Inspector: Create (and Return)", F_SOCK, r'"\(ax).create"', "LabelInspectorFoot.swift", 'accessibilityIdentifier("mac-labels.inspector.create")'),
    ("Inspector: Save", F_SOCK, r'"\(ax).apply"', "LabelInspectorFoot.swift", 'accessibilityIdentifier("mac-labels.inspector.apply")'),
    ("Inspector: Not now (and Esc)", F_SOCK, r'"\(ax).notnow"', "LabelInspectorFoot.swift", 'accessibilityIdentifier("mac-labels.inspector.notnow")'),
    ("Inspector: Re-run (and key R)", F_SOCK, r'"\(ax).rerun"', "LabelInspectorFoot.swift", 'accessibilityIdentifier("mac-labels.inspector.rerun")'),
    ("Move picker: label rows", "LabelDoors.swift", r't["\(ax).\(l.id)"]', "LabelMovePicker.swift", r'accessibilityIdentifier("mac-labels.move.\(r.label.id)")'),
    ("Move picker: filter field", "LabelDoors.swift", r't["\(ax).filter"]', "LabelMovePicker.swift", 'TextField(Words.text("Move to"), text: $state.query)'),
    ("Move picker: Esc", "LabelDoors.swift", r't["\(ax).close"]', "LabelMovePicker.swift", ".onKeyPress(.escape)"),
]


def door_text(root, entry):
    """(door source, control source) of one entry, read from the tree at `root`."""
    _, dfile, _, cfile, _ = entry
    rd = lambda f: open(os.path.join(root, LC, f), encoding="utf-8").read()
    return rd(dfile), rd(cfile)


def door_gap(root, entry, drop=None):
    """Which half of an entry is absent: [] when the door and its control are both there. `drop` removes one half first (the mutant)."""
    _, _, dtext, _, ctext = entry
    d, c = door_text(root, entry)
    if drop == "door":
        d = d.replace(dtext, "")
    if drop == "control":
        c = c.replace(ctext, "")
    return [w for w, src, t in (("door", d, dtext), ("control", c, ctext)) if t not in src]


# ---------------------------------------------------------------------------------------------------------------------
# triage-keys: « Pourquoi ici ? » says the truth when the sorting model cannot answer. The core's `message/<id>/filing` carries `ai`
# ("refused" with `ai_refusal`, "no_provider", "answered", ...); the inspector's foot says in plain words that the AI could not sort the
# mail and why, in the engine's own words, never "sorted by AI". Two checks: the screen (needs a core that serves the filing: the fixture
# does not, so it is NOT JUDGED there) and the Swift decision itself (the real function compiled alone and fed every `ai` word).
FOOT_SRC = os.path.join(ROOT, "Sources", "Features", "LabelsContactsMac", "LabelInspectorFoot.swift")
UNSORTED_WORDS = "The AI could not sort this mail: %s"


def f_unsorted_screen(c):
    """On a mail the core filed with `ai: refused`, opening Why here shows the refusal sentence with the engine's words in the foot."""
    reset(c)
    ids = [r["id"] for r in c.rows(LIST)]

    def scan():
        seen, refused = 0, None
        for i in ids:
            try:
                f = c.core("message/%s/filing" % i)
            except Unreadable:
                f = None
            if f:
                seen += 1
                if f.get("ai") == "refused" and f.get("ai_refusal"):
                    return seen, (i, f)
        return seen, refused
    seen, refused = scan()
    if seen and refused is None and ids:   # the fixture core: put one mail in the refused state (`fixture filing-refuse`), as the real core's model would refuse it
        c.send("fixture", "filing-refuse", ids[0], "The writing model is not installed on this Mac.")
        seen, refused = scan()
    if not seen:
        raise Unreadable("this core serves no message/<id>/filing for any listed mail (the fixture core does not): judged on the real core only")
    if refused is None:
        raise Unreadable("no listed mail has ai: refused with words: nothing to judge on this core")
    mid, f = refused
    c.send("click", "%s.row.%d" % (LIST, ids.index(mid)))
    say(c, "%s.open.modify.%s" % (INS, BOARD))
    settle(c)
    say(c, INS + ".why")
    foot = c.rows(INS + ".foot")[0]
    if "unsorted" not in foot:
        raise Unreadable("the foot row has no `unsorted` field: the socket's foot row (LabelFlowSocket.swift) is not served by this lane; the line is drawn as mac-labels.inspector.unsorted")
    line = foot["unsorted"]
    say(c, INS + ".why")
    want = UNSORTED_WORDS % f["ai_refusal"]
    return line == want and "sorted by AI" not in (line or ""), "message %s filed ai: refused (%r): the foot says %r, want %r" % (mid, f["ai_refusal"], line, want)


def unsorted_swift_cases(src=None):
    """Compile LabelInspectorFoot.unsorted alone (stubs for Words and StoredRules) and feed it every `ai` word; returns [(input, output)]."""
    import re
    import shutil
    import subprocess
    import tempfile
    text = src if src is not None else open(FOOT_SRC, encoding="utf-8").read()
    a = text.index("static func unsorted(")
    depth, i = 0, text.index("{", a)
    j = i
    while True:
        depth += {"{": 1, "}": -1}.get(text[j], 0)
        if depth == 0:
            break
        j += 1
    fn = text[a:j + 1]
    cases = ['{"ai":"refused","ai_refusal":"The writing model is not installed."}', '{"ai":"refused","ai_refusal":"  "}', '{"ai":"refused","ai_refusal":null}',
             '{"ai":"no_provider","ai_refusal":null}', '{"ai":"answered","ai_refusal":null}', '{"ai":"not_reached"}', '{"ai":"configured"}', "not json", ""]
    harness = ('import Foundation\n'
               'enum Words { static func text(_ f: String, _ a: CVarArg...) -> String { String(format: f, arguments: a) } }\n'
               'enum StoredRules { static func object(_ json: String?) -> Any? { guard let json, let d = json.data(using: .utf8) else { return nil }; return try? JSONSerialization.jsonObject(with: d) } }\n'
               'enum LabelInspectorFoot {\n    ' + fn + '\n}\n'
               'for a in CommandLine.arguments.dropFirst() { print(LabelInspectorFoot.unsorted(a.isEmpty ? nil : a) ?? "nil") }\n')
    base = os.environ.get("WHOOSH_CHECK_TMP") or None
    work = tempfile.mkdtemp(prefix="whoosh-unsorted-", dir=base)
    try:
        path = os.path.join(work, "main.swift")
        open(path, "w", encoding="utf-8").write(harness)
        built = subprocess.run(["swiftc", "-swift-version", "6", "-o", os.path.join(work, "run"), path], capture_output=True, text=True, timeout=240)
        if built.returncode != 0:
            raise Unreadable("the extracted function does not compile: %s" % built.stderr.strip()[-300:])
        ran = subprocess.run([os.path.join(work, "run")] + [x if x else "" for x in cases], capture_output=True, text=True, timeout=60)
        if ran.returncode != 0:
            raise Unreadable("the harness crashed: %s" % ran.stderr.strip()[-200:])
        out = ran.stdout.split("\n")[:len(cases)]
        return list(zip(cases, out))
    finally:
        shutil.rmtree(work, ignore_errors=True)


UNSORTED_EXPECT = {'{"ai":"refused","ai_refusal":"The writing model is not installed."}': "The AI could not sort this mail: The writing model is not installed.",
                   '{"ai":"no_provider","ai_refusal":null}': "No AI is set up to sort this mail."}


def f_unsorted_swift(c, src=None):
    """Every `ai` word the engine serves, through the real Swift function: refused with words says why, no_provider says no AI is set up, and
    answered, not_reached, configured, a refusal with no words or no read at all say nothing (the line is not drawn)."""
    got = unsorted_swift_cases(src)
    wrong = [(a, o) for a, o in got if o != UNSORTED_EXPECT.get(a, "nil")]
    return not wrong, "%d inputs through the compiled LabelInspectorFoot.unsorted; wrong: %s" % (len(got), wrong[:3])


class FakeFiling:
    """The inspector's foot and one refused mail, for --selftest: Why here opens the line when the filing says ai: refused."""
    WORDS = "The writing model is not installed."

    def __init__(self, mutant=None):
        self.m, self.why, self.at = mutant, False, 0

    def send(self, verb, *a):
        if verb == "click":
            if a[0].startswith(LIST + ".row."):
                self.at = int(a[0].rsplit(".", 1)[1])
            elif a[0] == INS + ".why":
                self.why = not self.why
        return {"ok": True}

    def rows(self, ax):
        if ax == LIST:
            return [{"id": "m0"}, {"id": "m1"}]
        if ax == INS:
            return [{"open": True, "counting": False}]
        if ax == INS + ".foot":
            line = None
            if self.why and self.at == 1:
                line = {"refusal-hidden": None, "says-sorted-by-ai": "Sorted by AI", "words-dropped": "The AI could not sort this mail: "}.get(self.m, UNSORTED_WORDS % self.WORDS)
            return [{"why_open": self.why, "unsorted": line}]
        raise Unreadable("the stand-in has no region %s" % ax)

    def core(self, path):
        if path == "message/m1/filing":
            return {"ai": "refused", "ai_refusal": self.WORDS, "card": "hand"}
        if path == "message/m0/filing":
            return {"ai": "answered", "ai_refusal": None, "card": "ai"}
        raise Unreadable("no path %s" % path)


FILING = [("why-here-says-the-ai-could-not-sort", f_unsorted_screen, "refusal-hidden"),
          ("why-here-says-the-ai-could-not-sort", f_unsorted_screen, "says-sorted-by-ai"),
          ("why-here-says-the-ai-could-not-sort", f_unsorted_screen, "words-dropped"),
          ("why-here-the-swift-line-follows-the-filing", f_unsorted_swift, "swift:refusal-words-dropped"),
          ("why-here-the-swift-line-follows-the-filing", f_unsorted_swift, "swift:no-provider-says-sorted"),
          ("why-here-the-swift-line-follows-the-filing", f_unsorted_swift, "swift:blank-refusal-worded")]


def filing_mutated(src):
    return {"swift:refusal-words-dropped": src.replace('Words.text("The AI could not sort this mail: %@", why)', 'Words.text("Sorted by AI")', 1),
            "swift:no-provider-says-sorted": src.replace('Words.text("No AI is set up to sort this mail.")', 'Words.text("Sorted by AI")', 1),
            "swift:blank-refusal-worded": src.replace(", !why.isEmpty else { return nil }", " else { return nil }", 1)}



# ---------------------------------------------------------------- lane miss-labels-cases: two open items of the labels, one case per clause
# Item "Prove the labelling flow with the À répondre example" (step a94-flow-example): on a build, hidden: for the label "À répondre" SPEC A140's three
# steps read 37 / 32 / 24 / 8: step 1 picks 37 (the 32 rows of the file plus 5 corpus mails that open "Hi Assem"), the AI accepts 32 and 5 still wait for
# its ruling, step 3 keeps 24, 8 go to Primary, and a reply moves one message out of the label (kept 23, to Primary 9, picks and accepts unchanged).
# The figures come from the file (example_figures); the case ids were a94-the-entry-rule-catches-1 / -the-ai-sorts-31 / -the-exit-filter-keeps-23 / -8-go-to-primary
# / -a-reply-moves-one-message-out-of-the-label before A140 (the old order: step 1 filed 1 mail directly, the AI sorted 31).
# Item "Let decisions pick any node of the label tree" (step a-tree-node-decision): on the Labels decision page a rule or the model picks ANY node of the
# tree with its path shown, the model may propose a new child under an existing parent for the owner to accept, and decided_by names the node.
# A clause the app serves no row or door for is NOT JUDGED with the missing row or door named (never faked). Notes: docs/lanes/miss-labels-cases.md.
import re as _re

REPLY_MSG = "flow-kept-1"           # a message the file's `flow_example` rows file under À répondre: step 1 picks it, the AI accepts it, the exit filter keeps it (A140)
ENTRY_MSG = "flow-entry-1"          # the row that was the "entry" card before A140 (step 1 filed it, no AI): now picked, accepted and kept like the others


def example_open(c):
    reset(c)
    say(c, "%s.open.modify.%s" % (INS, BOARD))
    settle(c)


def example_counts(c):
    """What the inspector's cards say and what the core serves (label/<id>/flow/counts), both as numbers; the two must agree."""
    cards = flow_rows(c)[0]
    core = c.core("label/%s/flow/counts" % BOARD) or {}
    ent = _re.search(r"picks (\d+)", cards["entry"].get("count") or "")
    ai = _re.search(r"(\d+) accepted", cards["ai"].get("count") or "")
    wait = _re.search(r"(\d+) waiting for the AI", cards["ai"].get("count") or "")
    ex = _re.search(r"keeps (\d+) of (\d+)", cards["exit"].get("count") or "")
    foot = _re.search(r"(\d+) to Primary", (c.rows(INS + ".foot") or [{}])[0].get("text") or "")
    return {"card_entry": ent and int(ent.group(1)), "card_ai": ai and int(ai.group(1)), "card_kept": ex and int(ex.group(1)),
            "card_waiting": wait and int(wait.group(1)), "card_ai_text": cards["ai"].get("count"), "foot_primary": foot and int(foot.group(1)), "core": core}


def e_entry(c):
    example_open(c)
    n = example_counts(c)
    # fails on: the old "catches 1" (step 1 filing directly) or any picked count but 37; the card and the core's caught_by_entry (now "picked") must agree
    return n["card_entry"] == EX["entry"] and n["core"].get("caught_by_entry") == EX["entry"], \
        "entry card says picks %r, the core says caught_by_entry %r (want %s)" % (n["card_entry"], n["core"].get("caught_by_entry"), EX["entry"])


def e_ai(c):
    example_open(c)
    n = example_counts(c)
    # fails on: the old 31 (the AI sorted what step 1 did not catch) or any accepted count but 32
    return n["card_ai"] == EX["ai"] and n["core"].get("sorted_by_ai") == EX["ai"], \
        "AI card says %r accepted, the core says sorted_by_ai %r (want %s)" % (n["card_ai"], n["core"].get("sorted_by_ai"), EX["ai"])


def e_kept(c):
    example_open(c)
    n = example_counts(c)
    # fails on: the old 23 (the row flow-entry-1 was not kept by step 3) or any kept count but 24
    return n["card_kept"] == EX["kept"] and n["core"].get("kept_by_exit") == EX["kept"], \
        "exit card says keeps %r, the core says kept_by_exit %r (want %s)" % (n["card_kept"], n["core"].get("kept_by_exit"), EX["kept"])


def e_primary(c):
    example_open(c)
    n = example_counts(c)
    fb = flow_rows(c)[0]["exit"].get("fallback")
    return n["foot_primary"] == EX["sent"] and n["core"].get("sent_to_fallback") == EX["sent"] and fb == "primary", \
        "foot says %r to Primary, the core says sent_to_fallback %r, the fallback is %r" % (n["foot_primary"], n["core"].get("sent_to_fallback"), fb)


def e_reply(c):
    """The replied fact flips on one kept message through `fixture reply <id>`: the kept count drops by one, the fallback count rises by one,
    and the message leaves the label (the core's assignment loses it)."""
    try:
        fixture_reset(c)      # the cases before it (filing, doors) act on the top rows of the inbox, where the flow example's rows are: this one starts from the launch state
    except Unreadable:
        pass
    example_open(c)
    before = example_counts(c)
    if not in_board(c, REPLY_MSG):
        raise Unreadable("%s is not filed under %s in this corpus, so there is no kept message to answer | label_assignment/%s is %r; staged: the file's flow_example rows (kept) filed by the AI"
                         % (REPLY_MSG, BOARD, REPLY_MSG, assigned(c, REPLY_MSG)))
    r = c.send("fixture", "reply", REPLY_MSG)
    example_open(c)
    after = example_counts(c)
    held = in_board(c, REPLY_MSG)
    kept_b, kept_a = before["core"].get("kept_by_exit"), after["core"].get("kept_by_exit")
    sent_b, sent_a = before["core"].get("sent_to_fallback"), after["core"].get("sent_to_fallback")
    # A140: the mail stays picked and accepted (step 1 and the AI are not re-read: the fact that flipped is read by step 3), so picks and accepts do not move
    pk_b, pk_a = before["core"].get("caught_by_entry"), after["core"].get("caught_by_entry")
    ac_b, ac_a = before["core"].get("sorted_by_ai"), after["core"].get("sorted_by_ai")
    ok = isinstance(kept_b, int) and isinstance(kept_a, int) and kept_a == kept_b - 1 and after["card_kept"] == before["card_kept"] - 1 \
        and sent_a == sent_b + 1 and not held and (pk_a, ac_a) == (pk_b, ac_b)      # the absolute figures are judged by the four cases above
    return ok, "reply on %s (ended in %r): kept %s -> %s (card %s -> %s; want one fewer), to Primary %s -> %s (want one more), picked %s -> %s and accepted %s -> %s (want no change), still in the label: %s" % (
        REPLY_MSG, r.get("ended_in"), kept_b, kept_a, before["card_kept"], after["card_kept"], sent_b, sent_a, pk_b, pk_a, ac_b, ac_a, held)


def e_waiting(c):
    """New under A140: the mails step 1 picked and the AI has not ruled on yet are counted, on the card and in the core (`waiting_for_ai`)."""
    example_open(c)
    n = example_counts(c)
    # fails on: no waiting part on the AI card (the old "31 messages"), a core without waiting_for_ai, or any waiting count but 5
    return n["card_waiting"] == EX["waiting"] and n["core"].get("waiting_for_ai") == EX["waiting"], \
        "AI card says %r (waiting %r), the core says waiting_for_ai %r (want %s)" % (n["card_ai_text"], n["card_waiting"], n["core"].get("waiting_for_ai"), EX["waiting"])


GONE_WORDS = ("will see none", "straight in", "no AI")      # the removed amber line "The AI will see none of them" and the old card sentence "goes straight in, no AI"


def e_nowarning(c):
    """New under A140: step 1 no longer files a mail the AI reads, so the entry card has no amber warning: no `warning` read-out and none of the old words on any row."""
    example_open(c)
    rows = c.rows(FL) + (c.rows(INS + ".foot") or [])
    warned = [r.get("ax") or r.get("card") or r.get("chain") for r in rows if r.get("warning") not in (None, "")]
    worded = [(r.get("card") or r.get("chain") or "foot", k, v) for r in rows for k, v in r.items() if isinstance(v, str) and any(w in v for w in GONE_WORDS)]
    # fails on: a row that carries a warning text, or any row string with a removed word
    return not warned and not worded, "rows with a warning: %s; rows with a removed word %s: %s" % (warned or "none", list(GONE_WORDS), worded or "none")


def e_whyentry(c):
    """The row that was the entry card before A140 is picked by step 1, accepted by the AI and kept by step 3: its why read names the exit card, the AI's answer and step 1's own read."""
    example_open(c)
    try:
        w = c.core("message/%s/why/%s" % (ENTRY_MSG, BOARD)) or {}
    except Unreadable as e:
        raise Unreadable("%s is not filed under %s in this corpus | %s" % (ENTRY_MSG, BOARD, e))
    entry = w.get("entry") or {}
    # fails on: the old card "entry" (step 1 filed it, no AI), no "ai": "yes", or no step 1 read (chain and the held body rule)
    ok = w.get("card") == "exit" and w.get("ai") == "yes" and isinstance(entry.get("chain_id"), int) and bool(entry.get("matched"))
    return ok, "why of %s: card %r (want 'exit'), ai %r (want 'yes'), entry %r (want a chain_id and the held conditions)" % (ENTRY_MSG, w.get("card"), w.get("ai"), w.get("entry"))


EXAMPLE = [("a140-step-1-picks-37", e_entry, "entry-count-wrong"), ("a140-the-ai-accepts-32", e_ai, "ai-count-wrong"),
           ("a140-the-exit-filter-keeps-24", e_kept, "kept-count-wrong"), ("a140-8-go-to-primary", e_primary, "primary-count-wrong"),
           ("a140-a-reply-moves-one-message-out-of-the-label", e_reply, "reply-keeps-it-in"),
           ("a140-a-reply-moves-one-message-out-of-the-label", e_reply, "reply-leaves-counts"),
           ("a140-a-reply-moves-one-message-out-of-the-label", e_reply, "reply-moves-the-picks"),
           ("a140-5-picked-mails-wait-for-the-ai", e_waiting, "waiting-missing"),
           ("a140-no-warning-says-the-ai-will-see-none", e_nowarning, "warning-back"),
           ("a140-the-example-row-step-1-once-filed-is-now-exit", e_whyentry, "why-says-entry")]


class FakeExample:
    """The inspector on À répondre and the fixture's reply door, for --selftest: 37 / 32 / 24 / 8 with 5 waiting (A140, from the file), a kept message, and a reply
    that moves it to Primary."""
    def __init__(self, mutant=None):
        self.m, self.open, self.lid = mutant, False, None
        self.n = {"entry": EX["entry"] + (1 if mutant == "entry-count-wrong" else 0), "ai": EX["ai"] - (1 if mutant == "ai-count-wrong" else 0),
                  "kept": EX["kept"] - (1 if mutant == "kept-count-wrong" else 0), "sent": EX["sent"] - (1 if mutant == "primary-count-wrong" else 0),
                  "wait": EX["waiting"]}
        self.held = True

    def send(self, verb, *a):
        if verb == "click" and a[0] == "%s.open.modify.%s" % (INS, BOARD):
            self.open, self.lid = True, BOARD
        elif verb == "key":
            self.open = False
        elif verb == "fixture" and a[0] == "reply":
            if self.m != "reply-leaves-counts":
                self.n["kept"] -= 1
                self.n["sent"] += 1
            if self.m == "reply-moves-the-picks":      # a re-read of step 1 and the AI on the flip: the mail drops out of the picks and the accepts
                self.n["entry"] -= 1
                self.n["ai"] -= 1
            self.held = self.m == "reply-keeps-it-in"
            return {"ended_in": "primary"}
        return {}

    def rows(self, ax):
        n = self.n
        if ax == INS:
            return [{"open": self.open, "counting": False, "label_id": self.lid}]
        if ax == FL:
            ai_text = "%d accepted" % n["ai"] if self.m == "waiting-missing" else "%d accepted · %d waiting for the AI" % (n["ai"], n["wait"])
            entry = {"card": "entry", "on": True, "count": "picks %d" % n["entry"]}
            if self.m == "warning-back":      # the amber line of the old flow, read out as `warning`
                entry["warning"] = "The AI will see none of them."
            return [entry, {"card": "ai", "on": True, "count": ai_text},
                    {"card": "exit", "on": True, "count": "keeps %d of %d" % (n["kept"], n["ai"]), "fallback": "primary"}]
        if ax == INS + ".foot":
            return [{"text": "%d messages · %d picked · %d accepted · %d to Primary" % (n["kept"], n["entry"], n["ai"], n["sent"])}]
        return []

    def core(self, path):
        if path == "label/%s/flow/counts" % BOARD:
            n = self.n
            out = {"caught_by_entry": n["entry"], "sorted_by_ai": n["ai"], "kept_by_exit": n["kept"], "sent_to_fallback": n["sent"]}
            if self.m != "waiting-missing":
                out["waiting_for_ai"] = n["wait"]
            return out
        if path == "message/%s/why/%s" % (ENTRY_MSG, BOARD):
            if self.m == "why-says-entry":      # the old read: step 1 filed it, no AI
                return {"card": "entry", "chain_id": 0, "matched": [{"kind": "body", "words": BODY_WORDS}]}
            return {"card": "exit", "chain_id": 0, "ai": "yes", "entry": {"chain_id": 0, "matched": [{"kind": "body", "words": BODY_WORDS}]},
                    "matched": [{"kind": "fact", "fact": "unanswered", "wanted": True}]}
        if path == "label_assignment/%s" % REPLY_MSG:
            return [BOARD] if self.held else []
        raise Unreadable("no path %s" % path)


# ---- the Labels decision page: any node of the tree
AIW = "mac-settings-ai"
PG = AIW + ".decisions.page.labels"
SEP = " › "
RULE_WORDS = "from provanta"      # the preview runs on the test engine's own sample (FixtureDecisionPreview.sample: senders are NAMES, m10 "Provanta" and m19 "Provanta Billing"); an address rule matches none of them (main104: no message matched `from @provanta-example.be`)
NODE_FINDING = ("the Labels decision page serves no door that sets a rule's label | clicked %r; missing: a door `" + PG + ".entry.<n>.node.<k>` (k = the row of `"
                + AIW + ".label-menu` as listed) that sets entry n's outcome to that node; staged: nothing, DecisionLabelMenu.swift's SwiftUI Menu is the only control "
                "(Sources/Features/SettingsAIMac/DecisionLabelMenu.swift:9-12, its choice calls store.update) and SettingsAIMacFeature.swift:185-189 registers the page's doors "
                "preview, apply, add-rule, add-model and proposal.accept; pattern to follow: `" + AIW + ".agents.model.<n>.choose.<k>` (SettingsAIMacFeature.swift:150-152)")
APPLY_FINDING = ("Apply writes nothing to the core | rerun_count of node %r was %r before Apply and %r after; missing: a core command that writes the decision list "
                 "(AISettingsStore.apply(page:), Sources/Features/SettingsAIMac/AISettingsStore.swift:394, \"INTERIM: no core command writes a decision list; Apply records it in this "
                 "session only\") and the engine's filing by a decision rule (core/crates/decision/); pattern to follow: the re-run's own write, `label/<l>/rerun` (rerunPreviewLocked, Sources/CoreBridge/FixtureAgents.swift:239); staged: a rule with a nested node as its outcome and Apply")


def tree_page(c):
    c.send("click", AIW + ".open")
    c.send("click", AIW + ".decisions.open.labels")


def tree_paths(core_tree, above=()):
    """Every node of the core's tree (labels/tree: id, name, children) as (id, display path, depth)."""
    out = []
    for n in core_tree or []:
        p = above + (n["name"],)
        out.append((n["id"], SEP.join(p), len(p)))
        out += tree_paths(n.get("children"), p)
    return out


def menu(c):
    tree_page(c)
    return [r["path"] for r in c.rows(AIW + ".label-menu")]


def seeded_rule(c):
    tree_page(c)
    # the case's own rule (by its words): the rule `t_pick` makes ("subject contains …", any node) is another case's and matches no mail
    rules = [r for r in c.rows(PG + ".entries") if r.get("kind") == "rule" and r.get("text") == RULE_WORDS]
    if not rules:
        # the Labels decision list ships with its default alone (lead ruling): the case makes its rule, as `fixture add-rule` puts one first ("Create a rule"),
        # filed under the nested node Provanta, for the sender the corpus holds
        shown = [p for _, p, _ in tree_paths(c.core("labels/tree"))]      # the core's tree, not the menu: a broken menu must not move this rule
        target = next((p for p in shown if p.endswith("Provanta") and p.count(SEP) >= 1), max(shown, key=lambda p: p.count(SEP), default=None))
        if target:
            c.send("fixture", "add-rule", "labels", RULE_WORDS, target)
            tree_page(c)
            rules = [r for r in c.rows(PG + ".entries") if r.get("kind") == "rule" and r.get("text") == RULE_WORDS]
    if not rules:
        raise Unreadable("the Labels page lists no rule entry, and `fixture add-rule` made none | rows %r" % c.rows(PG + ".entries"))
    return rules[0]


def t_menu(c):
    shown = menu(c)
    tree = tree_paths(c.core("labels/tree"))
    want = [p for _, p, _ in tree]
    deep = [p for p in shown if p.count(SEP) >= 1]
    return sorted(shown) == sorted(want) and len(want) > 0 and len(deep) > 0, "the menu lists %d nodes, the core's tree holds %d; nested ones shown with their path: %d (deepest %r)" % (
        len(shown), len(want), len(deep), max(shown, key=lambda p: p.count(SEP), default=None))


def t_rule(c):
    r = seeded_rule(c)
    tree = [p for _, p, _ in tree_paths(c.core("labels/tree"))]
    return r["outcome"] in tree and r["outcome"].count(SEP) >= 1, "the rule %r files under %r; it is a node of the core's tree: %s, nested: %s" % (
        r.get("text"), r["outcome"], r["outcome"] in tree, r["outcome"].count(SEP) >= 1)


def t_pick(c):
    shown = menu(c)
    target = max(shown, key=lambda p: p.count(SEP))
    k = shown.index(target) + 1
    c.send("click", PG + ".add-rule")       # a node is picked on a RULE entry: the list ships with its default alone, "Create a rule" puts a new rule first (entry 1)
    try:
        c.send("click", "%s.entry.1.node.%d" % (PG, k))
    except Unreadable as e:
        raise Unreadable(NODE_FINDING % str(e))
    out = [r for r in c.rows(PG + ".entries") if r.get("n") == 1][0]["outcome"]
    return out == target, "picked row %d of the menu, %r; entry 1 now files under %r" % (k, target, out)


def preview_hits(c):
    r = seeded_rule(c)
    c.send("click", PG + ".preview")
    # the preview is asked off the main actor and lands a moment after the click: read until a rule's row is there, bounded (main111: the first read held only
    # "entry 2 · otherwise" rows, the read in the failure line, a moment later, held m10 decided by "entry 1 · rule · Work > Clients > Provanta")
    rows = []
    for _ in range(40):
        rows = [p for p in c.rows(PG + ".preview") if " · rule" in str(p.get("decided_by", ""))]
        if rows:
            break
        time.sleep(0.15)
    if not rows:
        raise Unreadable("no message matched the rule %r in the preview | preview rows %r" % (r.get("text"), c.rows(PG + ".preview")))
    return r, rows


def t_filed(c):
    r, rows = preview_hits(c)
    bad = [p["message"] for p in rows if p["outcome"] != r["outcome"]]
    return not bad, "the preview files %d matching message(s) %s under %r (the rule's node); one under another outcome: %s" % (
        len(rows), [p["message"] for p in rows], r["outcome"], bad)


def t_decided(c):
    r, rows = preview_hits(c)
    names = [(p["message"], p["decided_by"]) for p in rows if r["outcome"] not in p["decided_by"]]
    return not names, "the node is %r; decided_by of the rows: %s; rows whose decided_by does not name the node: %s" % (
        r["outcome"], [p["decided_by"] for p in rows], names)


def t_core_files(c):
    r = seeded_rule(c)
    tree = tree_paths(c.core("labels/tree"))
    node = next((i for i, p, _ in tree if p == r["outcome"]), None)
    if node is None:
        raise Unreadable("the rule's node %r is not in the core's tree | tree %r" % (r["outcome"], [p for _, p, _ in tree]))
    read = lambda: {i: c.core("labels/%s/rerun_count" % i) for i, _, _ in tree}
    before = read()
    c.send("click", PG + ".apply")
    after = read()
    if after == before:
        raise Unreadable(APPLY_FINDING % (r["outcome"], before.get(node), after.get(node)))
    moved = {i: (before[i], after[i]) for i in before if before[i] != after[i]}
    return isinstance(before[node], int) and isinstance(after[node], int) and after[node] > before[node], \
        "Apply: files under node %r (%s) went %s -> %s; every node that moved: %s" % (r["outcome"], node, before[node], after[node], moved)


def proposal(c):
    tree_page(c)
    rows = c.rows(PG + ".proposal")
    if not rows:
        raise Unreadable("the Labels page shows no proposal | rows %r" % rows)
    return rows[0]


def t_proposal(c):
    p = proposal(c)
    tree = [x for _, x, _ in tree_paths(c.core("labels/tree"))]
    parent = p["path"].rsplit(SEP, 1)[0]
    return p.get("parent_in_tree") is True and parent in tree and p["path"] not in tree, "the model proposes %r; its parent %r is in the core's tree: %s, parent_in_tree says %r, the child is new: %s" % (
        p["path"], parent, parent in tree, p.get("parent_in_tree"), p["path"] not in tree)


def accepted(c):
    """The Accept click happens once per rig (it consumes the proposal): both clauses read the same before and after."""
    if getattr(c, "_accepted", None) is None:
        p = proposal(c)
        before_rows, before_tree = c.rows(PG + ".entries"), [x for _, x, _ in tree_paths(c.core("labels/tree"))]
        c.send("click", PG + ".proposal.accept")
        c._accepted = (p, before_rows, c.rows(PG + ".entries"), before_tree, [x for _, x, _ in tree_paths(c.core("labels/tree"))], c.rows(PG + ".proposal"))
    return c._accepted


def t_accept_rule(c):
    p, b, a, _, _, left = accepted(c)
    got = [r for r in a if r.get("outcome") == p["path"]]
    return len(a) == len(b) + 1 and bool(got) and not left, "accepting %r: entries %d -> %d, an entry files under it: %s, proposal still shown: %s" % (p["path"], len(b), len(a), bool(got), bool(left))


def t_accept_label(c):
    p, _, _, tb, ta, _ = accepted(c)
    return p["path"] not in tb and p["path"] in ta, "accepting %r: the core's tree held it before: %s, after: %s (%d -> %d nodes)" % (p["path"], p["path"] in tb, p["path"] in ta, len(tb), len(ta))


TREE = [("a-tree-node-the-menu-lists-every-node-with-its-path", t_menu, "menu-top-level-only"),
        ("a-tree-node-a-rule-files-under-a-nested-node", t_rule, "rule-top-level"),
        ("a-tree-node-the-preview-files-a-match-under-the-node", t_filed, "preview-leaf-only"),
        ("a-tree-node-decided-by-names-the-node", t_decided, "decided-by-no-node"),
        ("a-tree-node-the-owner-picks-a-nested-node-for-a-rule", t_pick, "pick-ignored"),
        ("a-tree-node-apply-files-under-the-node-in-the-core", t_core_files, "apply-files-wrong-node"),
        ("a-tree-node-the-model-proposes-a-child-of-an-existing-node", t_proposal, "proposal-parent-missing"),
        ("a-tree-node-accepting-the-proposal-adds-a-rule", t_accept_rule, "accept-ignored"),
        ("a-tree-node-accepting-the-proposal-creates-the-label", t_accept_label, "accept-no-label")]
NEEDS_DOOR = ("a-tree-node-the-owner-picks-a-nested-node-for-a-rule", "a-tree-node-apply-files-under-the-node-in-the-core")


class FakeTree:
    """The Labels decision page and the core's tree, for --selftest. `door` = the page serves the node door and Apply files in the core (what the finding asks for);
    without it the stand-in is today's app. The stand-in's decided_by names the node, its accept creates the label: the two the finding rows say the app lacks."""
    T = [("lbl-work", "Work", None), ("lbl-clients", "Clients", "lbl-work"), ("lbl-prox", "Provanta", "lbl-clients"), ("lbl-rfp", "RFP 2026", "lbl-prox"), ("lbl-q4", "Q4", "lbl-rfp"),
         ("lbl-sup", "Suppliers", "lbl-work"), ("lbl-pers", "Personal", None)]

    def __init__(self, mutant=None, door=True):
        self.m, self.door = mutant, door
        self.nodes = list(self.T)
        self.entries = [{"n": 1, "kind": "default", "outcome": "none"}]      # the list ships with its default alone
        self.proposed = "Work › Clients › Provanta › Tender 2027"
        self.preview, self.filed = [], {}

    def path(self, i):
        d = {n[0]: n for n in self.nodes}
        out, cur = [], d[i]
        while cur:
            out.insert(0, cur[1])
            cur = d.get(cur[2])
        return SEP.join(out)

    def put_rule(self, words, outcome):
        self.entries.insert(0, {"n": 1, "kind": "rule", "text": words, "outcome": outcome})
        for i, e in enumerate(self.entries):
            e["n"] = i + 1

    def send(self, verb, *a):
        if verb == "fixture" and a[:2] == ("add-rule", "labels"):
            self.put_rule(a[2], "Work" if self.m == "rule-top-level" else a[3])
            return {}
        if verb != "click":
            return {}
        ax = a[0]
        if ax == PG + ".add-rule":
            self.put_rule("subject contains …", self.path(self.nodes[0][0]))
        elif ax == PG + ".preview":
            o0 = self.entries[0]["outcome"]
            o = o0.rsplit(SEP, 1)[-1] if self.m == "preview-leaf-only" else o0
            by = "entry 1 · rule" if self.m == "decided-by-no-node" else "entry 1 · rule → %s" % o0
            self.preview = [{"message": "m10", "outcome": o, "entry": 1, "decided_by": by}, {"message": "m19", "outcome": o, "entry": 1, "decided_by": by}]
        elif ax.startswith(PG + ".entry.1.node."):
            if not self.door:
                raise Unreadable("click %r: no such click target" % (ax,))
            if self.m != "pick-ignored":
                self.entries[0]["outcome"] = self.path(self.nodes[int(ax.rsplit(".", 1)[1]) - 1][0])
        elif ax == PG + ".apply":
            if self.door:
                target = next(n[0] for n in self.nodes if self.path(n[0]) == self.entries[0]["outcome"])
                if self.m == "apply-files-wrong-node":
                    target = next(n[2] for n in self.nodes if n[0] == target)
                self.filed[target] = self.filed.get(target, 0) + 2
        elif ax == PG + ".proposal.accept" and self.proposed:
            if self.m != "accept-ignored":
                self.entries.insert(0, {"n": 1, "kind": "rule", "text": "the model proposed it", "outcome": self.proposed})
                for i, e in enumerate(self.entries):
                    e["n"] = i + 1
            if self.m != "accept-no-label":
                parent = next(n[0] for n in self.nodes if self.path(n[0]) == self.proposed.rsplit(SEP, 1)[0])
                self.nodes.append(("lbl-new", "Tender 2027", parent))
            self.proposed = None
        return {}

    def rows(self, ax):
        if ax == AIW + ".label-menu":
            ps = [self.path(n[0]) for n in self.nodes]
            return [{"path": p} for p in (ps if self.m != "menu-top-level-only" else [p for p in ps if SEP not in p])]
        if ax == PG + ".entries":
            return [dict(e) for e in self.entries]
        if ax == PG + ".preview":
            return list(self.preview)
        if ax == PG + ".proposal":
            if not self.proposed:
                return []
            return [{"path": self.proposed, "parent_in_tree": self.m != "proposal-parent-missing"}]
        return []

    def core(self, path):
        if path == "labels/tree":
            def kids(p):
                return [{"id": n[0], "name": n[1], "children": kids(n[0])} for n in self.nodes if n[2] == p]
            return kids(None)
        if path.startswith("labels/") and path.endswith("/rerun_count"):
            return self.filed.get(path.split("/")[1], 0)
        raise Unreadable("no path %s" % path)



def run(c):
    out = []
    c.send("click", "mac-settings-general.appearance.language.en")   # the words the checks read are English, whatever a past run left
    seen = set()
    # the AI refusal case stages a new installation (`fixture install-state new`); the Settings > AI pages keep what they read there after `fixture reset`
    # (main92: a label of that case in the decision tree, no rule entries, no re-run preview), so it runs LAST and nothing reads after it
    last = [x for x in FLOW if x[1] in (f_ai_refusal, f_ai_under_trust)]      # (the under-trust case opens Settings through the Decisions door)
    first = [x for x in FLOW if x[1] not in (f_ai_refusal, f_ai_under_trust)]      # (the needs-Apple case opens nothing: a test copy only records the URL)
    for name, fn, _m in CHECKS + KEYS + INSPECTOR + first + WINDOW + DOORS_IN + FILING + EXAMPLE + TREE + last:
        if fn in seen:
            continue   # one case, two mutants: it is driven once
        seen.add(fn)
        try:
            ok, d = fn(c)
            out.append((name, "pass" if ok else "fail", d))
            if fn is e_reply:
                try:
                    fixture_reset(c)   # the reply edited the label's flow: the launch state (37 / 32 / 24 / 8, 5 waiting, served) is back for what follows
                except Unreadable as e:
                    out.append((name + "-then-the-reset", "unjudged", str(e)))
        except Unreadable as e:
            out.append((name, "unjudged", str(e)))
        except (KeyError, IndexError, TypeError, ValueError) as e:
            out.append((name, "fail", "reply shape: %s" % e))
    return out


def report(res):
    for n, st, d in res:
        print("%-28s %-8s %s" % (n, st, d))
    f, u = sum(r[1] == "fail" for r in res), sum(r[1] == "unjudged" for r in res)
    print("%d checks, %d failed, %d not judged" % (len(res), f, u))
    return 1 if f or u else 0


def selftest():
    bad = 0
    print("MUTANTS | check | clean | mutant | mutant result")
    for name, fn, mutant in CHECKS:
        clean, _ = fn(Fake())
        red, _ = fn(Fake(mutant))
        print("MUTANTS | %s | %s | %s | %s" % (name, "green" if clean else "RED", mutant, "red" if not red else "STILL GREEN"))
        bad += (not clean) + bool(red)
    for name, fn, mutant in KEYS:
        clean, cd = fn(FakeKeys())
        red, rd = fn(FakeKeys(mutant))
        print("MUTANTS | %s | %s | %s | %s | %s" % (name, "green" if clean else "RED " + cd, mutant, "red" if not red else "STILL GREEN", rd))
        bad += (not clean) + bool(red)
    # the inspector checks run as one session, in order, on one stand-in (create needs the live rule, modify needs the created label)
    for name, fn, mutant in INSPECTOR:
        for label, m in (("clean", None), ("mutant", mutant)):
            f = FakeInspector(m)
            try:
                for pre in INSPECTOR:
                    if pre[0] == name:
                        break
                    pre[1](f)
                ok, detail = fn(f)
            except Unreadable as e:
                ok, detail = False, str(e)
            if label == "clean":
                clean, cd = ok, detail
            else:
                red, rd = ok, detail
        print("MUTANTS | %s | %s | %s | %s | %s" % (name, "green" if clean else "RED", mutant, "red" if not red else "STILL GREEN", rd))
        bad += (not clean) + bool(red)
    for name, fn, mutant in FLOW:
        res = []
        for m in (None, mutant):
            try:
                ok, detail = fn(FakeFlow(m))
            except Unreadable as e:
                ok, detail = False, str(e)
            res.append((ok, detail))
        (clean, cd), (red, rd) = res
        print("MUTANTS | %s | %s | %s | %s | %s" % (name, "green" if clean else "RED " + cd, mutant, "red" if not red else "STILL GREEN", rd))
        bad += (not clean) + bool(red)
    # triage-keys: Why here on a refused filing, on its stand-in and on broken copies of the Swift (each must go red)
    src0 = open(FOOT_SRC, encoding="utf-8").read()
    mut = filing_mutated(src0)
    for k, v in mut.items():
        assert v != src0, "mutant %s changes nothing: its anchor text is gone from LabelInspectorFoot.swift" % k
    for name, fn, mutant in FILING:
        res = []
        for m in (None, mutant):
            try:
                if fn is f_unsorted_swift:
                    ok, detail = fn(None, mut[m] if m else src0)
                else:
                    ok, detail = fn(FakeFiling(m))
            except Unreadable as e:
                ok, detail = False, str(e)
            res.append((ok, detail))
        (clean, cd), (red, rd) = res
        print("MUTANTS | %s | %s | %s | %s | %s" % (name, "green" if clean else "RED " + cd, mutant, "red" if not red else "STILL GREEN", rd))
        bad += (not clean) + bool(red)
    # each layout rule red on its own case (the owner's 16:02 defects), on the stand-in's clean geometry
    base, fit = FakeInspector().geometry(), FakeInspector().rowfit()
    def case(edit):
        g = [dict(x) for x in base]
        edit({x["id"]: x for x in g}, g)
        return layout_faults(g, fit)
    for rule, edit, word in (
            ("row-outside-its-card", lambda d, g: d["geo.ctl.label.scope"].update(x=40), "inset"),
            ("fact-outside-its-card", lambda d, g: d["geo.ctl.entry.fact.unread"].update(x=640, w=200), "inset"),
            ("two-close-controls", lambda d, g: g.append({"id": "geo.free.close.header", "kind": "free", "card": None, "x": 500, "y": 20, "w": 60, "h": 26}), "Close controls"),
            ("description-narrower", lambda d, g: d["geo.ctl.ai.editor"].update(w=300), "description"),
            ("two-controls-overlap", lambda d, g: d["geo.ctl.exit.fact.unanswered"].update(x=200, y=560), "overlap")):
        found = case(edit)
        red = any(word in f for f in found)
        print("MUTANTS | layout rule %s | %s | %s" % (rule, "red" if red else "STILL GREEN", found[:2]))
        bad += not red
    # doors-labels: each new case on its stand-in, clean and with the mutant that disables its one control
    def twice(fn, make, mutant):
        res = []
        for m in (None, mutant):
            try:
                ok, detail = fn(make(m))
            except Unreadable as e:
                ok, detail = False, str(e)
            res.append((ok, detail))
        return res
    for name, fn, mutant in WINDOW:
        (clean, cd), (red, rd) = twice(fn, FakeWin, mutant)
        print("MUTANTS | %s | %s | %s | %s | %s" % (name, "green" if clean else "RED " + cd, mutant, "red" if not red else "STILL GREEN", rd))
        bad += (not clean) + bool(red)
    for name, fn, mutant in DOORS_IN:
        (clean, cd), (red, rd) = twice(fn, FakeDoors, mutant)
        print("MUTANTS | %s | %s | %s | %s | %s" % (name, "green" if clean else "RED " + cd, mutant, "red" if not red else "STILL GREEN", rd))
        bad += (not clean) + bool(red)
    # the evidence a key the inspector ignores carries: the focused field, the derived `editing`, the inspector's mode (a mutant keeps the field focused)
    for kname in ("kd-key-return-creates-the-new-label", "kd-key-r-reruns-the-offer", "kd-key-a-adds-the-other-rule", "kd-key-question-mark-opens-and-closes-why-here"):
        kfn = next(f for n, f, _m in DOORS_IN if n == kname)
        try:
            ok, detail = kfn(FakeDoors("editing-stuck"))
        except Unreadable as e:
            ok, detail = False, "NOT JUDGED " + str(e)
        print("MUTANTS | %s | editing-stuck | %s | %s" % (kname, "red" if not ok else "STILL GREEN", detail))
        bad += bool(ok)
        for need in ("focused field 'other:mac-labels.inspector.name'", "a text field holds the keys: yes", "'mode':", "'editing': 'absent'"):
            if need not in detail:
                print("MUTANTS | %s | evidence missing: %s" % (kname, need))
                bad += 1
    # lane tail-keys: the four cases that cannot tell a check defect from an app defect (and the four key cases above) print `evidence:` on their mutant
    ev_ok, ev_all = 0, 0
    for name, group, make in (("flow-ai-thread-saved", FLOW, FakeFlow), ("flow-ai-refusal-shown-and-opens-decisions", FLOW, FakeFlow),
                              ("w-sender-card-row-opens-its-thread", WINDOW, FakeWin), ("d-plus-gives-the-value-field-back", DOORS_IN, FakeDoors)):
        for n, fn, mutant in group:
            if n != name:
                continue
            ev_all += 1
            try:
                ok, detail = fn(make(mutant))
            except Unreadable as e:
                ok, detail = False, str(e)
            has = (not ok) and " | evidence: " in detail and "inspector" in detail.split(" | evidence: ", 1)[1]
            ev_ok += bool(has)
            if not has:
                print("MUTANTS | %s | %s | evidence missing: %s" % (name, mutant, detail))
                bad += 1
    print("evidence on red (par-labelscontactsmac): %d of %d new evidence cases print an `evidence:` part on their mutant (the four key cases are counted above)" % (ev_ok, ev_all))
    # the two cases on reads the app's core does not serve say what is missing, with the value the row held
    try:
        engine(FakeDoors())
        bad += 1
        print("MUTANTS | engine preview | a row with no engine_preview raised nothing")
    except Unreadable as e:
        print("MUTANTS | a125 engine preview | NOT JUDGED %s" % e)
        for need in ("the row says engine_preview None for label", "not staged", "core/crates/engine/src/flow_ctx.rs:259", "missing piece"):
            if need not in str(e):
                print("MUTANTS | a125 engine preview | evidence missing: %s" % need)
                bad += 1
    # the control table: each door and its drawn control are in the sources; a mutant removes one half and goes red
    gaps = [(e[0], door_gap(ROOT, e)) for e in DOORS if door_gap(ROOT, e)]
    print("MUTANTS | door table | %s | %d controls each have a click door and a drawn control%s" % ("green" if not gaps else "RED", len(DOORS), (": " + str(gaps)) if gaps else ""))
    bad += bool(gaps)
    for e in DOORS:
        for half in ("door", "control"):
            red = half in door_gap(ROOT, e, drop=half)
            print("MUTANTS | door table | green | the %s of '%s' is removed | %s" % (half, e[0], "red" if red else "STILL GREEN"))
            bad += not red
    # A140: the example's figures the cases expect are the ones the design fixture file defines (derived), and the owner's pinned ones; a file that drifts goes red here
    print("MUTANTS | example figures | %s | derived from docs/design/fixtures/mac-main-sample.json %s, pinned %s" % ("green" if EX == EX_PINNED else "RED", EX, EX_PINNED))
    bad += EX != EX_PINNED
    # lane miss-labels-cases: each new case green on its stand-in, red on its own mutant, and the mutant leaves every other case of its step green
    def judge(fn, make):
        try:
            return fn(make())
        except Unreadable as e:
            return None, "NOT JUDGED " + str(e)
    for step, group, make in (("a94-flow-example", EXAMPLE, lambda m: FakeExample(m)), ("a-tree-node-decision", TREE, lambda m: FakeTree(m, True))):
        fns = {n: f for n, f, _m in group}
        for name, fn, mutant in group:
            clean, cd = judge(fn, lambda: make(None))
            red, rd = judge(fn, lambda: make(mutant))
            others = [n for n, f in fns.items() if f is not fn and judge(f, lambda: make(mutant))[0] is not True]
            print("MUTANTS | %s | %s | %s | %s | %s | other cases of the step turned red or unjudged by it: %s" % (
                name, "green" if clean else "RED " + cd, mutant, "red" if red is False else "STILL GREEN or unjudged", rd, others or "none"))
            bad += (clean is not True) + (red is not False) + len(others)
    # the two clauses the app lacks a door for: on today's app the case is NOT JUDGED and names the missing door and its file
    for name in NEEDS_DOOR:
        ok, d = judge(dict((n, f) for n, f, _m in TREE)[name], lambda: FakeTree(None, False))
        words = ("missing:", "pattern to follow", "AISettingsStore.swift:394") if "apply" in name else ("missing: a door", "pattern to follow", "DecisionLabelMenu.swift:9-12")
        print("MUTANTS | %s | today's app | %s | %s" % (name, "NOT JUDGED" if ok is None else "JUDGED %s" % ok, d))
        bad += ok is not None or any(w not in d for w in words)
    print("%d checks, %d failed, 0 not judged" % (len(CHECKS) + len(KEYS) + len(INSPECTOR) + len(FLOW) + 5 + len(WINDOW) + len(DOORS_IN) + len(FILING) + 1 + 2 * len(DOORS) + len(EXAMPLE) + len(TREE), bad))
    return 1 if bad else 0


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        sys.exit(selftest())
    try:
        ctl = Ctl().connect()
    except CouldNotDrive as e:
        sys.exit(report([(n, "unjudged", str(e)) for n, *_ in CHECKS]))
    try:
        sys.exit(report(run(ctl)))
    finally:
        ctl.close()
