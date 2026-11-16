#!/usr/bin/env python3
"""imported-name check: the catch-all account for imported mail ("imported@whoosh.local", stored name "Imported") is drawn
with the catalogue's "Imported" wherever an account is named (the owner's rule: "don't mix languages").

Static (default and --dry): 1) Sources/App/AccountNames.swift holds the one rule and the address constant; 2) every render site
below calls AccountNames; 3) every other Swift file that reads the core's account list is on the id/address-only list, so a new
reader that draws a name fails here until it is placed; 4) the catalogue holds "Imported" in 14 languages.
  --dry        the static check, nothing else (no app, no socket)
  --selftest   the static check on the tree (green), then once per site with that one call reverted in memory (each must go red)
  --live       driven (sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/imported-name-check.py --live'):
               case imported-account-row-is-in-the-app-language
Prints 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0."""
import json, os, re, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CAT = os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings")
LANGS = ["da", "de", "en", "es", "fi", "fr", "it", "ja", "nb", "nl", "pl", "pt", "sv", "zh-Hans"]
FR = "Importés"

# render site -> the call that must stand there (the place the core's account name enters the app)
SITES = {
    "Sources/App/AccountBook.swift": "name: AccountNames.display(a)",
    "Sources/Features/Sidebar/SidebarData.swift": "displayName: AccountNames.display(a)",
    "Sources/Features/SettingsAccountsMac/AccountsSession.swift": "name: AccountNames.display(a)",
    "Sources/Features/QuickReply/QuickReplyModel.swift": "m.core.accounts().map(AccountNames.localised)",
    "Sources/Features/QuickReply/QuickReplyThreadCards.swift": "app.core.accounts().map(AccountNames.localised)",
    "Sources/Features/AddAccount/AddAccountFlow.swift": "AccountNames.display($0)",
    "Sources/Features/MacShell/MacSyncStatus.swift": "AccountNames.display(a)",
    "Sources/Features/WelcomeMac/WelcomeMacFeature.swift": "AccountNames.display(address: r.address, name: r.name)",
    "Sources/Features/FirstStart/FirstStartArt.swift": "accounts().last.map { AccountNames.display($0) }",
    "Sources/Features/WelcomePhone/WelcomePhoneFlow.swift": "name: AccountNames.display(a)",
    # the row the Welcome screen DRAWS (WelcomeRowView draws row.name): the socket row of WelcomeMacFeature above passes it through the rule a second time
    "Sources/Features/WelcomeMac/WelcomeModel.swift": "name: AccountNames.display(a)",
}
# readers of the core's account list or the AccountBook that draw no name, or draw the one AccountBook gives (already localised)
NO_NAME = {
    "Sources/App/AccountNames.swift": "the rule itself",
    "Sources/App/BaseFeature.swift": "AccountBook entries (localised at AccountBook)",
    "Sources/Features/MacShell/MacShellCommands.swift": "id only (control-N switch)",
    "Sources/Features/Shortcuts/Commands.swift": "id only (control-N switch)",
    "Sources/Features/SettingsGeneralMac/NotificationsPaneView.swift": "AccountBook entries",
    "Sources/Features/SettingsGeneralMac/SettingsGeneralMacFeature.swift": "AccountBook entries",
    "Sources/Features/SettingsWindow/SettingsWindowRegistry.swift": "AccountBook entries (ids)",
    "Sources/Features/Notch/NotchFeature.swift": "ids only; names come from RowAccount (AccountBook)",
    "Sources/Features/Sidebar/SidebarStructure.swift": "SidebarData accounts (localised at SidebarData)",
    "Sources/Features/Sidebar/SidebarState.swift": "SidebarData accounts (localised at SidebarData)",
    "Sources/Features/Sidebar/SidebarView.swift": "SidebarData accounts (localised at SidebarData)",
    "Sources/Features/Sidebar/SidebarPicker.swift": "SidebarData accounts (localised at SidebarData)",
    "Sources/Features/Sidebar/SidebarLabelsSection.swift": "SidebarData accounts (localised at SidebarData)",
    "Sources/Features/Sidebar/SidebarFeature.swift": "SidebarData accounts (localised at SidebarData)",
    "Sources/Features/LabelsContactsMac/LabelInspectorBlocks.swift": "AccountBook entries",
    "Sources/Features/LabelsContactsMac/LabelInspectorRules.swift": "AccountBook entries",
    "Sources/Features/LabelsContactsMac/LabelProofViews.swift": "a count",
    "Sources/Features/LabelsContactsMac/LabelProofRow.swift": "a count",
    "Sources/Features/LabelsContactsMac/LabelInspectorSocket.swift": "ids only",
    "Sources/Features/LabelsContactsMac/LabelFlowSocket.swift": "ids only",
    "Sources/Features/LabelsContactsMac/LabelInspectorModel.swift": "AccountBook entries",
    "Sources/Features/LabelsContactsMac/LabelsContactsSource.swift": "AccountBook entries",
    "Sources/Features/WelcomeMac/ImportModel.swift": "another lane's file (Import*.swift): its Into list, reported in the note",
    "Sources/Features/SearchMac/SearchListPane.swift": "a count",
    "Sources/Features/SearchMac/SearchMacRows.swift": "AccountBook entries",
    "Sources/Features/SearchMac/SearchIndexing.swift": "AccountBook entries",
    "Sources/Features/AddAccount/AddAccountFeature.swift": "AddAccountRows over model.added (localised at AddAccountFlow)",
    "Sources/Features/QuickReply/ReplyContext.swift": "address only",
    "Sources/Features/PaletteMac/PaletteMacRows.swift": "AccountBook entries",
    "Sources/Features/PaletteMac/PaletteMacItems.swift": "AccountBook entries",
    "Sources/Features/PaletteMac/PaletteMacModel.swift": "AccountBook entries",
    "Sources/Features/PaletteMac/PaletteMacCatalog.swift": "AccountBook entries",
    "Sources/Features/SettingsAccountsMac/AccountRowView.swift": "AccountsSession entries (localised there)",
    "Sources/Features/ComposerMac/ComposerContext.swift": "AccountBook entries",
    "Sources/Features/ComposerMac/ComposerModel.swift": "ComposerContext (AccountBook entries)",
    "Sources/Features/InboxList/RowFacts.swift": "AccountBook entries",
    "Sources/Features/Notch/NotchRows.swift": "id and AccountBook mark only (NotchRows.swift:23-25); the wings name comes from RowAccount",
    "Sources/Features/FirstStart/FirstStartModel.swift": "a count (FirstStartModel.swift:86)",
    "Sources/Features/SettingsRepliesMac/SourceTiles.swift": "the address (SourceTiles.swift:12), never the stored name",
    "Sources/Features/Reader/ReaderChrome.swift": "addresses only (the owner's own addresses, ReaderChrome.swift:19)",
    "Sources/Features/WelcomePhone/WelcomePhoneFeature.swift": "a count (WelcomePhoneFeature.swift:92)",
    "Sources/Features/LabelsContactsPhone/LabelPhoneRead.swift": "AccountBook entries (LabelPhoneRead.swift:51)",
    "Sources/Features/QuickReply/ThreadCardsRegistry.swift": "address only (ThreadCardsRegistry.swift:43)",
    "Sources/Features/SettingsAccountsMac/SettingsAccountsMacFeature.swift": "a count (SettingsAccountsMacFeature.swift:40)",
}
READ = re.compile(r"core\.accounts\(\)|\.accounts\(core:|AccountBook")


def swift_files():
    for base, _, files in os.walk(os.path.join(ROOT, "Sources")):
        if os.sep + "CoreBridge" in base:
            continue
        for f in files:
            if f.endswith(".swift"):
                yield os.path.relpath(os.path.join(base, f), ROOT)


def read(rel):
    with open(os.path.join(ROOT, rel), encoding="utf-8") as h:
        return h.read()


def static(rd=read):
    res = []
    rule = rd("Sources/App/AccountNames.swift")
    res.append(("rule-file-holds-address-and-catalogue-word",
                'importedAddress = "imported@whoosh.local"' in rule and 'Words.text("Imported")' in rule and "import_run.rs" in rule, "AccountNames.swift"))
    for rel, call in SITES.items():
        res.append(("site:" + rel, call in rd(rel), call))
    strays = [rel for rel in swift_files() if rel not in SITES and rel not in NO_NAME and READ.search(rd(rel))]
    res.append(("every-account-reader-is-placed", not strays, "unplaced readers: %s" % (strays or "none")))
    try:
        cat = json.load(open(CAT, encoding="utf-8"))["strings"]["Imported"]["localizations"]
        got = sorted(k for k, v in cat.items() if v["stringUnit"]["value"].strip())
        res.append(("catalogue-imported-in-14-languages", got == LANGS and cat["fr"]["stringUnit"]["value"] == FR, "%d languages" % len(got)))
    except (OSError, ValueError, KeyError) as e:
        res.append(("catalogue-imported-in-14-languages", False, repr(e)))
    return res


def report(res):
    bad = [n for n, ok, _ in res if not ok]
    for n, ok, why in res:
        print("%s  %s  %s" % ("ok  " if ok else "RED ", n, "" if ok else why))
    print("%d checks, %d failed, 0 not judged" % (len(res), len(bad)))
    return bad


def selftest():
    if report(static()):
        print("selftest: the tree is not green"); return 1
    missed = []
    for rel, call in SITES.items():
        def rd(p, rel=rel, call=call):
            s = read(p)
            return s.replace(call, "REVERTED") if p == rel else s
        red = [n for n, ok, _ in static(rd) if not ok]
        print("mutant %-62s %s" % (rel, "red on " + ",".join(red) if red else "STAYED GREEN"))
        if ("site:" + rel) not in red:
            missed.append(rel)
    print("selftest: %s" % ("GREEN, every mutant red" if not missed else "RED, not caught: %s" % missed))
    return 1 if missed else 0


def live():
    """Driven: fixture import-sources apple-mail=3, start, finish; the sidebar row of the Imported account reads the French catalogue value
    under whoosh.language=fr (the rig is launched with -whoosh.language fr, as i18n-check.py --picked does)."""
    import importlib.util
    spec = importlib.util.spec_from_file_location("wm", os.path.join(ROOT, "checks", "welcomemac-check.py"))
    wm = importlib.util.module_from_spec(spec)
    argv, sys.argv = sys.argv, [sys.argv[0]]
    spec.loader.exec_module(wm)
    sys.argv = argv
    try:
        r = wm.Rig(os.environ.get("WHOOSH_SOCKET_DIR", "")).connect()
        I = "mac-welcome-import"
        r.act("fixture", "reset")
        r.send("fixture", "import-sources", "apple-mail=3")
        r.act("click", I + ".scan"); r.act("click", I + ".select.apple-mail"); r.act("click", I + ".start")
        run = r.rows(I + ".progress")[0]["run_id"]
        r.send("fixture", "import-progress", run, "finish")
        rows = [x for x in r.rows("mac-main.sidebar.accounts") if x.get("address") == "imported@whoosh.local"]
    except wm.Unreadable as e:
        print("imported-account-row-is-in-the-app-language: NOT JUDGED (%s)" % e); print("1 checks, 0 failed, 1 not judged"); return 1
    if not rows:
        print("imported-account-row-is-in-the-app-language: NOT JUDGED (the fixture core seeds no imported account: FixtureImport.swift credits 'imported' "
              "in the result but adds no row to accounts(); that file is CoreBridge, outside this lane)")
        print("1 checks, 0 failed, 1 not judged"); return 1
    ok = rows[0].get("name") == FR
    print("imported-account-row-is-in-the-app-language: %s (row name %r, expected %r)" % ("ok" if ok else "RED", rows[0].get("name"), FR))
    print("1 checks, %d failed, 0 not judged" % (0 if ok else 1)); return 0 if ok else 1


if __name__ == "__main__":
    a = sys.argv[1:]
    if "--selftest" in a: sys.exit(selftest())
    if "--live" in a: sys.exit(live())
    sys.exit(1 if report(static()) else 0)
