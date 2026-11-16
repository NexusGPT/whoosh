#!/usr/bin/env python3
"""Labels and contacts (iPhone): a label's page and a contact's page (docs/JOURNEYS.md J2.6 and J2.7 as the phone walks them).

Drives the RUNNING iPhone app over the control socket ($WHOOSH_SOCKET_DIR, docs/CONTROL-SOCKET.md) through journeys/lib/ctl.py.
Expected values come from docs/design/fixtures/mac-main-sample.json (the labels, the contacts) and from the Mac's own catalogue
words (Sources/Features/LabelsContactsMac: "Entry rules", "AI sorting", "Exit filter", the rule kinds), never from a built screen.
The ids the screen registers are in Sources/Features/LabelsContactsPhone/LabelsContactsPhoneFeature.swift and ...Probe.swift.

Run on a build:   sh scripts/launch-one.sh <iphone-bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/labelscontactsphone-check.py'
Self-test:        python3 checks/labelscontactsphone-check.py --selftest     the cases against a model app and a copy of the source, each case red on its mutant
Last line:        'N checks, F failed, U not judged' (exit 1 when F > 0 or U > 0: a check that could not run is not green).
Limits, said once: the self-test proves the judges, the order and the ids this file assumes, never the app. The driven run is the lead's.
Not driven: the merge offer (the fixture has no look-alike person) and the swipe-back gesture (the pages hide the system bar).
"""
import inspect
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.fixture import FX                                                       # noqa: E402

S = "iphone-labels-contacts"
SRC = os.path.join(ROOT, "Sources", "Features", "LabelsContactsPhone")
CARD_TITLES = ["Entry rules", "AI sorting", "Exit filter"]       # LabelFlowSocket.swift cards(): the Mac's names
EDIT_NOTE = "Rules are edited on the Mac."                        # LabelPagePhone.swift: the one line that says where rules are edited
NO_RULE = "No rule yet"
KIND = {"domain": "Domain is", "subject": "Subject contains"}     # RuleKind.title
THREAD_OVERLAY = "iphone-thread"                                  # InboxPhoneScreen.swift: a row opens the thread overlay


def flat(nodes, depth=1, path=""):
    out = []
    for n in nodes:
        p = (path + " > " if path else "") + n["name"]
        out.append({"name": n["name"], "path": p, "depth": depth, "colour": n.get("colour"), "pinned": bool(n.get("pinned")),
                    "meaning": n.get("description", ""), "rules": n.get("rules")})
        out += flat(n.get("children", []), depth + 1, p)
    return out


FXL = flat(FX["labels"])
BY = {l["name"]: l for l in FXL}
PERSON = FX["contacts"][0]
PERSON_ADDRS = sorted(a.lower() for a in PERSON["addresses"])
PERSON_MAIL = sorted(m["subject"] for m in FX["messages"] if m["from"].lower() in PERSON_ADDRS)
ACC_OF = {m["id"]: m["account"] for m in FX["messages"]}          # the account each fixture message was received on


def newest_first(rows):
    """The contact page lists the sender's messages newest first: received_ms never rises down the list (the Mac's J2.7 step reads the same order)."""
    ms = [r["received_ms"] for r in rows]
    return all(a >= b for a, b in zip(ms, ms[1:]))


class Refused(Exception):
    """An action the app refused: the control is not there, or it is dead."""


class Drive:
    """The socket client, or the model: the same four calls."""
    def __init__(self, app):
        self.app = app

    def click(self, ax):
        try:
            self.app.click(ax)
        except Refused:
            raise
        except Exception as e:                       # Unreadable from lib.ctl
            raise Refused("%s: %s" % (ax, e))

    def rows(self, ax):
        return self.app.rows(ax)

    def core(self, path):
        return self.app.core(path)

    def state(self):
        return self.app.uistate()

    # the helpers the cases share
    def row(self, ax):
        r = self.rows(ax)
        if not r:
            raise Refused("no row for %s" % ax)
        return r[0]

    def door(self):
        self.click(S + ".open")
        return self

    def label_id(self, name):
        for r in self.rows(S + ".labels"):
            if r["name"] == name:
                return r["id"]
        raise Refused("no label named %s in %s.labels" % (name, S))

    def open_label(self, name):
        lid = self.label_id(name)
        self.door().click(S + ".list.label." + lid)
        return lid

    def person_index(self, name):
        for i, r in enumerate(self.rows(S + ".people")):
            if r["name"] == name:
                return i
        raise Refused("no person named %s" % name)

    def pinned_ids(self):
        v = self.core("labels/pinned")
        v = json.loads(v) if isinstance(v, str) else v
        return {x["id"] for x in (v or [])}


# ---------------------------------------------------------------------------------------------- the driven cases
def l1_list(d):
    """The list is the core's tree. The test engine's tree holds the built-in category labels (Needs Reply, Updates, Promotions, Social, VIPs) ahead of the fixture's own 16
    (FixtureLabels.swift `installBuiltInLabels`); the Mac's Labels screen lists them too, and the phone lists what the core lists. So the fixture's 16 are judged as the rows
    that carry their names, in tree order with their depth, and the line prints the rows the phone served beside the 16 it wants (main phone6: only a count was printed)."""
    d.door()
    rows = d.rows(S + ".labels")
    want = [(l["name"], l["depth"]) for l in FXL]
    names = {n for n, _ in want}
    got_all = [(r["name"], r["depth"]) for r in rows]
    got = [g for g in got_all if g[0] in names]
    same = len([1 for g, w in zip(got, want) if g == w])
    extra = [g[0] for g in got_all if g[0] not in names]
    return got == want, "labels in tree order with their depth: %d of %d (rows served %d, built-in or other rows ahead of or among them: %s)%s" % (
        same, len(want), len(got_all), extra, "" if got == want else " | got %s | want %s" % (got_all, want))


def l2_label_page(d):
    bad = []
    for name in ("Provanta", "Suppliers"):
        e = BY[name]
        d.open_label(name)
        r = d.row(S + ".label")
        got = (r["name"], r["path"], r["colour"], r["meaning"], r["pinned"])
        if got != (e["name"], e["path"], e["colour"], e["meaning"], e["pinned"]):
            bad.append("%s: %r" % (name, got))
    return not bad, "; ".join(bad) or "Provanta and Suppliers equal the fixture's name, path, colour, meaning and pin"


def l3_cards_named(d):
    d.open_label("Suppliers")
    cards = d.rows(S + ".label.cards")
    titles = [c["title"] for c in cards]
    return titles == CARD_TITLES and [c["card"] for c in cards] == ["entry", "ai", "exit"], "cards: %r" % titles


def l4_rules_in_words(d):
    e = BY["Suppliers"]["rules"]
    want = [KIND[c["kind"]] + " " + (c.get("domain") or c.get("words")) for c in e["conditions"]]
    d.open_label("Suppliers")
    entry = d.rows(S + ".label.cards")[0]
    ok = entry["lines"] == want and entry["chains"][0]["mode"] == ("Any of these" if e["match"] == "any" else "All of these")
    d.open_label("Provanta")
    plain = d.rows(S + ".label.cards")
    ok2 = plain[0]["lines"] == [NO_RULE] and plain[1]["lines"][0].startswith("The AI reads: Subject")
    return ok and ok2, "Suppliers entry %r (want %r); Provanta entry %r, AI %r" % (entry["lines"], want, plain[0]["lines"], plain[1]["lines"])


def l5_where_edited(d):
    d.open_label("Suppliers")
    r = d.row(S + ".label")
    cards = d.rows(S + ".label.cards")
    return r["edit_note"] == EDIT_NOTE and not any("switch" in c or "field" in c for c in cards), "the page says %r" % r["edit_note"]


def l6_pin(d):
    lid = d.open_label("Provanta")
    start = d.row(S + ".label")["pinned"] and lid in d.pinned_ids()
    d.click(S + ".label.pin")
    off = (not d.row(S + ".label")["pinned"]) and lid not in d.pinned_ids()
    d.click(S + ".label.pin")
    on = d.row(S + ".label")["pinned"] and lid in d.pinned_ids()
    return start and off and on, "pinned at start %s, after one tap off in the core %s, after two on again %s" % (start, off, on)


def l7_door(d):
    d.open_label("Provanta")
    n = d.row(S + ".label")["mail"]
    d.click(S + ".label.mail")
    page = d.row(S + ".page")
    rows = d.rows(S + ".mail")
    here = page["kind"] == "mail" and page["depth"] == 2 and len(rows) == n
    d.click(S + ".back")
    back = d.row(S + ".page")["kind"] == "label"
    return here and back, "mail page %s with %d rows for a label that says %d; Back returns to %s" % (page["kind"], len(rows), n, "the label" if back else "?")


def l8_contact(d):
    d.door().click(S + ".list.person.%d" % d.person_index(PERSON["name"]))
    c = d.row(S + ".contact")
    msgs = d.rows(S + ".contact.messages")
    ok = c["name"] == PERSON["name"] and sorted(a.lower() for a in c["addresses"]) == PERSON_ADDRS and c["mark"] == "none"
    mail = sorted(m["subject"] for m in msgs) == PERSON_MAIL
    order = newest_first(msgs)
    accounts = [m["account"] for m in msgs] == [ACC_OF.get(m["id"]) for m in msgs]
    return ok and mail and order and accounts, ("one page, name %r, addresses %r, mark %r, %d messages (want %d), newest first over %d rows: %s, "
                                                 "account on each row as the fixture has it: %s") % (c["name"], c["addresses"], c["mark"], len(msgs), len(PERSON_MAIL), len(msgs), order, accounts)


def l9_every_control(d):
    """Each control once, each with its visible outcome."""
    seen = []
    def step(name, ok):
        seen.append((name, ok))
    d.click(S + ".open"); step("open", d.row(S + ".page")["kind"] == "list")
    lid = d.label_id("Provanta")
    d.click(S + ".list.label." + lid); step("list.label", d.row(S + ".page")["kind"] == "label")
    before = d.row(S + ".label")["pinned"]
    d.click(S + ".label.pin"); step("label.pin", d.row(S + ".label")["pinned"] != before)
    d.click(S + ".label.pin")
    d.click(S + ".label.mail"); step("label.mail", d.row(S + ".page")["kind"] == "mail")
    d.click(S + ".back"); step("back", d.row(S + ".page")["kind"] == "label")
    d.click(S + ".back"); step("back.to-list", d.row(S + ".page")["kind"] == "list")
    d.click(S + ".list.person.%d" % d.person_index(PERSON["name"])); step("list.person", d.row(S + ".page")["kind"] == "contact")
    if d.rows(S + ".contact.messages"):
        d.click(S + ".contact.messages.0"); step("contact.messages", d.state().get("overlay") == THREAD_OVERLAY)
        d.click(S + ".open")
    d.click(S + ".list.label." + lid); d.click(S + ".label.mail")
    if d.rows(S + ".mail"):
        d.click(S + ".mail.row.0"); step("mail.row", d.state().get("overlay") == THREAD_OVERLAY)
    d.click(S + ".open"); d.click(S + ".back"); step("back.closes", d.row(S + ".page")["kind"] == "closed")
    bad = [n for n, ok in seen if not ok]
    return not bad, "%d controls tapped once: %s%s" % (len(seen), ", ".join(n for n, _ in seen), ("; no outcome: " + ", ".join(bad)) if bad else "")


DRIVEN = [("L1", "the labels list is the fixture's tree, with its depth", l1_list),
          ("L2", "a label's page equals the fixture's label", l2_label_page),
          ("L3", "the three cards are named as on the Mac", l3_cards_named),
          ("L4", "the rules read in words, an empty card says so", l4_rules_in_words),
          ("L5", "the page says where rules are edited and offers no edit", l5_where_edited),
          ("L6", "the pin switch changes the core's value, both ways", l6_pin),
          ("L7", "the door opens the label's mail and Back returns", l7_door),
          ("L8", "a contact's page joins both addresses of one sender", l8_contact),
          ("L9", "every control is tapped once and shows an outcome", l9_every_control)]


# ---------------------------------------------------------------------------------------------- the source cases
EXPECTED_IDS = [S + ".back", S + ".label.pin", S + ".label.mail", S + ".list.label.", S + ".list.person.", S + ".mail.row.",
                S + ".contact.messages.", S + ".contact.merge", S + ".label.card.", S + ".label.edit-on-mac"]


def source_text(src):
    out = {}
    for n in sorted(os.listdir(src)):
        if n.endswith(".swift"):
            with open(os.path.join(src, n), encoding="utf-8") as f:
                out[n] = f.read()
    return out


def s1_ids(src):
    t = "\n".join(source_text(src).values())
    miss = [i for i in EXPECTED_IDS if not re.search(re.escape(i) + r"\\\(" if i.endswith(".") else '"' + re.escape(i) + '"', t)]
    return not miss, "ids in source: %d of %d%s" % (len(EXPECTED_IDS) - len(miss), len(EXPECTED_IDS), (" (missing " + ", ".join(miss) + ")") if miss else "")


def s2_no_literal_text(src):
    bad = []
    for n, t in source_text(src).items():
        for m in re.finditer(r'Text\("((?:[^"\\]|\\.)*)"\)', t):
            words = re.sub(r"\\\([^)]*\)", "", m.group(1))
            if re.search(r"[A-Za-z]", words):
                bad.append("%s: Text(%r)" % (n, m.group(1)))
    return not bad, "no literal user-facing text in code" if not bad else "; ".join(bad)


def s3_read_only(src):
    t = "\n".join(source_text(src).values())
    toggles = len(re.findall(r"\bToggle\(", t))
    edits = re.findall(r"\b(TextField|TextEditor|Stepper|Picker|DatePicker)\(", t)
    writes = re.findall(r"\.(createLabelFull|updateLabel|createLabel|renameLabel|moveLabel|deleteLabel|rerunLabel)\b", t)
    return toggles == 1 and not edits and not writes, "toggles %d (the pin), edit fields %r, label writes %r" % (toggles, edits, writes)


def s4_no_network(src):
    t = "\n".join(source_text(src).values())
    hits = re.findall(r"URLSession|https?://|NWConnection|AsyncImage|WKWebView", t)
    return not hits, "no network and no web view in the folder" if not hits else "found %r" % hits


def s5_figures_follow_the_mail(src):
    """A page that draws a label's unread figure from the core's labels is drawn again when the mail changes: its body reads AppModel.mailRevision. The Mac
    sidebar kept its old count after a mail was read in the list (the lead's walk on main183); the phone's two pages read the same figure the same way."""
    t = source_text(src)
    bad = [n for n in ("LabelsListPhone.swift", "LabelMailPhone.swift") if "m.core.labels()" in t.get(n, "") and "let _ = m.mailRevision" not in t.get(n, "")]
    return not bad, "the labels list and a label's page follow the mail" if not bad else "no mailRevision read in %s" % ", ".join(bad)


STATIC = [("S1", "every control carries its identifier", s1_ids), ("S2", "every user-facing word comes from the catalogue", s2_no_literal_text),
          ("S3", "the pin is the only write", s3_read_only), ("S4", "nothing leaves the device", s4_no_network),
          ("S5", "a label's figure follows the mail", s5_figures_follow_the_mail)]


# ---------------------------------------------------------------------------------------------- the Mac journey's steps (journeys/J2.py J2.6, J2.7)
# Every check call of those two steps is a key, with what the phone does with it (same literal on both sides: "same"; the phone repeats the step on
# its own reading: "substituted"; the phone has the thing and no case reads it: "not-driven"; the phone has no such thing: "excluded").
# A Mac step with no row, a row whose Mac step or literal is gone, a phone case or literal that is not in the case's code, a row with no reason: red.
MAC_JOURNEY = os.path.join(ROOT, "journeys", "J2.py")
MAC_STEPS = ["J2.6", "J2.7"]
# (Mac step/check, kind, phone case, Mac literal, phone literal, reason)
STEP_ROWS = [
    ("J2.6/rule-in-words", "substituted", "L4", 'EXP["exact_query"]', "Any of these", "the Mac reads the rule of the folder the search saved; the phone reads the fixture label Suppliers' rule in words (kind, value, any or all)"),
    ("J2.6/nested-under-work", "substituted", "L1", "smart_folder/%s/parent", '"depth"', "nesting is made on the Mac (the phone's page says 'Rules are edited on the Mac.', case L5); the phone reads that the fixture's tree shows its depth"),
    ("J2.6/sidebar-shows-nesting", "substituted", "L1", "sidebar_parent", '"depth"', "the Mac sidebar names the parent; the phone's list shows the depth of each label"),
    ("J2.6/rule-edited", "excluded", "L5", " has:attachment", "EDIT_NOTE", "rules are edited on the Mac only; L5 asserts the phone offers no edit and says where it is done"),
    ("J2.6/count-updates", "substituted", "L7", '"count"', '["mail"]', "the Mac reads the folder's count after the edit; the phone reads that the label's mail count equals the rows of its mail page"),
    ("J2.7/sender-messages-newest-first", "substituted", "L8", 'EXP["sender_messages_newest_first"]', "newest_first(msgs)", "the Mac compares the ids with its expected order; the phone reads that received_ms never rises down the sender's list (the phone fixture holds one message from this sender, so the order is judged on one row here and on synthetic rows in --selftest)"),
    ("J2.7/each-row-has-account", "substituted", "L8", 'EXP["sender_accounts"]', "ACC_OF.get(m", "the Mac reads the account of each row against its expected list; the phone reads the account of each row against the account the fixture gives that message id"),
    ("J2.7/sender-is-linea", "substituted", "L8", 'EXP["sender_address"]', "PERSON_ADDRS", "the Mac reads the one address of the sender; the phone reads that the page joins all the person's addresses"),
]
_CALL = re.compile(r'(?:\bs\.expect_le\(|\bs\.expect\(|\bs\._record\(|\bexpect_ctx\(s, |\bsame\(s, |\blt\(s, )"([a-z0-9][a-z0-9-]*)"')
_PATCH = set()


def journey_text():
    text = open(MAC_JOURNEY, encoding="utf-8").read()
    if "journey-gains-a-step" in _PATCH:
        i = text.index('s = j.step("J2.7"')
        j = text.index("\n", i)
        text = text[: j + 1] + '    s.expect("sender-new-step", lambda: 1, 1)\n' + text[j + 1:]
    if "journey-moves-a-value" in _PATCH:
        assert 'EXP["sender_address"]' in text
        text = text.replace('EXP["sender_address"]', 'EXP["sender_other"]', 1)
    return text


def mac_steps(text):
    out = {}
    for step in MAC_STEPS:
        i = text.index('j.step("%s"' % step)
        j = text.find('j.step("', i + 1)
        k = text.find("# MUTANTS", i)
        end = min(x for x in (j, k, len(text)) if x > i)
        sec = text[i:end]
        hits = list(_CALL.finditer(sec))
        for n, m in enumerate(hits):
            out["%s/%s" % (step, m.group(1))] = sec[m.start(): hits[n + 1].start() if n + 1 < len(hits) else len(sec)]
    return out


def judge_steps(text):
    mac = mac_steps(text)
    table = {r[0]: r for r in STEP_ROWS}
    code = {cid: inspect.getsource(fn) for cid, _n, fn in DRIVEN}
    rows = []
    for key, seg in mac.items():
        r = table.get(key)
        if r is None:
            rows.append((key, False, "the Mac journey step has no phone case and no named exclusion"))
            continue
        _k, kind, case, mac_lit, phone_lit, why = r
        if mac_lit not in seg:
            rows.append((key, False, "the Mac step no longer holds %r" % mac_lit))
        elif len(why) < 25:
            rows.append((key, False, "a row needs its reason"))
        elif case and case not in code:
            rows.append((key, False, "phone case %s does not exist" % case))
        elif case and phone_lit not in code[case]:
            rows.append((key, False, "phone case %s no longer holds %r" % (case, phone_lit)))
        else:
            rows.append((key, True, "%s -> %s: %s" % (kind, case or "no phone case", why)))
    for key in table:
        if key not in mac:
            rows.append((key, False, "the table row names a Mac step that is not in journeys/J2.py any more"))
    return rows


def m1_steps(_src):
    rows = judge_steps(journey_text())
    bad = [n for n, ok, _w in rows if not ok]
    return not bad, "%d Mac steps of J2.6 and J2.7 each with a phone case or a named exclusion%s" % (len(rows), ("; red: " + ", ".join(bad)) if bad else "")


STATIC.append(("M1", "every step of the Mac journey J2.6 and J2.7 has a phone case or a named exclusion", m1_steps))


# ---------------------------------------------------------------------------------------------- the model app (the self-test)
class ModelApp:
    """A model of the screen as the sockets serve it. Each mutant makes it fail the way a wrong build would."""
    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.overlay, self.path, self.pin_fail = None, [], None
        self.labels = []
        for i, l in enumerate(FXL):
            self.labels.append(dict(l, id="lbl-%d" % (i + 1)))
        self.pinned = {l["id"] for l in self.labels if l["pinned"]}
        self.messages = [{"id": m["id"], "sender": m["sender"], "from": m["from"].lower(), "subject": m["subject"], "account": m["account"],
                          "received_ms": 1_000_000 - 1000 * i} for i, m in enumerate(FX["messages"])]
        self.people = [{"id": "ct-0", "name": PERSON["name"], "addresses": [a.lower() for a in PERSON["addresses"]]}]
        for m in self.messages:
            if not any(m["from"] in p["addresses"] for p in self.people):
                self.people.append({"id": "ct-%d" % len(self.people), "name": m["sender"], "addresses": [m["from"]]})

    def cur_label(self):
        for p in reversed(self.path):
            if p[0] in ("label", "mail"):
                return next(l for l in self.labels if l["id"] == p[1])
        return None

    def cur_person(self):
        if self.path and self.path[-1][0] == "contact":
            a = self.path[-1][1]
            return next(p for p in self.people if a in p["addresses"])
        return None

    def person_mail(self, p):
        if "contact-wrong-mail" in self.m:
            return list(self.messages)
        return [m for m in self.messages if m["from"] in p["addresses"]]

    def label_mail(self, l):
        return [m for m in self.messages if m["from"].endswith("linea-supply-example.com")] if l["name"] == "Suppliers" else self.messages[:3]

    def uistate(self):
        return {"surface": "iphone-inbox", "overlay": self.overlay}

    def core(self, path):
        if path == "labels/pinned":
            return json.dumps([{"id": i} for i in sorted(self.pinned)])
        return None

    def rows(self, ax):
        r = ax[len(S) + 1:]
        l, p = self.cur_label(), self.cur_person()
        if r == "page":
            kind = self.path[-1][0] if self.path else ("list" if self.overlay == S else "closed")
            return [{"ax": ax, "kind": kind, "depth": len(self.path)}]
        if r == "labels":
            return [{"ax": ax, "id": x["id"], "name": x["name"], "depth": 1 if "labels-flat" in self.m else x["depth"]} for x in self.labels]
        if r == "people":
            return [{"ax": ax, "id": x["id"], "name": x["name"], "addresses": x["addresses"]} for x in self.people]
        if r == "label" and l:
            name = "Unnamed" if "label-name" in self.m else l["name"]
            return [{"ax": ax, "id": l["id"], "name": name, "path": l["path"], "colour": None if "label-colour" in self.m else l["colour"],
                     "meaning": "" if "label-meaning" in self.m else l["meaning"], "pinned": l["id"] in self.pinned,
                     "mail": len(self.label_mail(l)), "edit_note": "" if "edit-note-missing" in self.m else EDIT_NOTE}]
        if r == "label.cards" and l:
            titles = ["Pre-filter", "Classifier", "Post-filter"] if "cards-names" in self.m else CARD_TITLES
            conds = (l["rules"] or {}).get("conditions", [])
            lines = [(KIND[c["kind"]] + " " + (c.get("domain") or c.get("words"))) if "entry-lines" not in self.m
                     else "%s=%s" % (c["kind"], c.get("domain") or c.get("words")) for c in conds] or [NO_RULE]
            mode = ("Any of these" if (l["rules"] or {}).get("match") == "any" else "All of these") if len(conds) > 1 else ""
            return [{"ax": ax + "." + k, "card": k, "title": t, "lines": lines if k == "entry" else (["The AI reads: Subject, Body"] if k == "ai" else []),
                     "chains": [{"lead": "File when", "mode": mode, "lines": lines}] if k == "entry" and conds else []}
                    for k, t in zip(["entry", "ai", "exit"], titles)]
        if r == "mail" and self.path and self.path[-1][0] == "mail":
            return [{"ax": "%s.row.%d" % (ax, i), "id": m["id"]} for i, m in enumerate(self.messages if "mail-wrong" in self.m else self.label_mail(l))]
        if r == "contact" and p:
            addrs = p["addresses"][:1] if "contact-one-address" in self.m else p["addresses"]
            return [{"ax": ax, "name": p["name"], "addresses": addrs, "mark": "none", "mail": len(self.person_mail(p))}]
        if r == "contact.messages" and p:
            return [{"ax": "%s.%d" % (ax, i), "id": m["id"], "subject": m["subject"],
                     "account": "acc-00" if "contact-wrong-account" in self.m else m["account"], "received_ms": m["received_ms"]}
                    for i, m in enumerate(self.person_mail(p))]
        return []

    def click(self, ax):
        r = ax[len(S) + 1:]
        l = self.cur_label()
        if r == "open":
            self.path, self.overlay = [], S
        elif r == "back":
            if "back-dead" in self.m:
                return
            if self.path:
                self.path.pop()
            else:
                self.overlay = None
        elif r.startswith("list.label."):
            self.path.append(("label", r[len("list.label."):]))
        elif r.startswith("list.person."):
            self.path.append(("contact", self.people[int(r.rsplit(".", 1)[1])]["addresses"][0]))
        elif r == "label.pin" and l:
            if "pin-no-write" not in self.m:
                self.pinned ^= {l["id"]}
        elif r == "label.mail" and l:
            if "door-dead" not in self.m:
                self.path.append(("mail", l["id"]))
        elif r.startswith("mail.row.") or r.startswith("contact.messages."):
            if "thread-dead" not in self.m:
                self.overlay = THREAD_OVERLAY
        else:
            raise Refused("no control %s" % ax)


# case id -> the mutants that must turn it red (the self-test checks each one)
MUTANTS = {
    "labels-flat": "L1", "label-name": "L2", "label-colour": "L2", "label-meaning": "L2", "cards-names": "L3", "entry-lines": "L4",
    "edit-note-missing": "L5", "pin-no-write": "L6", "door-dead": "L7", "mail-wrong": "L7", "contact-one-address": "L8",
    "contact-wrong-mail": "L8", "contact-wrong-account": "L8", "back-dead": "L9", "thread-dead": "L9",
}
SOURCE_MUTANTS = {
    "id-removed": ("S1", lambda t: t.replace("iphone-labels-contacts.label.pin", "iphone-labels-contacts.label.pinx")),
    "literal-word": ("S2", lambda t: t.replace("PhoneBackButton { state.back(m) }", 'Text("Hello"); PhoneBackButton { state.back(m) }', 1)),
    "second-toggle": ("S3", lambda t: t.replace("PhoneBackButton { state.back(m) }", 'Toggle(Words.text("Pinned"), isOn: .constant(true)); PhoneBackButton { state.back(m) }', 1)),
    "network": ("S4", lambda t: t.replace("PhoneBackButton { state.back(m) }", "_ = URLSession.shared; PhoneBackButton { state.back(m) }", 1)),
    "figures-stay": ("S5", lambda t: t.replace("let _ = m.mailRevision", "let _ = 0", 1)),
}


JOURNEY_MUTANTS = ["journey-gains-a-step", "journey-moves-a-value"]


def run_driven(app):
    out = []
    for cid, name, fn in DRIVEN:
        try:
            ok, detail = fn(Drive(app))
            out.append((cid, name, "ok" if ok else "FAIL", detail))
        except Refused as e:
            out.append((cid, name, "FAIL", "refused: %s" % e))
        except (KeyError, IndexError, TypeError, StopIteration, ValueError) as e:
            out.append((cid, name, "FAIL", "unreadable: %s %s" % (type(e).__name__, e)))
    return out


def run_static(src):
    return [(cid, name, "ok" if ok else "FAIL", detail) for cid, name, fn in STATIC for ok, detail in [fn(src)]]


def selftest():
    bad = []
    desc = [{"received_ms": 30}, {"received_ms": 20}, {"received_ms": 20}, {"received_ms": 10}]
    for name, rows, want in (("newest first", desc, True), ("oldest first", desc[::-1], False), ("one row", desc[:1], True), ("no rows", [], True)):
        got = newest_first(rows)
        print("  order judge %-13s -> %s" % (name, got))
        if got != want:
            bad.append("newest_first said %s on %s rows" % (got, name))
    base = run_driven(ModelApp()) + run_static(SRC)
    for st, cid in [(r[2], r[0]) for r in base if r[2] != "ok"]:
        bad.append("baseline %s is %s on the model: %s" % (cid, st, [r[3] for r in base if r[0] == cid][0]))
    for m, case in MUTANTS.items():
        got = {r[0]: r[2] for r in run_driven(ModelApp([m]))}
        if got[case] != "FAIL":
            bad.append("mutant %s did not turn %s red" % (m, case))
        print("  mutant %-20s -> %s red: %s" % (m, case, got[case] == "FAIL"))
    for m in JOURNEY_MUTANTS:
        _PATCH.clear()
        _PATCH.add(m)
        got = {r[0]: r[2] for r in run_static(SRC)}
        _PATCH.clear()
        if got["M1"] != "FAIL":
            bad.append("journey mutant %s did not turn M1 red" % m)
        print("  mutant %-20s -> M1 red: %s" % (m, got["M1"] == "FAIL"))
    tmp = tempfile.mkdtemp(prefix="lcphone-selftest-")
    try:
        for m, (case, edit) in SOURCE_MUTANTS.items():
            dst = os.path.join(tmp, m)
            shutil.copytree(SRC, dst)
            changed = False
            for n in os.listdir(dst):
                p = os.path.join(dst, n)
                with open(p, encoding="utf-8") as f:
                    t = f.read()
                u = edit(t)
                if u != t:
                    with open(p, "w", encoding="utf-8") as f:
                        f.write(u)
                    changed = True
            got = {r[0]: r[2] for r in run_static(dst)}
            if not changed or got[case] != "FAIL":
                bad.append("source mutant %s did not turn %s red (edit applied: %s)" % (m, case, changed))
            print("  mutant %-20s -> %s red: %s" % (m, case, changed and got[case] == "FAIL"))
    finally:
        shutil.rmtree(tmp)
    total = len(DRIVEN) + len(STATIC)
    if bad:
        for b in bad:
            sys.stderr.write("SELFTEST FAIL %s\n" % b)
        print("selftest: %d cases, %d mutants, %d problems" % (total, len(MUTANTS) + len(SOURCE_MUTANTS) + len(JOURNEY_MUTANTS), len(bad)))
        return 1
    print("selftest: %d cases green on the model, each red on its mutant (%d model mutants, %d source mutants, %d journey mutants)" % (total, len(MUTANTS), len(SOURCE_MUTANTS), len(JOURNEY_MUTANTS)))
    return 0


# ---------------------------------------------------------------------------------------------- the driven run
def why_not_driven():
    d = os.environ.get("WHOOSH_SOCKET_DIR", "")
    if d and os.path.exists(os.path.join(d, "whoosh.sock")):
        return ""
    return "no iPhone app socket: launch the iPhone app in the hidden rig, then set WHOOSH_SOCKET_DIR (scripts/launch-one.sh)"


def main():
    if "--selftest" in sys.argv:
        return selftest()
    results = run_static(SRC)
    why = why_not_driven()
    if why:
        results += [(cid, name, "NOT JUDGED", why) for cid, name, _ in DRIVEN]
    else:
        from lib.ctl import Ctl
        ctl = Ctl().connect()
        try:
            results += run_driven(ctl)
        finally:
            ctl.close()
    for cid, name, st, detail in results:
        print("%-10s %s %s :: %s" % (st, cid, name, detail))
    for key, ok, why in judge_steps(journey_text()):
        print("%-10s M1 step %s :: %s" % ("ok" if ok else "FAIL", key, why))
    failed = len([r for r in results if r[2] == "FAIL"])
    unjudged = len([r for r in results if r[2] == "NOT JUDGED"])
    print("%d checks, %d failed, %d not judged" % (len(results), failed, unjudged))
    return 0 if failed == 0 and unjudged == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
