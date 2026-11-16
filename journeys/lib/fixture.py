"""Expected values come from the design fixture and docs/CONSTANTS.md, never from a built screen."""
import json
import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
FIXTURE_PATH = os.path.join(ROOT, "docs", "design", "fixtures", "mac-main-sample.json")
CONSTANTS_PATH = os.path.join(ROOT, "docs", "CONSTANTS.md")

with open(FIXTURE_PATH, encoding="utf-8") as _f:
    FX = json.load(_f)

FLOW_EXAMPLE = FX.get("flow_example", [])


def _flow_row(r):
    """A flow_example row as the list serves it: seeded by FixtureFlow.seedFlowMessagesLocked as a Primary message with the body as its one paragraph."""
    sender = r["sender"]
    initials = "".join(w[0] for w in sender.split()[:2]).upper()
    return {"id": r["id"], "sender": sender, "from": r["from"], "initials": initials, "account": r["account"], "subject": r["subject"],
            "preview": r["body"][:160], "time": "", "unread": True, "pinned": False, "attachments": list(r.get("attachments") or []),
            "thread_count": 1, "trackers_blocked": 0, "tab": "primary", "body": [r["body"]], "step": r["step"], "kind": "row"}


# The rows the PRIMARY list serves (docs/lanes/corebridge-d17.md: the flow rows are inserted, outside the tab figures). Only a "rejected"
# row sits in Primary (the label's fallback); the "kept" rows live in the Needs Reply label. SPEC A140 (docs/lanes/flow3-swift.md): step 1 picks every row (its body opens
# "Hi Assem"), the AI accepts the kept and rejected rows, step 3 keeps the kept ones; the file holds no "entry" row any more (flow-entry-1 is a "kept" row), and
# step 1 also picks the corpus mails that open the same way (they wait for the AI): picked 37 = 32 rows + 5 of them, accepted 32, kept 24, to Primary 8.
# The ORDER is the app's, by arrival (docs/lanes/fixture-example.md): FixtureFlow.seedFlowMessagesLocked (FixtureFlow.swift:117-134) walks the file's
# flow_example in file order and inserts each row at the front of `order` (line 125: `order.insert(r.id, at: 0)`), so the last row of the file arrived
# last and stands first; FixtureCore.rows(_:) (FixtureCore.swift:925-933) lists `order` filtered, with the pinned rows moved ahead of the others
# and every other row keeping its place. So the list is: the pinned file messages, the rejected flow rows newest first (the reverse of the file's
# order), then the file's other messages. ROWS is the list as served, so a check that presses J n times lands on ROWS[n].
FLOW_PRIMARY = [_flow_row(r) for r in FLOW_EXAMPLE if r["step"] == "rejected"]          # in the file's order
_PINNED = [m for m in FX["messages"] if m.get("pinned")]
_UNPINNED = [m for m in FX["messages"] if not m.get("pinned")]
ROWS = _PINNED + FLOW_PRIMARY[::-1] + _UNPINNED
ACCOUNT = {a["id"]: a for a in FX["accounts"]}
TAB = {t["id"]: t for t in FX["tabs"]}
PRIMARY_UNREAD = TAB["primary"]["unread"]            # the file's figure; the list serves served_unread("primary", PRIMARY_UNREAD): the file's plus the rejected flow rows
MSG = {m["id"]: m for m in ROWS}
ROW_COUNT = len(ROWS)                                # the file's messages plus the rejected flow rows
FIRST_SENDER = ROWS[0]["sender"]                     # Linea Supply
ACCOUNT_COUNT = len(FX["accounts"])                  # 30
FAILING_ACCOUNT = FX["states"]["sync"]["failing_account"]


# The Superhuman preset as it ships now (docs/lanes/shortcuts.md "The final map", docs/reference/superhuman-shortcuts-v8.json):
# anchor keys with the action the key overlay names. The design fixture's 13-key list is the v1 mock and no longer the preset.
PRESET_ANCHORS = {"J": "next", "K": "previous", "E": "archive", "H": "snooze", "L": "label", "⇧P": "pin", "R": "reply",
                  "Z": "undo", "⌘K": "search", "⌘P": "palette", "Tab": "next split"}
PRESET_KEY_ROWS = 111      # the v8 map (108 chords) plus the Whoosh extras (⌘P palette, ⇧P pin, ⌘\\ sidebar, ⇧H header, ⌘J write a reply draft: owner-approved AI replies, docs/JOURNEYS.md J10, Commands.swift:59) = the fixture "shortcuts" map, as served by mac-trainer.overlay.keys
# Contract delta 16 (docs/lanes/contracts.md "## Delta 16", docs/lanes/inboxlist.md round 10): Primary and All are VIEWS, the strip is
# Primary, All, then the pinned labels. All = every category summed (VIPs is a mailbox, not a category): the file's declared figures,
# Primary 23 and All 23+6+41+117+9 = 196 unread, 4+6+0+0+0 = 10 waiting.
# The flow example's rows are seeded as messages too (FixtureFlow.seedFlowMessagesLocked, all unread): the rejected row in Primary,
# the kept rows in the Needs Reply label (A140: no row stops at the entry card). The strip's figures are the file's plus those rows (served_unread below;
# every figure comes from the file's flow_example rows).
FLOW_UNREAD = {"primary": sum(1 for r in FLOW_EXAMPLE if r["step"] == "rejected"),
               "needs-reply": sum(1 for r in FLOW_EXAMPLE if r["step"] != "rejected")}
FLOW_UNREAD["all"] = FLOW_UNREAD["primary"] + FLOW_UNREAD["needs-reply"]


# The AI card has no flow_example row: the needs-reply label's exit filter is on, so its kept rows carry the exit card. The pinned Provanta label
# (docs/design/fixtures/mac-main-sample.json: lbl-6, description "Everything about the Provanta account: the RFP, the contract, the people.")
# has the AI card on and the exit filter off, so an arrival whose words fit that description is filed under it by the fixture model with the AI
# card (FixtureFiling.fileByModelLocked, exit off). The words avoid the Suppliers description ("Quotes and orders from our suppliers"). A140: the arrival
# opens "Hi Assem", so step 1 of needs-reply (a body rule "Hi Assem") picks it too and both labels are offered to the AI; it shares no word of five letters with
# the needs-reply description (waiting, reply, question, request, decision, someone, emails) and five with the Provanta one (about, account, contract, people,
# provanta), so the fixture model (FixtureFiling.meaningLabelLocked, the best score wins) files it under Provanta only.
AI_VIEW = "lbl-6"
AI_ARRIVAL = {"id": "why-ai-1", "account": "acc-04", "from": "pia@provanta-note-example.be", "sender": "Pia Note", "subject": "Notes for the account review",
              "body": "Hi Assem, a short note about the Provanta contract and the people on the account."}


def write_arrival(name, fields):
    """A one-arrival fixture file for `fixture deliver <name> <path>`: the verb reads `arrivals.<name>` and needs an id and an account."""
    import tempfile
    path = os.path.join(tempfile.mkdtemp(prefix="whoosh-arrival-"), "arrivals.json")
    with open(path, "w", encoding="utf-8") as f:
        json.dump({"arrivals": {name: fields}}, f)
    return path


def served_unread(tab_id, file_unread):
    """A tab's unread badge as the list serves it: the file's figure plus the flow rows seeded into that tab."""
    return file_unread + FLOW_UNREAD.get(tab_id, 0)


ALL_VIEW = {"id": "all", "name": "All",
            "unread": served_unread("all", sum(t["unread"] for t in FX["tabs"] if t["id"] != "vips")),
            "waiting": sum(t["waiting"] for t in FX["tabs"] if t["id"] != "vips")}
TAB_IDS_NOW = ["primary", "all"] + [t["id"] for t in FX["tabs"][1:] if t["name"] not in ("Social", "VIPs")]   # the views, then the built-in tabs (the pinned user label's id is not in the fixture)
TABS_NOW = [FX["tabs"][0]["name"], ALL_VIEW["name"]] + [t["name"] for t in FX["tabs"][1:] if t["name"] not in ("Social", "VIPs")] + ["Provanta"]   # views, then the pinned labels (list round 5); the fixture core pins Provanta


def _num(text):
    m = re.search(r"\d[\d,]*(?:\.\d+)?", text)
    return float(m.group(0).replace(",", "")) if m else None


def const(name):
    """Value of one constant in docs/CONSTANTS.md (first number in its cell)."""
    for line in open(CONSTANTS_PATH, encoding="utf-8"):
        if not line.startswith("|"):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) < 2:
            continue
        names = [n.strip().split(" (")[0] for n in re.split(r"[·/]", cells[0])]
        if name in names:
            vals = [v.strip() for v in re.split(r"·", cells[1])]
            v = vals[names.index(name)] if len(vals) == len(names) else vals[0]
            n = _num(v)
            if n is None:
                raise KeyError("constant %s has no number: %r" % (name, cells[1]))
            return int(n) if n == int(n) else n
    raise KeyError("constant %s not in docs/CONSTANTS.md" % name)
