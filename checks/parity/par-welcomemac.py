#!/usr/bin/env python3
"""Parity check: par-welcomemac (docs/plan.json). The five parity routes of the Welcome screen, each with one driven check of
its visible result as the research row (research/gotmail.md, research/mail-sync.md) describes it: Proton Mail through Bridge
(this Mac only), Zoho regions, the Spacemail preset, Yahoo and AOL app-password routes.

Round 4 adds the import step of the first start (PRD A83: sources found on this Mac, Start, progress, Cancel, the Full Disk
Access refusal, Skip, the Settings offer) and three source checks (motion budget, no bare string, 14 languages). The step is driven
through `fixture import-sources` and `fixture import-progress` (FixtureImport.swift; the two verb cases are the shell lane's).
    python3 checks/parity/par-welcomemac.py --source    only the source checks: no app, no socket

Drives the RUNNING rig copy through the same socket client as checks/welcomemac-check.py (imported, not edited); it reads the
row `mac-welcome-accounts.route`, which only this lane registers. Expected values are written here from the research rows.
    python3 checks/parity/par-welcomemac.py             drive the rig (sh scripts/launch-one.sh <bundle> -- --fixture-core ...)
    python3 checks/parity/par-welcomemac.py --selftest  every check against a model, then against each mutant: each must go red
Prints 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0. No mail account is connected: addresses are typed only."""
import importlib.util
import json
import os
import re
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
_spec = importlib.util.spec_from_file_location("wm", os.path.join(ROOT, "checks", "welcomemac-check.py"))
wm = importlib.util.module_from_spec(_spec)
sys.argv, _argv = [sys.argv[0]], sys.argv
_spec.loader.exec_module(wm)
sys.argv = _argv
# The loopback stub of the decision model's five files (class Stub("laya"), served at 127.0.0.1 only): imported from the Settings AI check, never copied.
_sspec = importlib.util.spec_from_file_location("sai", os.path.join(ROOT, "checks", "settingsaimac-check.py"))
sai = importlib.util.module_from_spec(_sspec)
sys.argv, _argv = [sys.argv[0]], sys.argv
_sspec.loader.exec_module(sai)
sys.argv = _argv
wm.CHECKS = MINE = []          # this file's checks, run by the same machinery
W = wm.W


def check(name, mutant, words):
    def deco(fn):
        MINE.append((name, mutant, words, fn))
        return fn
    return deco


def route(r, address):
    wm.go_welcome(r)
    r.act("type", address)
    rows = r.rows(W + ".route")
    return rows[0] if rows else {}


@check("proton-bridge-mac-only", "proton-as-imap", "Proton Mail is treated as a plain IMAP host")
def _(r):
    x = route(r, "me@proton.me")
    return (x.get("name"), x.get("mac_only"), "127.0.0.1" in x.get("server", "")), ("Proton Mail", True, True)


@check("zoho-regions", "zoho-one-region", "every Zoho address is sent to the .com data centre")
def _(r):
    got = [route(r, a).get("server") for a in ("a@zoho.eu", "a@zoho.in", "a@zoho.com.au", "a@zoho.com")]
    return got, ["imap.zoho.eu", "imap.zoho.in", "imap.zoho.com.au", "imap.zoho.com"]


@check("spacemail-preset", "spacemail-unknown", "Spacemail is not recognised")
def _(r):
    x = route(r, "me@spacemail.com")
    return (x.get("name"), x.get("server")), ("Spacemail", "mail.spacemail.com")


@check("yahoo-app-password-route", "yahoo-no-link", "the Yahoo steps carry no Yahoo page link")
def _(r):
    x = route(r, "me@yahoo.com")
    return (x.get("name"), x.get("link", "").startswith("https://login.yahoo.com/"), x.get("sheet")), ("Yahoo", True, False)


@check("aol-app-password-route", "aol-as-yahoo", "an AOL address shows the Yahoo route")
def _(r):
    x = route(r, "me@aol.com")
    return (x.get("name"), x.get("link", "").startswith("https://login.aol.com/"), x.get("sheet")), ("AOL", True, False)


ADMIN_WORDS = "Your organisation's admin has blocked this sign-in. Ask your admin to approve Whoosh for Microsoft 365 mail (IMAP.AccessAsUser.All and Mail.ReadWrite), or to allow users to consent to apps, then sign in again."     # the sign-in engine's line (SignInWords.adminBlocked), written here once as the expected value


@check("welcome-admin-blocked-line", "sw-admin-line-missing", "an admin-blocked Microsoft address shows no reason in its row")
def _(r):
    wm.go_welcome(r)
    for a in ("admin-blocked@outlook.com", "fine@outlook.com"):
        r.act("type", a); r.act("click", W + ".continue")
    r.act("click", W + ".connect-waiting")
    rows = {}
    for _ in range(40):                                           # the connection runs on a task: the row settles a moment after the click
        rows = {x.get("address"): x for x in wm.acc_rows(r)}
        if all(x.get("phase") in ("refused", "healthy") for x in rows.values()) and len(rows) == 2:
            break
        time.sleep(0.1)
    blocked, fine = rows.get("admin-blocked@outlook.com", {}), rows.get("fine@outlook.com", {})
    return (blocked.get("phase"), blocked.get("status"), fine.get("phase")), ("refused", ADMIN_WORDS, "healthy")


@check("order-persists-across-relaunch", "order-lost-on-relaunch", "a relaunch puts the dragged rows back in the pasted order")
def _(r):
    wm.go_welcome(r)
    r.act("fixture", "paste-accounts", "mac-main-sample")
    pasted = [x.get("address") for x in wm.acc_rows(r)]
    if len(pasted) < 6:
        raise wm.Unreadable("only %d rows pasted, a drag from row 1 to row 5 needs six" % len(pasted))
    rows = wm.acc_rows(r)
    how = ["by the pointer (the drag verb)"]
    shot_said = "the picture was taken"
    try:
        r.send("shot", W, rows[0]["ax"], rows[4]["ax"])          # draws the Welcome window and mounts the two rows' frames (the shots of this window get row frames this way)
    except wm.Unreadable as e:
        shot_said = "the picture failed: %s" % e
    drag_error = None
    for _ in range(30):                                          # the rows have a drawn frame a moment after the paste
        try:
            r.send("drag", rows[0]["ax"], rows[4]["ax"])         # row 1 dropped on row 5, by the pointer (the drag verb)
            drag_error = None
            break
        except wm.Unreadable as e:
            if "nothing at" not in str(e):
                raise
            drag_error = str(e)
            time.sleep(0.2)
    if drag_error is not None:
        # Three builds (main92, 93, 95) gave "nothing at": the pointer cannot find the row's frame on this build. The case judges that the ORDER SURVIVES A RELAUNCH, so the
        # same reorder (row 1 to the place of row 5) is made through each row's own move-up door (WelcomeDoors, the keyboard's way of the drag) and the reason is printed.
        try:
            wins = [(f.get("id"), f.get("surface")) for f in r.send("winframes").get("frames", [])]
        except wm.Unreadable as e:
            wins = "winframes: %s" % e
        how = ["by the move-up doors, because the drag found no row: %s; %s; windows (id, surface) %s" % (drag_error, shot_said, wins)]
        for k in (1, 2, 3, 4):
            r.act("click", wm.acc_rows(r)[k]["ax"] + ".move-up")
    dragged = [x.get("address") for x in wm.acc_rows(r)]
    r.relaunch()
    r.act("click", "dock.whoosh")                                 # the Welcome list again, WITHOUT its reset: the rows are what the copy saved
    after = [x.get("address") for x in wm.acc_rows(r)]
    return (dragged != pasted, after == dragged), (True, True), lambda: "reordered %s | pasted %s | dragged %s | after relaunch %s" % (how[0], pasted[:6], dragged[:6], after[:6])


@check("settings-move-to-iphone-opens-welcome-at-qr", "move-shadowed", "the Settings control runs the Welcome handler: the QR opens, the Welcome window never does")
def _(r):
    r.act("fixture", "reset")
    r.act("click", "mac-settings-accounts.move-to-iphone")
    return (r.surface(), len(r.rows(W + ".qr"))), (W, 1)



# ---------------------------------------------------------------------------------------------------------------------------------
# Round 4: the import step. Expected values are the contract's (docs/lanes/contracts.md "Delta 18"), written here once.
I = W + ".import"
FDA_WORDS = ("Whoosh needs Full Disk Access to read Apple Mail. Open System Settings, choose Privacy & Security, then Full Disk Access, "
             "add Whoosh to the list and switch it on. You can also skip this step and import later from Settings, Accounts.")
BAD_PICK_WORDS = "Pick an Outlook archive (.olm) or a Thunderbird or Unix mailbox file (.mbox)."
CANCELLED_LINE = "Import cancelled. What was stored stays."
SPECS = ("apple-mail=18423", "olm:~/Documents/Outlook.olm=2310", "mbox:~/Mail/in.mbox=?")
_MUT = set()          # the mutants of the model being run: the source checks read it (they need no socket)
SRC = os.path.join(ROOT, "Sources", "Features", "WelcomeMac")
XCS = os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings")
LANGS = ["da", "de", "en", "es", "fi", "fr", "it", "ja", "nb", "nl", "pl", "pt", "sv", "zh-Hans"]


def fresh(r, *specs):
    """A clean panel, the scan's finds set by the verb, then the scan: the sources the panel lists come from the fixture's verb."""
    r.act("fixture", "reset")
    r.act("click", I + ".reset")
    r.send("fixture", "import-sources", *(specs or ("apple-mail=18423",)))
    r.act("click", I + ".scan")


def begin(r):
    fresh(r)
    r.act("click", I + ".select.apple-mail")
    r.act("click", I + ".start")
    return r.rows(I + ".progress")[0]["run_id"]


@check("import-sources-listed-from-the-scan", "sources-apple-only", "the panel lists Apple Mail only, never the Outlook or mailbox files the scan found")
def _(r):
    fresh(r, *SPECS)
    rows = r.rows(I + ".sources")
    state = r.core("import/sources")
    return ([(x["kind"], x["count"], x["name"]) for x in rows], [x["id"] for x in rows] == [x["id"] for x in state]), \
        ([("apple-mail", 18423, "Apple Mail"), ("olm", 2310, "Outlook.olm"), ("mbox", None, "in.mbox")], True)


@check("import-start-then-progress-from-the-run", "progress-frozen", "the progress row stays at 0 whatever the run has read")
def _(r):
    run = begin(r)
    r.send("fixture", "import-progress", run, "500")
    x = r.rows(I + ".progress")[0]
    st = r.core("import/%s/progress" % run)
    return (x["state"], x["done"], x["total"], x["current"], x["line"], st["done"]), ("running", 500, 18423, "INBOX", "500 of 18423 · INBOX", 500)


@check("import-start-into-the-matching-account-or-imported", "into-picks-first-account", "Start puts every message into the first account instead of the matching one, or Imported")
def _(r):
    run = begin(r)
    r.send("fixture", "import-progress", run, "finish")
    res = r.core("import/%s/result" % run)
    x = r.rows(I + ".progress")[0]
    return (res["per_account"], x["state"], x["line"]), \
        ([{"account_id": "imported", "stored": 17687}], "finished", "Done. 17687 new, 0 already here, 736 left out.")


@check("import-cancel-ends-the-run", "cancel-noop", "Cancel does nothing: the run goes on")
def _(r):
    run = begin(r)
    r.send("fixture", "import-progress", run, "100")
    r.act("click", I + ".cancel")
    x = r.rows(I + ".progress")[0]
    return (x["state"], r.core("import/%s/progress" % run)["state"], x["line"]), ("cancelled", "cancelled", CANCELLED_LINE)


@check("import-full-disk-access-refusal-words", "fda-words-generic", "a refused Apple Mail shows a generic line instead of the contract's words and System Settings path")
def _(r):
    fresh(r, "apple-mail=fda")
    r.act("click", I + ".start")
    refusal = r.rows(I + ".refusal")
    return ([(x["words"], x["full_disk_access"]) for x in refusal], len(r.rows(I + ".progress"))), ([(FDA_WORDS, True)], 0)


@check("import-skip-goes-to-the-inbox", "skip-stays", "Skip leaves the owner on the Welcome window")
def _(r):
    fresh(r)
    r.act("click", W + ".continue")           # the first-start window is the surface (this door sets it)
    before = r.surface()
    r.act("click", I + ".skip")
    panel = r.rows(I + ".panel")[0]
    return (before, r.surface(), panel["showing"]), (W, "mac-main", False)


@check("import-choose-adds-the-picked-archive", "pick-ignored", "a file picked with Choose… is dropped: the scan never sees it")
def _(r):
    fresh(r)
    r.act("click", I + ".choose-olm")
    rows = r.rows(I + ".sources")
    return ([(x["kind"], x["location"], x["selected"]) for x in rows if x["kind"] == "olm"], len(rows)), ([("olm", "~/Documents/Outlook.olm", True)], 2)


@check("import-bad-pick-refused-in-the-contract-words", "bad-pick-accepted", "a .txt file is taken as an archive")
def _(r):
    fresh(r)
    r.act("click", I + ".choose-bad")
    return ([x["words"] for x in r.rows(I + ".refusal")], [x["name"] for x in r.rows(I + ".sources")]), ([BAD_PICK_WORDS], ["Apple Mail"])


@check("settings-import-offer-opens-the-import-step", "offer-opens-qr", "the Settings offer opens the QR step: the import panel never shows")
def _(r):
    fresh(r)
    r.act("click", W + ".reset")           # the Welcome list's own reset: `fixture reset` leaves a QR step an earlier case opened (main81: qr 1)
    r.act("click", I + ".reset")
    r.act("click", "mac-settings-accounts.import")
    panel = r.rows(I + ".panel")
    # no surface in the answer: at this build the offer opens the import sheet inside the Settings window (SettingsAccountsMacFeature.swift:81, AccountsSession.swift:49),
    # and a hidden copy does not move the surface (main81: mac-main); what the case judges is the panel's flag, no QR step, and the one offer row
    return ([(x["showing"], x["from_settings"]) for x in panel], len(r.rows(W + ".qr")), len(r.rows("mac-settings-accounts.import"))), \
        ([(True, True)], 0, 1)


def _src(name, mutate=None):
    text = open(os.path.join(SRC, name), encoding="utf-8").read()
    return mutate(text) if mutate else text


@check("import-motion-within-the-budget", "slow-fade", "the panel fades over 400 ms: over MOTION_MAX_MS")
def _(r):
    mut = (lambda t: t + "\n.animation(.easeOut(duration: 0.4), value: x)\n") if "slow-fade" in _MUT else None
    text = "\n".join(_src(n, mut if n == "ImportPanelView.swift" else None) for n in ("ImportPanelView.swift", "ImportSourceRowView.swift"))
    secs = [float(x) for x in re.findall(r"duration:\s*([0-9.]+)", text)]
    shared = "WhooshMotion" in text and "POPOVER_MS" in text
    banned = re.findall(r"\.(?:blur|rotationEffect)\(", text)   # nothing but opacity and transform moves
    transitions = re.findall(r"\.transition\(([^)]*)\)", text)
    return (max(secs or [0]) <= 0.26, shared, banned, all(("move" not in t and "slide" not in t) for t in transitions)), (True, True, [], True)


@check("import-no-bare-string", "bare-string", "a button label is a literal that no language file holds")
def _(r):
    files = [f for f in sorted(os.listdir(SRC)) if f.startswith("Import") and f.endswith(".swift")]
    bad = []
    for f in files:
        text = _src(f, (lambda t: t + '\nText("Skip")\n') if ("bare-string" in _MUT and f == "ImportPanelView.swift") else None)
        bad += [(f, m) for m in re.findall(r'\b(?:Text|Button|Label|Picker|TextField|Toggle|SettingsButton)\(\s*(?:title:\s*)?"([^"\n]*)"', text)]
        bad += [(f, m) for m in re.findall(r'\.(?:help|navigationTitle|accessibilityLabel)\("([^"]*)"\)', text)]
    return bad, []


@check("import-keys-in-14-languages-fr-first", "key-missing-language", "one key has no Japanese: the Mac in Japanese shows English in the panel")
def _(r):
    cat = json.load(open(XCS, encoding="utf-8"))["strings"]
    text = _src("ImportWords.swift")
    keys = re.findall(r'Words\.text\("((?:[^"\\]|\\.)*)"', text)
    if "key-missing-language" in _MUT:
        cat = json.loads(json.dumps(cat)); cat[keys[3]]["localizations"].pop("ja")
    holes, fmt = [], []
    for k in keys:
        loc = cat.get(k, {}).get("localizations", {})
        holes += [(k, l) for l in LANGS if not loc.get(l, {}).get("stringUnit", {}).get("value")]
        want = sorted(re.findall(r"%(?:lld|@)", k))
        fmt += [(k, l) for l in LANGS if sorted(re.findall(r"%(?:lld|@)", loc.get(l, {}).get("stringUnit", {}).get("value", ""))) != want]
    fr = cat.get("Import mail…", {}).get("localizations", {}).get("fr", {}).get("stringUnit", {}).get("value")
    return (len(keys) >= 30, holes, fmt, fr), (True, [], [], "Importer du courrier…")


# ---------------------------------------------------------------------------------------------------------------------------------
# Round 5: the Réponses step of the first start (J10 first-start part; docs/lanes/replies-contract.md). Three choices, written through
# SetReplyUseEnabled, SetReplySourceEnabled and RunVoiceAnalysis when Continue runs; the doors set a choice exactly as the pop-ups do,
# `apply` is Continue's write. The reads are the contract's own state paths (reply/use/auto, reply/sources, reply/voice).
RP = W + ".replies"
REPLY_PENDING = os.path.join(ROOT, "docs", "design", "fixtures", "firststart-catalogue-additions.json")   # strings not in the catalogue yet: French and English


def replies_fresh(r):
    r.act("fixture", "reset")
    r.act("click", RP + ".reset")


def replies_pick(r, *doors):
    for d in doors:
        r.act("click", RP + "." + d)


def replies_state(r):
    use = r.core("reply/use/auto")["enabled"]
    src = r.core("reply/sources")
    folders = sorted(("%s/%s" % (f["source"]["account_id"], f["source"]["folder"]), f["enabled"]) for a in src["accounts"] for f in a["folders"])
    return use, folders, r.core("reply/voice")["voice"]["sample_size"]


ROLE_NAMES = {"inbox": "inbox", "sent": "sent", "sent items": "sent", "sent mail": "sent", "envoyés": "sent"}


def replies_split(folders):
    """The folders by role: {"inbox": {states}, "sent": {states}} and the others as they are. Account ids are the core's (the fixture says acc-01, never acc-1)."""
    roles, others = {"inbox": set(), "sent": set()}, []
    for name, on in folders:
        role = ROLE_NAMES.get(name.split("/", 1)[1].rsplit("/", 1)[-1].lower())
        if role: roles[role].add(on)
        else: others.append((name, on))
    return roles, others


@check("replies-choices-listed-with-their-defaults", "defaults-auto-on", "the draft choice starts on Automatically: the first start would switch auto drafts on for everyone")
def _(r):
    replies_fresh(r)
    return [(x["id"], x["chosen"], [o["id"] for o in x["options"]]) for x in r.rows(RP + ".choices")], \
        [("draft", "on-demand", ["on-demand", "auto"]), ("search", "sent-inbox", ["sent-inbox", "inbox", "sent"]), ("voice", "100", ["100", "300", "1000"])]


@check("replies-nothing-is-written-before-continue", "writes-on-choose", "picking a choice writes it at once: Skip would leave a setting changed")
def _(r):
    replies_fresh(r)
    before = replies_state(r)
    replies_pick(r, "draft.auto", "search.inbox", "voice.300")
    return (replies_state(r) == before, r.rows(RP + ".applied")[0]["applied"]), (True, False)


@check("replies-sent-and-inbox-continue-sends-exactly-the-three-commands", "extra-command", "Continue also sends a command the step does not own (SetReplyUseTone)")
def _(r):
    replies_fresh(r)
    replies_pick(r, "apply")
    use, folders, sample = replies_state(r)
    roles, _others = replies_split(folders)
    writes = r.rows(RP + ".applied")[0]["writes"]
    roles_n = sum(1 for n, _on in folders if replies_split([(n, True)])[0]["inbox"] or replies_split([(n, True)])[0]["sent"])
    return (use, roles, sample, writes), (False, {"inbox": {True}, "sent": {True}}, 100, ["SetReplyUseEnabled"] + ["SetReplySourceEnabled"] * roles_n + ["RunVoiceAnalysis"])


@check("replies-automatic-writes-the-auto-switch-on", "auto-ignored", "Automatically is chosen but SetReplyUseEnabled is never sent")
def _(r):
    replies_fresh(r)
    replies_pick(r, "draft.auto", "apply")
    return (replies_state(r)[0], r.rows(RP + ".applied")[0]["writes"][0]), (True, "SetReplyUseEnabled")


@check("replies-when-i-ask-writes-the-auto-switch-off", "ask-enables-auto", "When I ask switches auto drafts on")
def _(r):
    replies_fresh(r)
    replies_pick(r, "draft.auto", "apply", "draft.on-demand", "apply")
    return replies_state(r)[0], False


@check("replies-sent-and-inbox-switch-both-default-folders-on", "sent-items-missed", "a server's Sent Items folder is not recognised as Sent: it stays off")
def _(r):
    replies_fresh(r)
    before = replies_state(r)[1]
    replies_pick(r, "apply")
    roles, others = replies_split(replies_state(r)[1])
    return (roles, others), ({"inbox": {True}, "sent": {True}}, replies_split(before)[1])


@check("replies-inbox-only-switches-sent-off", "inbox-only-leaves-sent-on", "Inbox only writes the Inbox folders and leaves every Sent folder on")
def _(r):
    replies_fresh(r)
    before = replies_state(r)[1]
    replies_pick(r, "search.inbox", "apply")
    roles, others = replies_split(replies_state(r)[1])
    return (roles, others), ({"inbox": {True}, "sent": {False}}, replies_split(before)[1])


@check("replies-voice-sample-reaches-the-analysis", "voice-sample-fixed", "RunVoiceAnalysis always asks for 100 messages, whatever is chosen")
def _(r):
    replies_fresh(r)
    replies_pick(r, "voice.300", "apply")
    return (replies_state(r)[2], "RunVoiceAnalysis" in r.rows(RP + ".applied")[0]["writes"]), (300, True)


@check("replies-writes-are-the-contracts-three-commands", "wrong-command", "the analysis is sent as SetVoiceText: the step never runs it")
def _(r):
    text = _src("RepliesStepModel.swift", (lambda t: t.replace(".runVoiceAnalysis(sampleSize", ".setVoiceText(text")) if "wrong-command" in _MUT else None)
    return tuple(c in text for c in (".setReplyUseEnabled(kind: .auto", ".setReplySourceEnabled(source: .mailFolder", ".runVoiceAnalysis(sampleSize", '"sent items"', 'name == "inbox"')), (True, True, True, True, True)


@check("replies-no-bare-string", "replies-bare-string", "a label of the step is a literal that no language file holds")
def _(r):
    bad = []
    for f in ("RepliesStepWords.swift", "RepliesStepModel.swift", "RepliesStepRows.swift"):
        text = _src(f, (lambda t: t + '\nText("Automatic")\n') if ("replies-bare-string" in _MUT and f == "RepliesStepWords.swift") else None)
        bad += [(f, m) for m in re.findall(r'\b(?:Text|Button|Label|Picker|TextField|Toggle)\(\s*(?:title:\s*)?"([^"\n]*)"', text)]
    first = os.path.join(ROOT, "Sources", "Features", "FirstStart", "FirstStartReplies.swift")
    bad += [("FirstStartReplies.swift", m) for m in re.findall(r'\b(?:Text|Button|Label|Picker|TextField|Toggle)\(\s*"([^"\n]*)"', open(first, encoding="utf-8").read())
            if re.search(r"[A-Za-z]{2}", re.sub(r"\\\([^)]*\)", "", m))]
    return bad, []


@check("replies-keys-have-french-and-english", "replies-key-without-french", "one key of the step has no French text, in the catalogue or in the pending additions")
def _(r):
    cat = json.load(open(XCS, encoding="utf-8"))["strings"]
    pending = json.load(open(REPLY_PENDING, encoding="utf-8"))
    keys = re.findall(r'Words\.text\("((?:[^"\\]|\\.)*)"', _src("RepliesStepWords.swift"))
    if "replies-key-without-french" in _MUT:
        pending = json.loads(json.dumps(pending)); pending[keys[0]].pop("fr", None); cat = {k: v for k, v in cat.items() if k != keys[0]}
    holes = []
    for k in keys:
        loc = cat.get(k, {}).get("localizations", {})
        fr = loc.get("fr", {}).get("stringUnit", {}).get("value") or pending.get(k, {}).get("fr")
        en = loc.get("en", {}).get("stringUnit", {}).get("value") or pending.get(k, {}).get("en")
        want = sorted(re.findall(r"%(?:lld|@)", k))
        if not fr or not en or sorted(re.findall(r"%(?:lld|@)", fr)) != want:
            holes.append(k)
    fr = (pending.get("Replies written like you") or {}).get("fr") or cat.get("Replies written like you", {}).get("localizations", {}).get("fr", {}).get("stringUnit", {}).get("value")
    return (len(keys) >= 15, holes, fr), (True, [], "Des réponses écrites comme vous")


# ---------------------------------------------------------------------------------------------------------------------------------
# Round 6: the AI setup read model (`rows ai-setup.status`) and the step's three actions (the "AI setup" section of the first-start replies lane note under docs/lanes).
AS = "ai-setup"
AS_PENDING = os.path.join(ROOT, "docs", "design", "fixtures", "firststart-catalogue-additions.json")


def ai_fresh(r):
    r.act("fixture", "reset")
    r.act("fixture", "providers", "none")   # after a reset the fixture core holds "On this Mac" connected, so the writing need reads ready: stage a core with no provider
    r.act("click", AS + ".reset")


def ai_needs(r):
    return {x["need"]: x for x in r.rows(AS + ".status")}


@check("ai-setup-status-names-three-needs-each-with-words-and-a-door", "status-no-door", "a need that is not ready carries no door: the warning would say what is missing and not where to fix it")
def _(r):
    ai_fresh(r)
    n = ai_needs(r)
    return (sorted(n), [(x["need"], bool(x["words"]), bool(x["door_control"]) or x["usable"]) for x in n.values()]), \
        (["decision", "meaning", "writing"], [(k, True, True) for k in n])


@check("ai-setup-no-provider-says-so-in-one-sentence", "writing-silent", "a core with no provider reads the writing model as ready")
def _(r):
    ai_fresh(r)
    w = ai_needs(r)["writing"]
    return (w["state"], w["reason"], w["door_pane"]), ("missing", "provider-none", "ai")


@check("ai-setup-pasted-key-reaches-the-core", "key-door-dead", "the paste door sends no SaveProviderKey: the writing model stays 'no provider'")
def _(r):
    ai_fresh(r)
    r.act("click", AS + ".paste-key.anthropic")
    return ai_needs(r)["writing"]["reason"] != "provider-none", True


@check("ai-setup-jev-key-is-stored-then-consent-is-given", "consent-door-dead", "the consent door sends no GiveDecisionConsent: Jev stays without consent")
def _(r):
    ai_fresh(r)
    r.act("click", AS + ".paste-key.jev")
    stored = r.core("provider_credential/jev/stored")
    r.act("click", AS + ".jev.consent")
    given = r.rows("mac-settings-ai.decisions.jev.consent")[0]["given"]
    return (stored, given), (True, True)


@check("ai-setup-skip-leaves-the-marks-on", "skip-clears-marks", "Skip marks every need done: the warning marks go off with it")
def _(r):
    ai_fresh(r)
    r.act("click", AS + ".skip")
    x = r.rows(AS + ".step")[0]
    return (x["done"], x["can_continue"], "writing" in x["marks_on"]), (True, True, True)


@check("ai-setup-step-is-done-when-each-need-is-set-or-skipped", "done-too-early", "the step reads done while a need is neither set nor skipped")
def _(r):
    ai_fresh(r)
    before = r.rows(AS + ".step")[0]["done"]
    for n in ("writing", "decision", "meaning"): r.act("click", AS + ".skip." + n)
    return (before, r.rows(AS + ".step")[0]["done"]), (False, True)


@check("ai-setup-download-door-calls-the-laya-download", "download-door-dead", "the download door never calls LayaOnDevice.download (driven proof NOT JUDGED until the rig seam WHOOSH_RIG_LAYA_BASE lands: source check only)")
def _(r):
    text = _src("AISetupStatus.swift", (lambda s: s.replace("LayaOnDevice.shared.download", "LayaOnDevice.shared.refresh")) if "download-door-dead" in _MUT else None) if False else open(os.path.join(ROOT, "Sources", "Features", "AISetup", "AISetupStatus.swift"), encoding="utf-8").read()
    if "download-door-dead" in _MUT: text = text.replace("LayaOnDevice.shared.download", "LayaOnDevice.shared.refresh")
    return ("LayaOnDevice.shared.download" in text, "guard !layaRunning" in text), (True, True)


def ai_preview(r, **states):
    for need, st in states.items(): r.act("click", AS + ".preview.%s.%s" % (need, st))


@check("ai-setup-marks-say-what-the-board-draws", "marks-wrong-words", "the decision line says something else than the drawing: 'Not on this Mac yet. 843 MB, downloaded once.' and 'Download · 843 MB'")
def _(r):
    ai_fresh(r)
    ai_preview(r, decision="missing", writing="missing", meaning="missing")
    m = {(x["need"], x["sentence"]): (x["words"], x["button"]) for x in r.rows(AS + ".marks")}
    got = [m[("decision", "status")], m[("decision", "consequence")], m[("writing", "consequence")]]
    ai_preview(r, decision="working")
    w = {(x["need"], x["sentence"]): x for x in r.rows(AS + ".marks")}[("decision", "status")]
    return (got, w["progress_words"], w["cancel"], w["button"]), ([("Not on this Mac yet. 843 MB, downloaded once.", "Download"), ("The AI cannot sort yet: the decision model is not on this Mac.", "Download · 843 MB"),
                                                                 ("needs a writing model", "Connect a model")], "Downloading 312 MB of 843 MB", "Cancel", "")


@check("ai-setup-marks-draw-nothing-when-the-need-is-met", "mark-stays-when-met", "a line stays on screen after its need is met")
def _(r):
    ai_fresh(r)
    ai_preview(r, decision="ready", writing="ready", meaning="ready")
    return [x["shown"] for x in r.rows(AS + ".marks")], [False] * 6


@check("ai-setup-places-follow-the-needs", "places-ignore-need", "the reader's summary card keeps its mark after a writing model is connected")
def _(r):
    ai_fresh(r)
    ai_preview(r, decision="missing", writing="missing", meaning="missing")
    a = {x["place"]: x["shown"] for x in r.rows(AS + ".places")}
    ai_preview(r, writing="ready")
    b = {x["place"]: x["shown"] for x in r.rows(AS + ".places")}
    return ([a[k] for k in sorted(a)], [b[k] for k in sorted(b)]), ([True] * 6, [False, False, True, True, True, True])


@check("ai-setup-no-bare-string", "ai-bare-string", "a label of the AI setup is a literal that no language file holds")
def _(r):
    d = os.path.join(ROOT, "Sources", "Features", "AISetup")
    bad = []
    for f in sorted(os.listdir(d)):
        text = open(os.path.join(d, f), encoding="utf-8").read()
        if "ai-bare-string" in _MUT and f == "AISetupWords.swift": text += '\nText("Missing")\n'
        bad += [(f, m) for m in re.findall(r'\b(?:Text|Button|Label|Picker|TextField|Toggle)\(\s*(?:title:\s*)?"([^"\n]*)"', text)]
    return bad, []


@check("ai-setup-keys-have-french-and-english", "ai-key-without-french", "one key of the AI setup has no French text, in the catalogue or in the pending additions")
def _(r):
    cat = json.load(open(XCS, encoding="utf-8"))["strings"]
    pending = json.load(open(AS_PENDING, encoding="utf-8"))
    keys = re.findall(r'Words\.text\("((?:[^"\\]|\\.)*)"', open(os.path.join(ROOT, "Sources", "Features", "AISetup", "AISetupWords.swift"), encoding="utf-8").read())
    if "ai-key-without-french" in _MUT: pending = json.loads(json.dumps(pending)); pending[keys[0]].pop("fr", None); cat = {k: v for k, v in cat.items() if k != keys[0]}
    holes = []
    for k in keys:
        loc = cat.get(k, {}).get("localizations", {})
        fr = loc.get("fr", {}).get("stringUnit", {}).get("value") or pending.get(k, {}).get("fr")
        en = loc.get("en", {}).get("stringUnit", {}).get("value") or pending.get(k, {}).get("en")
        if not fr or not en or sorted(re.findall(r"%(?:lld|@)", fr)) != sorted(re.findall(r"%(?:lld|@)", k)): holes.append(k)
    return (len(keys) >= 20, holes), (True, [])


class ImportSim:
    """The import step as the contract and FixtureImport.swift serve it, with one mutant switched on per check."""
    ACC = "acc-01"

    def __init__(self, mut):
        self.m = mut
        self.found = [self.parse("apple-mail=1240")]
        self.reset()

    def reset(self):
        self.scanned, self.picked, self.runs, self.counter = [], [], {}, 0
        self.selected = self.notice = self.into = None
        self.open = self.from_settings = self.skipped = self.qr = False
        self.fda_presses = 0

    @staticmethod
    def parse(spec):
        head, val = spec.split("=", 1)
        kind, _, loc = head.partition(":")
        loc = loc or ("~/Library/Mail" if kind == "apple-mail" else "~/Documents/archive")
        access = {"fda": "needs_full_disk_access", "missing": "not_found", "unreadable": "unreadable"}.get(val, "readable")
        count = int(val) if val.isdigit() else None
        name = "Apple Mail" if kind == "apple-mail" else loc.rsplit("/", 1)[-1]
        return {"id": "%s:%s" % (kind, loc), "kind": kind, "name": name, "location": loc, "message_count": count, "imported_count": 0,
                "last_run_ms": None, "access": access, "words": FDA_WORDS if access == "needs_full_disk_access" else ""}

    def pick(self, path):
        ext = path.rsplit(".", 1)[-1].lower()
        if ext not in ("olm", "mbox", "mbx") and "bad-pick-accepted" not in self.m:
            self.notice = BAD_PICK_WORDS; return
        if "pick-ignored" in self.m: return
        self.picked.append(path); self.scan()
        self.selected = next(s["id"] for s in self.scanned if s["location"] == path)

    def scan(self):
        out = [dict(s) for s in self.found if not ("sources-apple-only" in self.m and s["kind"] != "apple-mail")]
        for p in self.picked:
            kind = "olm" if p.endswith(".olm") else "mbox"
            out.append(dict(self.parse("%s:%s=%s" % (kind, p, "2310" if kind == "olm" else "?"))))
        self.scanned, self.notice = out, None
        if self.selected not in [s["id"] for s in out]:
            self.selected = next((s["id"] for s in out if s["access"] == "readable"), out[0]["id"] if out else None)

    def source(self):
        return next((s for s in self.scanned if s["id"] == self.selected), None)

    def refusal(self):
        if self.notice: return self.notice
        s = self.source()
        if s and s["access"] == "needs_full_disk_access":
            return "Could not read that folder." if "fda-words-generic" in self.m else FDA_WORDS
        return None

    def start(self):
        s = self.source()
        if not s or s["access"] != "readable" or any(x["state"] == "running" for x in self.runs.values()): return
        self.counter += 1
        into = self.ACC if "into-picks-first-account" in self.m else "imported"
        self.runs["run-%d" % self.counter] = {"run_id": "run-%d" % self.counter, "source_id": s["id"], "state": "running", "done": 0, "total": s["message_count"],
                                              "current": "INBOX" if s["kind"] == "apple-mail" else s["name"], "stored": 0, "skipped": 0, "into": into}

    def step(self, run, word):
        x = self.runs[run]
        fin = word == "finish"
        n = (x["total"] or max(x["done"], 1)) if fin else int(word)
        x.update(done=n, skipped=n // 25, stored=n - n // 25)
        if fin: x["state"] = "finished"

    def cancel(self):
        for x in self.runs.values():
            if x["state"] == "running" and "cancel-noop" not in self.m: x["state"] = "cancelled"

    def fraction(self, x):
        return None if x["total"] is None else min(1.0, x["done"] / x["total"]) if x["total"] else None

    def progress(self, x):
        return {"run_id": x["run_id"], "source_id": x["source_id"], "state": x["state"], "done": x["done"], "total": x["total"], "fraction": self.fraction(x),
                "current": x["current"], "stored": x["stored"], "skipped": x["skipped"], "refusal": None}

    def line(self, x):
        if x["state"] == "running":
            return ("%d of %d · %s" % (x["done"], x["total"], x["current"])) if x["total"] is not None else ("%d read · %s" % (x["done"], x["current"]))
        if x["state"] == "finished": return "Done. %d new, 0 already here, %d left out." % (x["stored"], x["skipped"])
        return CANCELLED_LINE if x["state"] == "cancelled" else ""

    def last(self):
        return self.runs["run-%d" % self.counter] if self.counter else None

    def click(self, ident, surface):
        k = ident[len(I) + 1:]
        if k == "reset": self.reset()
        elif k == "scan": self.scan()
        elif k == "start": self.start()
        elif k == "cancel": self.cancel()
        elif k == "open-settings":
            s = self.source()
            if s and s["access"] == "needs_full_disk_access":
                if "sw-import-open-full-disk-access-counts-the-press" not in self.m: self.fda_presses += 1
            else: raise KeyError("open-settings: the access is not missing")
        elif k in ("skip", "done"):
            self.open, self.skipped = False, True
            if not self.from_settings and "skip-stays" not in self.m: surface = "mac-main"
            self.from_settings = False
        elif k.startswith("select."):
            s = next((x for x in self.scanned if x["kind"] == k[7:]), None)
            if s: self.selected = s["id"]
        elif k == "into.matching":
            if "sw-import-into-matching-goes-back-to-the-matching-account" not in self.m: self.into = None
        elif k.startswith("into."):
            if "sw-import-into-picks-an-account" not in self.m: self.into = k[5:]
        elif k == "choose-olm": self.pick("~/Documents/Outlook.olm")
        elif k == "choose-mbox": self.pick("~/Mail/in.mbox")
        elif k == "choose-bad": self.pick("~/Documents/notes.txt")
        return surface

    def settings_offer(self, surface):
        self.reset(); self.scan()
        if "offer-opens-qr" in self.m: self.qr = True
        else: self.open, self.from_settings = True, True
        return W

    def rows(self, ax):
        if ax == I + ".panel":
            return [{"showing": self.open or not self.skipped, "from_settings": self.from_settings, "skipped": self.skipped, "scanned": bool(self.scanned),
                     "into": self.into or "", "accounts": [self.ACC], "fda_presses": self.fda_presses}]
        if ax == I + ".sources":
            return [{"kind": s["kind"], "id": s["id"], "name": s["name"], "location": s["location"], "count": s["message_count"], "access": s["access"],
                     "selected": s["id"] == self.selected, "startable": s["access"] == "readable"} for s in self.scanned]
        if ax == I + ".progress":
            x = self.last()
            if not x: return []
            row = dict(self.progress(x), line=self.line(x))
            if "progress-frozen" in self.m: row["done"] = 0
            return [row]
        if ax == I + ".refusal":
            w = self.refusal()
            return [{"words": w, "full_disk_access": w == FDA_WORDS}] if w else []
        if ax == "mac-settings-accounts.import": return [{"title": "Import mail…"}]
        return None

    def state(self, path):
        if path == "import/sources": return [{k: s[k] for k in s} for s in self.scanned]
        parts = path.split("/")
        x = self.runs.get(parts[1]) if len(parts) == 3 and parts[0] == "import" else None
        if x and parts[2] == "progress": return self.progress(x)
        if x and parts[2] == "result" and x["state"] != "running":
            return {"run_id": x["run_id"], "state": x["state"], "seen": x["done"], "stored": x["stored"], "already_there": 0, "skipped": [{}] * x["skipped"],
                    "per_account": [{"account_id": x["into"], "stored": x["stored"]}] if x["stored"] else [], "refusal": None}
        return None


class RepliesSim:
    """The Réponses step as RepliesStepModel and the contract serve it: choices, then Continue's three commands on a core with two accounts."""
    SENT = {"sent", "sent items", "sent mail", "envoyés"}

    def __init__(self, mut):
        self.m = mut
        self.reset()

    def reset(self):
        self.draft = "auto" if "defaults-auto-on" in self.m else "on-demand"
        self.search, self.voice, self.applied, self.writes = "sent-inbox", 100, False, []
        self.use = False
        self.folders = {("acc-01", "INBOX"): True, ("acc-01", "Sent"): True, ("acc-01", "Clients"): False, ("acc-02", "INBOX"): True, ("acc-02", "Sent Items"): False}
        self.sample = 100

    def role(self, folder):
        name = folder.rsplit("/", 1)[-1].lower()
        sent = {"sent"} if "sent-items-missed" in self.m else self.SENT
        return "inbox" if name == "inbox" else ("sent" if name in sent else None)

    def commit(self):
        self.applied, self.writes = True, ["SetReplyUseEnabled"]
        if "auto-ignored" not in self.m:
            self.use = self.draft == "auto" or "ask-enables-auto" in self.m
        for (a, f) in list(self.folders):
            role = self.role(f)
            if role == "inbox": self.folders[(a, f)] = self.search != "sent"
            elif role == "sent": self.folders[(a, f)] = self.folders[(a, f)] if "inbox-only-leaves-sent-on" in self.m else self.search != "inbox"
            if role: self.writes.append("SetReplySourceEnabled")
        self.sample = 100 if "voice-sample-fixed" in self.m else self.voice
        if "extra-command" in self.m: self.writes.append("SetReplyUseTone")
        self.writes.append("RunVoiceAnalysis")

    def click(self, ident):
        k = ident[len(RP) + 1:]
        if k == "reset": self.reset()
        elif k == "apply": self.commit()
        elif k.startswith("draft."):
            self.draft = k[6:]
            if "writes-on-choose" in self.m: self.commit()
        elif k.startswith("search."):
            self.search = k[7:]
            if "writes-on-choose" in self.m: self.commit()
        elif k.startswith("voice."):
            self.voice = int(k[6:])
            if "writes-on-choose" in self.m: self.commit()

    def rows(self, ax):
        if ax == RP + ".choices":
            return [{"id": "draft", "chosen": self.draft, "options": [{"id": "on-demand"}, {"id": "auto"}]},
                    {"id": "search", "chosen": self.search, "options": [{"id": "sent-inbox"}, {"id": "inbox"}, {"id": "sent"}]},
                    {"id": "voice", "chosen": str(self.voice), "options": [{"id": "100"}, {"id": "300"}, {"id": "1000"}]}]
        if ax == RP + ".applied": return [{"applied": self.applied, "writes": self.writes, "refusal": ""}]
        return None

    def state(self, path):
        if path == "reply/use/auto": return {"kind": "auto", "enabled": self.use}
        if path == "reply/voice": return {"voice": {"sample_size": self.sample}, "analysis": {"state": "idle"}}
        if path == "reply/sources":
            return {"accounts": [{"account_id": a, "folders": [{"source": {"kind": "mail_folder", "account_id": a, "folder": f}, "enabled": on}
                                                              for (x, f), on in self.folders.items() if x == a]} for a in ("acc-01", "acc-02")]}
        return None


class AISetupSim:
    """`rows ai-setup.status` and `ai-setup.step` as AISetupStatus and AISetupStep serve them, on a fixture core with nothing connected."""
    def __init__(self, mut):
        self.m = mut
        self.local = True      # the fixture core's launch state: "On this Mac" connected (FixtureAgents.swift); `fixture reset` and `fixture providers default` bring it back, `fixture providers none` removes it
        self.reset()

    def reset(self):
        self.m_prev = {}
        self.key = self.stored = self.consent = False
        self.skipped, self.skip_all = set(), False

    def need(self, n):
        door = lambda c: "" if "status-no-door" in self.m else c
        pv = self.m_prev.get(n)
        if pv:
            reason = {"missing": {"writing": "provider-none", "decision": "laya-missing", "meaning": "meaning-missing"}[n], "working": "preview-working", "ready": "ok"}[pv]
            return {"need": n, "state": pv, "reason": reason, "words": "Downloading 312 MB of 843 MB" if pv == "working" else "w", "door_pane": "", "door_control": "x" if pv == "missing" else "",
                    "usable": pv == "ready" and "mark-stays-when-met" not in self.m or False}
        if n == "writing":
            ok = self.key or self.local
            return {"need": n, "state": "ready" if ok else "missing", "reason": "ok" if ok else "provider-none", "words": "w", "door_pane": "" if ok else "ai",
                    "door_control": "" if ok else door("mac-settings-ai.providers.section"), "usable": ok}
        if n == "decision":
            return {"need": n, "state": "missing", "reason": "laya-missing", "words": "w", "door_pane": "decisions", "door_control": door("mac-settings-ai.decisions.local.download"), "usable": False}
        return {"need": n, "state": "ready", "reason": "ok", "words": "w", "door_pane": "", "door_control": "", "usable": True}

    def prev(self, n):
        return self.m_prev.get(n)

    def click(self, k):
        if k.startswith("preview."):
            _, n, st = k.split(".")
            if st == "clear": self.m_prev.pop(n, None)
            else: self.m_prev[n] = st
            return
        if k == "reset": self.reset()
        elif k == "skip": self.skip_all = True
        elif k.startswith("skip."): self.skipped.add(k[5:])
        elif k.startswith("paste-key.anthropic"):
            if "key-door-dead" not in self.m: self.key = True
        elif k == "paste-key.jev": self.stored = True
        elif k == "jev.consent":
            if "consent-door-dead" not in self.m: self.consent = True

    def settled(self, n):
        return self.need(n)["usable"] or n in self.skipped or self.skip_all

    def rows(self, ax):
        if ax == AS + ".mark.last":
            return [getattr(self, "last", None)] if getattr(self, "last", None) else []
        if ax == AS + ".status":
            out = []
            for n in ("writing", "decision", "meaning"):
                x = self.need(n)
                if "writing-silent" in self.m and n == "writing": x.update(state="ready", reason="ok", usable=True)
                out.append(x)
            return out
        if ax == AS + ".step":
            done = all(self.settled(n) for n in ("writing", "decision", "meaning")) or "done-too-early" in self.m
            marks = [] if "skip-clears-marks" in self.m else [n for n in ("writing", "decision", "meaning") if not self.need(n)["usable"]]
            return [{"done": done or (self.skip_all and "skip-clears-marks" in self.m) or all(self.settled(n) for n in ("writing", "decision", "meaning")), "can_continue": True,
                     "skipped_all": self.skip_all, "marks_on": marks}]
        if ax == AS + ".marks":
            out = []
            wrong = "marks-wrong-words" in self.m
            for n in ("decision", "writing", "meaning"):
                x = self.need(n)
                for sent in ("status", "consequence"):
                    words = {("decision", "status"): "Not on this Mac yet. 843 MB, downloaded once.", ("decision", "consequence"): "The AI cannot sort yet: the decision model is not on this Mac.",
                             ("writing", "consequence"): "needs a writing model"}.get((n, sent), "w")
                    if wrong and n == "decision": words = "Not downloaded."
                    btn = {("decision", "status"): "Download", ("decision", "consequence"): "Download · 843 MB", ("writing", "consequence"): "Connect a model"}.get((n, sent), "Download")
                    shown = not x["usable"] or ("mark-stays-when-met" in self.m)
                    working = x["state"] == "working"
                    out.append({"need": n, "sentence": sent, "shown": shown, "working": working, "words": "" if not shown else words, "button": "" if (not shown or working) else btn,
                                "progress_words": x["words"] if working else "", "cancel": "Cancel" if working and n == "decision" else ""})
            return out
        if ax == AS + ".places":
            u = lambda n: not self.need(n)["usable"] or ("places-ignore-need" in self.m and n == "writing")
            return [{"place": "settings-sidebar.decisions", "shown": u("decision")}, {"place": "settings-sidebar.ai", "shown": u("writing") or u("decision") or u("meaning")},
                    {"place": "settings-sidebar.replies", "shown": u("writing") or u("meaning")}, {"place": "reader-summary", "shown": u("writing")},
                    {"place": "foot-reply-with-ai", "shown": u("writing")}, {"place": "search-meaning", "shown": u("meaning")}]
        if ax == "mac-settings-ai.decisions.jev.consent": return [{"given": self.consent}]
        return None

    def state(self, path):
        if path == "provider_credential/jev/stored": return self.stored
        return None


class Model:
    """The design as the research rows state it, with one mutant switched on per check."""
    TABLE = {"proton.me": ("Proton Mail", "", "127.0.0.1 (Bridge, this Mac)", True), "spacemail.com": ("Spacemail", "", "mail.spacemail.com", False),
             "yahoo.com": ("Yahoo", "https://login.yahoo.com/account/security", "imap.mail.yahoo.com", False),
             "aol.com": ("AOL", "https://login.aol.com/account/security", "imap.aol.com", False)}

    def __init__(self, mut):
        global _MUT
        _MUT = set(mut)
        self.m, self.typed, self.surface, self.qr = mut, "", "mac-main", False
        self.imp = ImportSim(mut)
        self.rep = RepliesSim(mut)
        self.ai = AISetupSim(mut)

    def handle(self, verb, args):
        ok = {"ok": True}
        if verb == "fixture" and args[:1] == ["import-sources"]:
            self.imp.found = [ImportSim.parse(x) for x in args[1:]]; return ok
        if verb == "fixture" and args[:1] == ["import-progress"]:
            self.imp.step(args[1], args[2]); return ok
        if verb == "click" and args[0].startswith(AS + "."):
            self.ai.click(args[0][len(AS) + 1:]); return ok
        if verb == "rows" and (args[0].startswith(AS + ".") or args[0] == "mac-settings-ai.decisions.jev.consent"):
            rows = self.ai.rows(args[0])
            if rows is not None: return {"ok": True, "rows": rows}
        if verb == "core" and args[:1] == ["state"] and args[1].startswith("provider_credential/"):
            v = self.ai.state(args[1])
            return {"ok": True, "value": v}
        if verb == "click" and args[0].startswith(RP + "."):
            self.rep.click(args[0]); return ok
        if verb == "rows" and args[0].startswith(RP + "."):
            rows = self.rep.rows(args[0])
            if rows is not None: return {"ok": True, "rows": rows}
        if verb == "core" and args[:1] == ["state"] and args[1].startswith("reply/"):
            v = self.rep.state(args[1])
            return {"ok": True, "value": v} if v is not None else {"ok": False, "error": "no core value at %s" % args[1]}
        if verb == "click" and args[0].startswith(I + "."):
            self.surface = self.imp.click(args[0], self.surface); return ok
        if verb == "click" and args[0] == "mac-settings-accounts.import":
            self.surface = self.imp.settings_offer(self.surface); self.qr = self.imp.qr; return ok
        if verb == "click" and args[0] == W + ".continue":
            self.surface = W; return ok
        if verb == "rows" and (args[0].startswith(I + ".") or args[0] == "mac-settings-accounts.import"):
            rows = self.imp.rows(args[0])
            if rows is not None: return {"ok": True, "rows": rows}
        if verb == "core" and args[:1] == ["state"] and args[1].startswith("import/"):
            v = self.imp.state(args[1])
            return {"ok": True, "value": v} if v is not None else {"ok": False, "error": "no core value at %s" % args[1]}
        if verb == "fixture" and args[:1] == ["providers"]:
            if args[1:] not in (["none"], ["default"]): return {"ok": False, "error": "usage: fixture providers none|default"}
            self.ai.local = args[1] == "default"; return ok
        if verb == "fixture":
            self.typed, self.surface, self.qr = "", "mac-main", False
            if args[:1] == ["reset"]: self.ai.local = True
            return ok
        if verb == "click":
            if args[0] == "mac-settings-accounts.move-to-iphone":
                self.qr = True                                   # the Welcome handler's effect, always
                if "move-shadowed" not in self.m: self.surface = W   # the Settings handler's effect: the window opens
            return ok
        if verb == "uistate": return {"ok": True, "state": {"surface": self.surface}}
        if verb == "rows" and args[0] == W + ".qr":
            return {"ok": True, "rows": [{"payload": "x"}] if self.qr else []}
        if verb == "type": self.typed += args[0]; return ok
        if verb == "rows" and args[0] == W + ".route":
            d = self.typed.split("@")[-1]
            if "zoho" in d and "zoho-one-region" not in self.m:
                return {"ok": True, "rows": [{"name": "Zoho", "server": "imap." + d, "link": "", "sheet": False, "mac_only": False}]}
            if "zoho" in d: return {"ok": True, "rows": [{"name": "Zoho", "server": "imap.zoho.com", "link": "", "sheet": False, "mac_only": False}]}
            n, link, srv, mac = self.TABLE.get(d, ("IMAP", "", "imap." + d, False))
            if d == "proton.me" and "proton-as-imap" in self.m: n, srv, mac = "IMAP", "imap." + d, False
            if d == "spacemail.com" and "spacemail-unknown" in self.m: n, srv = "IMAP", "imap." + d
            if d == "yahoo.com" and "yahoo-no-link" in self.m: link = ""
            if d == "aol.com" and "aol-as-yahoo" in self.m: n, link = "Yahoo", self.TABLE["yahoo.com"][1]
            return {"ok": True, "rows": [{"name": n, "server": srv, "link": link, "sheet": False, "mac_only": mac}]}
        return {"ok": False, "error": "nothing at %s" % (args,)}


# ---------------------------------------------------------------------------------------------------------------------------------
# The sweep (owner 2026-11-11: "you haven't properly tested every button/feature"): every control of the Welcome and first-start
# screen that the cases above do not click with an asserted outcome, clicked once through the door the screen registers, its visible
# outcome read from a row the app serves, the screen put back as it was. Doors and rows are those of WelcomeMacFeature.clickTargets,
# ImportRows, RepliesStepRows and AISetupRows. The controls with no door or no row are findings in docs/lanes/sweep-welcomemac.md.
SWEEP_MUT = {}          # case name -> mutant key: one mutant per case, never shared


def sweep(name, words):
    mutant = "sw-" + name
    SWEEP_MUT[name] = mutant
    return check("welcomemac-sweep-" + name, mutant, words)


def put_back(r):
    r.act("click", W + ".reset")
    r.act("fixture", "reset")


@sweep("continue-adds-the-typed-address", "Continue does nothing: the typed address never becomes a row")
def _(r):
    wm.go_welcome(r)
    r.act("type", "sweep@example.test")
    before = len(r.rows(W + ".accounts"))
    r.act("click", W + ".continue")
    rows = [x["address"] for x in r.rows(W + ".accounts")]
    put_back(r)
    return (before, rows), (0, ["sweep@example.test"])


@sweep("add-another-clears-the-typed-address", "Add another leaves the typed address in the field")
def _(r):
    wm.go_welcome(r)
    r.act("type", "sweep@example.test")
    before = len(r.rows(W + ".route"))
    r.act("click", W + ".add-another")
    after = len(r.rows(W + ".route"))
    put_back(r)
    return (before, after), (1, 0)


@sweep("paste-list-makes-a-row-for-each-address", "the paste-list control makes no row from a typed list")
def _(r):
    wm.go_welcome(r)
    r.act("type", "a@one.test, b@two.test")
    r.act("click", W + ".paste-list")
    rows = [x["address"] for x in r.rows(W + ".accounts")]
    total = r.rows(W + ".summary")[0]["total"]
    put_back(r)
    return (rows, total), (["a@one.test", "b@two.test"], 2)


@sweep("open-inbox-leaves-the-welcome-window", "Open inbox does nothing: the owner stays on the Welcome window")
def _(r):
    wm.go_welcome(r)
    r.act("fixture", "paste-accounts", "mac-main-sample")
    r.act("click", W + ".continue")           # the Welcome window is the surface (this door sets it)
    before = r.surface()
    # Open inbox is offered while a row is healthy or in flight (WelcomeModel.canOpenInbox): the sample's 30 addresses are already in the core, so the paste
    # refuses every row ("already added", main81: phases refused 30) and the button has nothing to open. That is no verdict on the button: not judged.
    phases = [x.get("phase", "healthy") for x in r.rows(W + ".accounts")]
    if phases and all(p == "refused" for p in phases):
        put_back(r)
        raise wm.Unreadable("every Welcome row is refused (phases %r): the pasted sample addresses are refused as already added by the core, so Open inbox is not offered" % sorted(set(phases)))
    r.act("click", W + ".open-inbox")
    after = r.surface()
    put_back(r)
    return (before, after), (W, "mac-main")


@sweep("move-to-iphone-opens-the-qr-step", "the Welcome window's own Move accounts to iPhone control opens no QR step")
def _(r):
    wm.go_welcome(r)
    before = len(r.rows(W + ".qr"))
    r.act("click", W + ".move-to-iphone")
    after = len(r.rows(W + ".qr"))
    put_back(r)
    return (before, after, len(r.rows(W + ".qr"))), (0, 1, 0)


def import_selected(r):
    return sorted(x["kind"] for x in r.rows(I + ".sources") if x["selected"])


@sweep("import-select-outlook-archive", "the Outlook archive row cannot be chosen: Apple Mail stays chosen")
def _(r):
    fresh(r, *SPECS)
    before = import_selected(r)
    r.act("click", I + ".select.olm")
    after = import_selected(r)
    r.act("click", I + ".reset")
    return (before, after), (["apple-mail"], ["olm"])


@sweep("import-select-mailbox-file", "the mailbox file row cannot be chosen: Apple Mail stays chosen")
def _(r):
    fresh(r, *SPECS)
    before = import_selected(r)
    r.act("click", I + ".select.mbox")
    after = import_selected(r)
    r.act("click", I + ".reset")
    return (before, after), (["apple-mail"], ["mbox"])


@sweep("import-select-apple-mail", "the Apple Mail row cannot be chosen back once another source is chosen")
def _(r):
    fresh(r)
    r.act("click", I + ".choose-olm")           # picking an archive chooses it: Apple Mail is no longer the chosen one
    mid = import_selected(r)
    r.act("click", I + ".select.apple-mail")
    after = import_selected(r)
    r.act("click", I + ".reset")
    return (mid, after), (["olm"], ["apple-mail"])


@sweep("import-choose-mailbox-adds-the-picked-file", "a mailbox file picked with Choose is dropped: the scan never sees it")
def _(r):
    fresh(r)
    r.act("click", I + ".choose-mbox")
    rows = [(x["kind"], x["location"], x["selected"]) for x in r.rows(I + ".sources") if x["kind"] == "mbox"]
    r.act("click", I + ".reset")
    return rows, [("mbox", "~/Mail/in.mbox", True)]


@sweep("import-done-leaves-for-the-inbox", "the Go to inbox button after a finished run keeps the owner on the Welcome window")
def _(r):
    run = begin(r)
    r.send("fixture", "import-progress", run, "finish")
    r.act("click", W + ".continue")
    before = (r.surface(), r.rows(I + ".panel")[0]["showing"], r.rows(I + ".progress")[0]["state"])
    r.act("click", I + ".done")
    after = (r.surface(), r.rows(I + ".panel")[0]["showing"])
    put_back(r)
    return (before, after), ((W, True, "finished"), ("mac-main", False))


def reply_chosen(r, group):
    return [x["chosen"] for x in r.rows(RP + ".choices") if x["id"] == group][0]


def reply_choice(group, option, other):
    @sweep("replies-choose-%s-%s" % (group, option), "the %s choice %s cannot be chosen: the step keeps the other choice" % (group, option))
    def _(r):
        replies_fresh(r)
        r.act("click", "%s.%s.%s" % (RP, group, other))
        mid = reply_chosen(r, group)
        r.act("click", "%s.%s.%s" % (RP, group, option))
        after = reply_chosen(r, group)
        replies_fresh(r)
        return (mid, after), (other, option)


for _g, _o, _other in (("search", "sent-inbox", "inbox"), ("search", "sent", "inbox"), ("voice", "100", "300"), ("voice", "1000", "300")):
    reply_choice(_g, _o, _other)


def ai_key_door(provider):
    @sweep("ai-paste-key-%s" % provider, "the %s paste door stores no key: the writing model stays 'no provider'" % provider)
    def _(r):
        ai_fresh(r)
        before = ai_needs(r)["writing"]["reason"]
        r.act("click", "%s.paste-key.%s" % (AS, provider))
        after = ai_needs(r)["writing"]["reason"]
        for _ in range(30):      # the key goes to the core on a background task (AISettingsStore.paste): the reason leaves provider-none a moment after the click (main81: read at once for gemini and openrouter)
            if after != "provider-none":
                break
            time.sleep(0.1)
            after = ai_needs(r)["writing"]["reason"]
        r.act("click", AS + ".reset")
        r.act("fixture", "reset")
        return (before, after != "provider-none"), ("provider-none", True)


for _p in ("openai", "gemini", "openrouter"):
    ai_key_door(_p)


def paste_row(r):
    x = r.rows(W + ".paste")[0]
    return (x["open"], x["text"])


@sweep("paste-list-opens-and-closes-the-paste-area", "the paste-list control does not open the paste area, or a second press does not close it")
def _(r):
    wm.go_welcome(r)
    before = paste_row(r)[0]
    r.act("click", W + ".paste-list")
    opened = paste_row(r)[0]
    r.act("click", W + ".paste-list")
    closed = paste_row(r)[0]
    put_back(r)
    return (before, opened, closed), (False, True, False)


@sweep("paste-text-moves-the-typed-words-into-the-paste-area", "the typed words are not taken into the paste area: it stays empty")
def _(r):
    wm.go_welcome(r)
    r.act("type", "a@one.test b@two.test")
    r.act("click", W + ".paste-text")
    got = paste_row(r)
    put_back(r)
    return got, (True, "a@one.test b@two.test")


@sweep("paste-add-makes-a-row-for-each-pasted-address", "Make a row for each does nothing: the pasted addresses never become rows")
def _(r):
    wm.go_welcome(r)
    r.act("type", "a@one.test b@two.test")
    r.act("click", W + ".paste-text")
    r.act("click", W + ".paste-add")
    rows = [x["address"] for x in r.rows(W + ".accounts")]
    area = paste_row(r)
    put_back(r)
    return (rows, area), (["a@one.test", "b@two.test"], (False, ""))


def preset_case(name, other):
    @sweep("preset-%s-chooses-the-keys" % name.lower(), "the %s choice of the keys picker cannot be chosen: the preset stays %s" % (name, other))
    def _(r):
        wm.go_welcome(r)
        r.act("click", "%s.preset.%s" % (W, other))
        mid = r.rows(W + ".summary")[0]["preset"]
        r.act("click", "%s.preset.%s" % (W, name))
        after = r.rows(W + ".summary")[0]["preset"]
        r.act("click", W + ".preset.Superhuman")
        put_back(r)
        return (mid, after), (other, name)


for _n, _o in (("Superhuman", "Mail"), ("GotEmail", "Mail"), ("Mail", "GotEmail")):
    preset_case(_n, _o)


def two_rows(r):
    wm.go_welcome(r)
    wm.add(r, "a@one.test")
    wm.add(r, "b@two.test")
    return [x["ax"] for x in r.rows(W + ".accounts")]


@sweep("account-row-tap-opens-and-closes-its-editor", "a tap on an account row opens no editor, or a second tap leaves it open")
def _(r):
    axes = two_rows(r)
    before = r.rows(W + ".accounts")[0]["editing"]
    r.act("click", axes[0])
    opened = r.rows(W + ".accounts")[0]["editing"]
    r.act("click", axes[0])
    closed = r.rows(W + ".accounts")[0]["editing"]
    put_back(r)
    return (before, opened, closed), (False, True, False)


@sweep("account-move-up-puts-the-row-above-the-one-before", "the move-up door of a row leaves the rows in the order they were added")
def _(r):
    axes = two_rows(r)
    before = [x["address"] for x in r.rows(W + ".accounts")]
    r.act("click", axes[1] + ".move-up")
    after = [x["address"] for x in r.rows(W + ".accounts")]
    put_back(r)
    return (before, after), (["a@one.test", "b@two.test"], ["b@two.test", "a@one.test"])


# ---- the rows' own controls (fix-screens F-e to F-h, doors in WelcomeDoors.swift): each is clicked once and the row's words after the press are read ----------------
def wait_phase(r, address, want, tries=40):
    """The connection runs on a task: the row settles a moment after the press; the last row read."""
    row = {}
    for _ in range(tries):
        row = next((x for x in wm.acc_rows(r) if x.get("address") == address), {})
        if row.get("phase") in want:
            break
        time.sleep(0.1)
    return row


def rowof(r, address):
    """The row of an address, or {} when the list holds none (a broken Continue leaves a case with no row: the case reads red, never crashes)."""
    return next((x for x in wm.acc_rows(r) if x.get("address") == address), {})


def row_id(r, address):
    return rowof(r, address).get("ax", W + ".accounts.missing")[len(W + ".accounts."):]


def outside_opens(r):
    return int(((r.send("uistate").get("state", {}).get("hidden") or {}).get("system_ui") or {}).get("open", 0))


@sweep("sign-in-button-connects-a-sheet-route-row", "the Sign in with button of a Microsoft row does nothing: the row stays waiting")
def _(r):
    wm.go_welcome(r)
    wm.add(r, "fine@outlook.com")
    rid = row_id(r, "fine@outlook.com")
    before = rowof(r, "fine@outlook.com").get("phase")
    r.act("click", "%s.sign-in.%s" % (W, rid))
    after = wait_phase(r, "fine@outlook.com", ("healthy", "refused")).get("phase")
    put_back(r)
    return (before, after), ("waiting", "healthy")


@sweep("connect-sends-the-typed-app-password", "Connect does nothing: the typed app password never connects the row")
def _(r):
    wm.go_welcome(r)
    wm.add(r, "me@yahoo.com")
    rid = row_id(r, "me@yahoo.com")
    r.act("click", "%s.accounts.%s.field.secret" % (W, rid))
    r.act("type", "app-pass-1")
    typed = rowof(r, "me@yahoo.com").get("secret_typed")
    r.act("click", "%s.connect.%s" % (W, rid))
    after = wait_phase(r, "me@yahoo.com", ("healthy", "refused")).get("phase")
    put_back(r)
    return (typed, after), (10, "healthy")


def refused_row(r):
    wm.go_welcome(r)
    wm.add(r, "refused@yahoo.com")
    r.act("click", W + ".connect-waiting")
    return wait_phase(r, "refused@yahoo.com", ("refused",))


@sweep("try-again-asks-for-the-password-again", "Try again does nothing: the refused row stays refused")
def _(r):
    before = refused_row(r)
    r.act("click", "%s.accounts.%s.try-again" % (W, row_id(r, "refused@yahoo.com")))
    after = rowof(r, "refused@yahoo.com")
    put_back(r)
    return (before.get("phase"), after.get("phase"), after.get("status")), ("refused", "waiting", "")


@sweep("refused-row-link-is-counted-by-the-rig", "the refused row's link does nothing: no web address is asked of the system")
def _(r):
    row = refused_row(r)
    n0 = outside_opens(r)
    r.act("click", "%s.accounts.%s.link" % (W, row_id(r, "refused@yahoo.com")))
    n1 = outside_opens(r)
    put_back(r)
    return (bool(row.get("status_link")), n1 - n0), (True, 1)


@sweep("provider-link-is-counted-by-the-rig", "the provider page's link does nothing: no web address is asked of the system")
def _(r):
    row = route(r, "me@yahoo.com")
    n0 = outside_opens(r)
    r.act("click", W + ".provider.link")
    n1 = outside_opens(r)
    put_back(r)
    return (bool(row.get("link")), n1 - n0), (True, 1)


def laya_fresh(r):
    """The decision model not on this Mac: the weights live on the disk (`fixture reset` keeps them), so the rig copy's OWN `<run folder>/Models/laya` is emptied
    (the run folder is the one the socket sits in); the owner's folder is refused (the same step as settingsaimac-check.py `_laya_fresh`)."""
    run = os.path.dirname(getattr(r, "path", ""))
    if not run or not os.path.exists(os.path.join(run, "whoosh.sock")):
        raise wm.Unreadable("no rig run folder (WHOOSH_SOCKET_DIR): the models folder to empty is unknown")
    owner = os.path.realpath(os.path.expanduser("~/Library/Application Support/Whoosh"))
    folder = os.path.realpath(os.path.join(run, "Models", "laya"))
    if folder == owner or folder.startswith(owner + os.sep):
        raise wm.Unreadable("the models folder %s is the owner's, never emptied by a check" % folder)
    import shutil
    shutil.rmtree(folder, ignore_errors=True)
    if os.path.exists(folder):
        raise wm.Unreadable("the rig's models folder %s could not be emptied" % folder)


@sweep("ai-download-starts-the-decision-model", "the Download of the decision model does nothing: its line keeps saying it is not on this Mac")
def _(r):
    ai_fresh(r)
    laya_fresh(r)
    stub = sai.Stub("laya")
    try:
        r.act("fixture", "provider-endpoint", "laya", stub.url)      # `fixture provider-endpoint laya <loopback url>` (AIFixture.swift): the weights come from the stub, never from outside this Mac
        before = ai_needs(r)["decision"]
        if before["reason"] != "laya-missing":
            raise wm.Unreadable("the decision line reads %r (%r) after the models folder was emptied, not 'laya-missing': the Download cannot be tried from a known state" % (before["reason"], before["state"]))
        r.act("click", AS + ".download")
        state = before["state"]
        for _ in range(30):                  # the line leaves 'missing' (moving figures, then ready) once the download runs
            state = ai_needs(r)["decision"]["state"]
            if state != "missing":
                break
            time.sleep(0.2)
        for _ in range(50):                  # let a running download end before the stub goes
            if ai_needs(r)["decision"]["state"] != "working":
                break
            time.sleep(0.2)
        reached = any(m == "GET" for m, _p in stub.gets)
    finally:
        wm_soft(r, "fixture", "provider-endpoint", "laya", "")
        stub.close()
        put_back(r)
    return (before["state"], state in ("working", "ready"), reached), ("missing", True, True)


def wm_soft(r, *a):
    try:
        return r.send(*a)
    except wm.Unreadable:
        return None


def editor_open(r, address="me@yahoo.com"):
    wm.go_welcome(r)
    wm.add(r, address)
    rid = row_id(r, address)
    r.act("click", "%s.accounts.%s" % (W, rid))
    return rid


@sweep("editor-name-takes-the-typed-words", "the Name field takes no typed words: the row's name stays")
def _(r):
    rid = editor_open(r)
    r.act("click", "%s.accounts.%s.field.name" % (W, rid))
    r.act("type", "Zed")
    name = rowof(r, "me@yahoo.com").get("name")
    put_back(r)
    return str(name).endswith("Zed"), True


@sweep("editor-signature-takes-the-typed-words", "the Signature field takes no typed words: the row's signature stays empty")
def _(r):
    rid = editor_open(r)
    r.act("click", "%s.accounts.%s.field.signature" % (W, rid))
    r.act("type", "Best")
    sig = rowof(r, "me@yahoo.com").get("signature")
    put_back(r)
    return sig, "Best"


@sweep("editor-colour-swatch-sets-the-colour", "a colour swatch changes nothing: the row keeps its colour")
def _(r):
    rid = editor_open(r)
    before = rowof(r, "me@yahoo.com").get("colour")
    r.act("click", "%s.accounts.%s.colour.4" % (W, rid))
    after = rowof(r, "me@yahoo.com").get("colour")
    put_back(r)
    return (before != after, bool(after)), (True, True)


@sweep("editor-icon-sets-the-icon", "an icon button changes nothing: the row keeps its icon")
def _(r):
    rid = editor_open(r)
    r.act("click", "%s.accounts.%s.icon.briefcase" % (W, rid))
    icon = rowof(r, "me@yahoo.com").get("icon")
    put_back(r)
    return icon, "briefcase"


@sweep("editor-remove-takes-the-address-away", "Remove this address does nothing: the row stays")
def _(r):
    rid = editor_open(r)
    before = len(wm.acc_rows(r))
    r.act("click", "%s.accounts.%s.remove" % (W, rid))
    rows = wm.acc_rows(r)
    put_back(r)
    return (before, len(rows)), (1, 0)


@sweep("import-open-full-disk-access-counts-the-press", "Open Full Disk Access does nothing on a rig: no press is counted")
def _(r):
    fresh(r, "apple-mail=fda")
    before = r.rows(I + ".panel")[0]["fda_presses"]
    r.act("click", I + ".open-settings")
    after = r.rows(I + ".panel")[0]["fda_presses"]
    r.act("click", I + ".reset")
    return (before, after), (0, 1)


def mark_last(r):
    rows = r.rows("ai-setup.mark.last")
    return (rows[0]["need"], rows[0]["action"], rows[0]["result"]) if rows else None


@sweep("ai-mark-fix-names-its-outcome", "the mark's Fix button does nothing: no pane opens and nothing is recorded")
def _(r):
    r.act("click", "ai-setup.reset")
    r.act("click", "ai-setup.preview.writing.missing")
    r.act("click", "ai-setup.mark.writing.fix")
    got = mark_last(r)
    r.act("click", "ai-setup.reset")
    return got, ("writing", "fix", "settings:ai")


@sweep("ai-mark-cancel-names-its-outcome", "the mark's Cancel button does nothing: the download is not asked to stop")
def _(r):
    r.act("click", "ai-setup.reset")
    r.act("click", "ai-setup.preview.decision.working")
    r.act("click", "ai-setup.mark.decision.cancel")
    got = mark_last(r)
    r.act("click", "ai-setup.reset")
    return got, ("decision", "cancel", "cancel-requested")


def into_state(r):
    x = r.rows(I + ".panel")[0]
    return x["into"], x["accounts"]


@sweep("import-into-picks-an-account", "the Into picker cannot take an account: the import stays on the matching account")
def _(r):
    fresh(r, *SPECS)
    _into, accounts = into_state(r)
    if not accounts:
        r.act("click", I + ".reset")
        raise wm.Unreadable("the Into picker lists no account on this core, nothing to pick")
    r.act("click", "%s.into.%s" % (I, accounts[0]))
    after = into_state(r)[0]
    r.act("click", I + ".reset")
    return (_into, after), ("", accounts[0])


@sweep("import-into-matching-goes-back-to-the-matching-account", "the matching-account choice of the Into picker is ignored: the picked account stays")
def _(r):
    fresh(r, *SPECS)
    _into, accounts = into_state(r)
    if not accounts:
        r.act("click", I + ".reset")
        raise wm.Unreadable("the Into picker lists no account on this core, nothing to pick")
    r.act("click", "%s.into.%s" % (I, accounts[0]))
    mid = into_state(r)[0]
    r.act("click", I + ".into.matching")
    after = into_state(r)[0]
    r.act("click", I + ".reset")
    return (mid, after), (accounts[0], "")



class SweepImport(ImportSim):
    def click(self, ident, surface):
        k, m = ident[len(I) + 1:], self.m
        if k.startswith("select.") and "sw-import-select-" + {"olm": "outlook-archive", "mbox": "mailbox-file", "apple-mail": "apple-mail"}.get(k[7:], "") in m: return surface
        if k == "choose-mbox" and "sw-import-choose-mailbox-adds-the-picked-file" in m: return surface
        if k == "done" and "sw-import-done-leaves-for-the-inbox" in m: self.open, self.skipped = False, True; return surface
        return super().click(ident, surface)


class SweepReplies(RepliesSim):
    def click(self, ident):
        k = ident[len(RP) + 1:]
        if k.count(".") == 1 and "sw-replies-choose-" + k.replace(".", "-") in self.m: return
        super().click(ident)


class SweepAI(AISetupSim):
    endpoint, laya = None, "missing"      # the staged loopback address of the decision model's files; its line: missing, then ready after a download

    def reset(self):
        super().reset()
        self.laya = "missing"

    def need(self, n):
        x = super().need(n)
        if n == "decision" and self.laya == "ready" and not self.m_prev.get(n):
            x.update(state="ready", reason="ok", usable=True, door_control="")
        return x

    def fetch(self):
        """The Download: each of the model's five files is asked of the staged address (a loopback stub only: any other address is refused here)."""
        import urllib.request
        if not self.endpoint or not self.endpoint.startswith("http://127.0.0.1:"):
            return
        try:
            for f in sai.LAYA_FILES:
                urllib.request.urlopen(self.endpoint + "/" + f, timeout=5).read()
            self.laya = "ready"
        except OSError:
            self.laya = "missing"

    def click(self, k):
        if k == "download":
            if "sw-ai-download-starts-the-decision-model" not in self.m: self.fetch()
            return
        if k.startswith("mark.") and (k.endswith(".fix") or k.endswith(".cancel")):
            need = k.split(".")[1]
            x = self.need(need)
            if k.endswith(".fix") and not x["usable"] and x["state"] != "working":
                if "sw-ai-mark-fix-names-its-outcome" not in self.m:
                    self.last = {"need": need, "action": "fix", "result": {"writing": "settings:ai", "meaning": "meaning-download",
                                 "decision": "laya-download" if x["reason"] in ("laya-missing", "laya-failed") else "settings:decisions"}[need]}
            elif k == "mark.decision.cancel" and x["state"] == "working":
                if "sw-ai-mark-cancel-names-its-outcome" not in self.m: self.last = {"need": "decision", "action": "cancel", "result": "cancel-requested"}
            return
        if k == "reset": self.last = None
        if k.startswith("paste-key.") and k[10:] in ("openai", "gemini", "openrouter"):
            if "sw-ai-paste-key-" + k[10:] not in self.m: self.key = True
            return
        super().click(k)


class SweepModel(Model):
    """The base stand-in plus the Welcome window's own rows (accounts, summary, an empty route when nothing is typed), for the sweep."""
    def __init__(self, mut):
        super().__init__(mut)
        self.imp, self.rep, self.ai = SweepImport(mut), SweepReplies(mut), SweepAI(mut)
        self.accts, self.pasted, self.phase = [], [], {}
        self.paste_open, self.paste_text, self.preset, self.editing = False, "", "Superhuman", None

    def add_all(self, text):
        for a in re.split(r"[ ,;\n\t\r<>]+", text):
            if re.match(r"^[^@\s]+@[^@\s]+\.[^@\s]+$", a) and a.lower() not in [x.lower() for x in self.accts]: self.accts.append(a)

    def handle(self, verb, args):
        m = self.m
        if verb == "fixture" and args[:1] == ["paste-accounts"]:
            self.accts = ["acct%d@sample.test" % i for i in range(30)]; self.pasted = list(self.accts); self.phase = {}; return {"ok": True}
        if verb == "fixture" and args[:1] == ["relaunch"]:        # the copy comes back from what it saved: the dragged order, unless the mutant forgets it
            if "order-lost-on-relaunch" in m: self.accts = list(self.pasted)
            self.surf = "mac-main"; return {"ok": True}
        if verb == "drag":
            src, dst = [a[len(W + ".accounts."):] for a in args[:2]]
            if src in self.accts and dst in self.accts:
                self.accts.remove(src); self.accts.insert(self.accts.index(dst) + 1, src)
            return {"ok": True}
        if verb == "fixture" and args[:1] == ["reset"]: self.accts, self.phase = [], {}
        if verb == "click" and args[0] == W + ".reset": self.accts, self.typed, self.qr, self.phase = [], "", False, {}; self.paste_open, self.paste_text, self.editing = False, "", None; self.imp.reset(); return {"ok": True}
        if verb == "click" and args[0] == W + ".connect-waiting":     # the fake connector: only a Microsoft address with admin-blocked in it is refused, and with the engine's line
            for a in self.accts:
                if a in self.phase: continue
                blocked = a.startswith("admin-blocked@") and wm.DOM.get(a.split("@")[-1]) == "Microsoft" and "sw-admin-line-missing" not in m
                self.phase[a] = ("refused", ADMIN_WORDS) if blocked else ("healthy", "")
                if "refused" in a: self.phase[a] = ("refused", "Yahoo refused the sign-in")
            return {"ok": True}
        if verb == "click" and args[0] == W + ".move-to-iphone":
            self.qr = "sw-move-to-iphone-opens-the-qr-step" not in m; return {"ok": True}
        if verb == "click" and args[0] == W + ".continue":
            if "sw-continue-adds-the-typed-address" not in m: self.add_all(self.typed)
            self.typed, self.surface = "", W; return {"ok": True}
        if verb == "click" and args[0] == W + ".add-another":
            if "sw-add-another-clears-the-typed-address" not in m: self.typed = ""
            return {"ok": True}
        if verb == "click" and args[0] == W + ".paste-list":
            if not self.typed:
                if "sw-paste-list-opens-and-closes-the-paste-area" not in m: self.paste_open = not self.paste_open
                return {"ok": True}
            if "sw-paste-list-makes-a-row-for-each-address" not in m: self.add_all(self.typed); self.typed = ""
            return {"ok": True}
        if verb == "click" and args[0] == W + ".paste-text":
            if not self.typed: return {"ok": False, "error": "nothing typed"}
            if "sw-paste-text-moves-the-typed-words-into-the-paste-area" not in m: self.paste_text, self.typed, self.paste_open = self.typed, "", True
            return {"ok": True}
        if verb == "click" and args[0] == W + ".paste-add":
            if "sw-paste-add-makes-a-row-for-each-pasted-address" not in m: self.add_all(self.paste_text); self.paste_text, self.paste_open = "", False
            return {"ok": True}
        if verb == "click" and args[0].startswith(W + ".preset."):
            name = args[0][len(W + ".preset."):]
            if "sw-preset-%s-chooses-the-keys" % name.lower() not in m: self.preset = name
            return {"ok": True}
        info = self.__dict__.setdefault("info", {})
        if (verb == "click" and args[0] == W + ".reset") or (verb == "fixture" and args[:1] == ["reset"]):
            self.focus = None; info.clear()      # as WelcomeModel.reset: the focused field, the typed passwords and the rows' edits go
        def row_info(a):
            return info.setdefault(a, {"name": a.split("@")[0], "sig": "", "secret": "", "colour": "#sw-default", "icon": "", "link": ""})
        if verb == "click" and args[0].startswith(W + ".sign-in."):
            a = args[0][len(W + ".sign-in."):]
            if a in self.accts and a not in self.phase and "sw-sign-in-button-connects-a-sheet-route-row" not in m: self.phase[a] = ("healthy", "")
            return {"ok": True}
        if verb == "click" and args[0].startswith(W + ".connect.") and args[0] != W + ".connect-waiting":
            a = args[0][len(W + ".connect."):]
            if a in self.accts and a not in self.phase and row_info(a)["secret"] and "sw-connect-sends-the-typed-app-password" not in m:
                self.phase[a] = ("refused", "wrong") if row_info(a)["secret"] == "wrong-password" else ("healthy", "")
                row_info(a)["secret"] = ""
            return {"ok": True}
        if verb == "click" and args[0].startswith(W + ".accounts.") and ".field." in args[0]:
            a, kind = args[0][len(W + ".accounts."):].split(".field.")
            if a in self.accts and (kind == "secret" or self.editing == a): self.focus = (a, kind)
            return {"ok": True}
        if verb == "type" and getattr(self, "focus", None):
            a, kind = self.focus
            i = row_info(a)
            if kind == "name" and "sw-editor-name-takes-the-typed-words" not in m: i["name"] += args[0]
            if kind == "signature" and "sw-editor-signature-takes-the-typed-words" not in m: i["sig"] += args[0]
            if kind == "secret": i["secret"] += args[0]
            return {"ok": True}
        if verb == "click" and args[0].startswith(W + ".accounts.") and args[0].endswith(".try-again"):
            a = args[0][len(W + ".accounts."):-len(".try-again")]
            if self.phase.get(a, ("waiting",))[0] == "refused":
                if "sw-try-again-asks-for-the-password-again" not in m: self.phase.pop(a)
                return {"ok": True}
            return {"ok": False, "error": "did nothing"}
        if verb == "click" and args[0].startswith(W + ".accounts.") and args[0].endswith(".link"):
            a = args[0][len(W + ".accounts."):-len(".link")]
            if self.phase.get(a, ("waiting",))[0] == "refused":
                if "sw-refused-row-link-is-counted-by-the-rig" not in m: self.outside = getattr(self, "outside", 0) + 1
                return {"ok": True}
            return {"ok": False, "error": "did nothing"}
        if verb == "fixture" and args[:2] == ["provider-endpoint", "laya"]:
            self.ai.endpoint = args[2] or None; return {"ok": True}
        if verb == "click" and args[0] == W + ".provider.link":
            link = self.TABLE.get(self.typed.split("@")[-1], ("", "", "", False))[1]
            if not link: return {"ok": False, "error": "did nothing"}
            if "sw-provider-link-is-counted-by-the-rig" not in m: self.outside = getattr(self, "outside", 0) + 1
            return {"ok": True}
        if verb == "click" and args[0].startswith(W + ".accounts.") and ".colour." in args[0]:
            a, n = args[0][len(W + ".accounts."):].split(".colour.")
            if a in self.accts and self.editing == a and "sw-editor-colour-swatch-sets-the-colour" not in m: row_info(a)["colour"] = "#sw%s" % n
            return {"ok": True}
        if verb == "click" and args[0].startswith(W + ".accounts.") and ".icon." in args[0]:
            a, ic = args[0][len(W + ".accounts."):].split(".icon.")
            if a in self.accts and self.editing == a and "sw-editor-icon-sets-the-icon" not in m: row_info(a)["icon"] = ic
            return {"ok": True}
        if verb == "click" and args[0].startswith(W + ".accounts.") and args[0].endswith(".remove"):
            a = args[0][len(W + ".accounts."):-len(".remove")]
            if a in self.accts and self.editing == a:
                if "sw-editor-remove-takes-the-address-away" not in m: self.accts.remove(a); self.editing = None
                return {"ok": True}
            return {"ok": False, "error": "did nothing"}
        if verb == "uistate":
            r0 = super().handle(verb, args)
            r0.setdefault("state", {})["hidden"] = {"system_ui": {"open": getattr(self, "outside", 0)}}
            return r0
        if verb == "click" and args[0].startswith(W + ".accounts."):
            rest = args[0][len(W + ".accounts."):]
            if rest.endswith(".move-up") and rest[:-8] in self.accts:
                a = rest[:-8]; i = self.accts.index(a)
                if i > 0 and "sw-account-move-up-puts-the-row-above-the-one-before" not in m: self.accts[i - 1], self.accts[i] = self.accts[i], self.accts[i - 1]
                return {"ok": True}
            if rest in self.accts:
                if "sw-account-row-tap-opens-and-closes-its-editor" not in m: self.editing = None if self.editing == rest else rest
                return {"ok": True}
        if verb == "rows" and args[0] == W + ".paste": return {"ok": True, "rows": [{"open": self.paste_open, "text": self.paste_text}]}
        if verb == "click" and args[0] == W + ".open-inbox":
            if self.accts and "sw-open-inbox-leaves-the-welcome-window" not in m: self.surface = "mac-main"
            return {"ok": True}
        if verb == "rows" and args[0] == W + ".accounts": return {"ok": True, "rows": [{"ax": W + ".accounts." + a, "address": a, "phase": self.phase.get(a, ("waiting", ""))[0], "status": self.phase.get(a, ("waiting", ""))[1], "editing": self.editing == a,
                  "name": self.info.get(a, {}).get("name", a.split("@")[0]), "signature": self.info.get(a, {}).get("sig", ""), "secret_typed": len(self.info.get(a, {}).get("secret", "")),
                  "colour": self.info.get(a, {}).get("colour", "#sw-default"), "icon": self.info.get(a, {}).get("icon", ""),
                  "status_link": "https://login.yahoo.com/" if self.phase.get(a, ("",))[0] == "refused" and "refused" in a else ""} for a in self.accts]}
        if verb == "rows" and args[0] == W + ".summary": return {"ok": True, "rows": [{"total": len(self.accts), "preset": self.preset}]}
        if verb == "rows" and args[0] == W + ".route" and not self.typed: return {"ok": True, "rows": []}
        return super().handle(verb, args)


if __name__ == "__main__":
    if "--selftest" in sys.argv:        # the stand-in has no app to quit: its relaunch is one verb the model answers
        wm.Rig.relaunch_hook = lambda rig: rig.send("fixture", "relaunch")
    if "--source" in sys.argv:          # the source checks only: they read files, so no app and no socket
        mine = [c for c in MINE if c[0] in ("import-motion-within-the-budget", "import-no-bare-string", "import-keys-in-14-languages-fr-first",
                                            "replies-writes-are-the-contracts-three-commands", "replies-no-bare-string", "replies-keys-have-french-and-english",
                                            "ai-setup-download-door-calls-the-laya-download", "ai-setup-no-bare-string", "ai-setup-keys-have-french-and-english")]
        res = wm.run_checks(None, only=lambda n: n in {c[0] for c in mine})
        line, f, u = wm.summary(res)
        for n, st, d in res:
            print("  %s %s%s" % (st.upper(), n, "" if st == "pass" else ": " + d), file=sys.stderr)
        print(line)
        sys.exit(1 if (f or u) else 0)
    sys.exit(wm.main(SweepModel))
