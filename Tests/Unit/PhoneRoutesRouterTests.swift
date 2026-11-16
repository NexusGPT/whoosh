import Foundation
import Testing
@testable import WhooshUI

// Lane gap-phone-wire: the router (Sources/App/PhoneRouter.swift) on the fixture core, each screen reached the way the socket reaches it and
// left the way its own Back leaves it. There is no view here: the root's `.onChange(of: overlay)` is one call, `PhoneRouter.overlayChanged()`,
// which each test makes where a screen's own door set the overlay. Each test names its broken copy in a trailing comment.
// Run with WHOOSH_FIXTURE_CORE=1 (the shared AppModel serves the design fixture).

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
@MainActor private func model() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    setenv("WHOOSH_FIXTURE_PATH", repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()   // launch state once per test (Tests/Support/SharedAppFresh.swift)
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    PhoneRouter.shared.send(.reset)
    if SearchPhoneModel.shared.active { _ = SearchPhoneModel.shared.close() }
    InboxPhoneState.shared.searching = false
    return m
}
@MainActor private func click(_ ax: String, _ m: AppModel) throws -> Bool {
    let tap = try #require(Registrations.click(ax), "no click is registered at \(ax)")
    return tap(m)
}
@MainActor private func rows(_ ax: String, _ m: AppModel) throws -> [ControlRow] {
    let src = try #require(Registrations.rowSource(ax), "no rows are registered at \(ax)")
    return try src(m)
}

// MARK: the inbox's own controls

@Test @MainActor func aTapOnAnInboxRowPushesTheThreadTheListKeepsItsPlaceAndBackReturnsToIt() throws {
    let m = try model()
    defer { ThreadPhoneState.shared.reset(); m.overlay = nil }
    let rowsNow = InboxPhoneState.shared.rows(m)
    let i = min(2, rowsNow.count - 1)
    try #require(i >= 0, "the fixture inbox has rows")
    #expect(PhoneRouter.shared.openThread(row: i))                                      // mutant: openThread(row:) skips `send(.overlayChanged(...))`
    #expect(PhoneRouter.shared.nav.path == [.inbox, .thread(rowsNow[i].messageId)])
    #expect(m.overlay == "iphone-thread")
    #expect(m.surface == "iphone-thread")
    let opened = try #require(try rows("iphone-thread.open", m).first)
    #expect(opened["message"] as? String == rowsNow[i].messageId, "the thread screen reads the mail the row held")
    // Back by the screen's own control: it clears the overlay, the root reports it, the push is popped.
    #expect(try click("iphone-thread.back", m))
    PhoneRouter.shared.overlayChanged()
    #expect(PhoneRouter.shared.nav.path == [.inbox])                                    // mutant: reducer pops nothing on a nil overlay
    #expect(m.overlay == nil)
    #expect(m.surface == "iphone-inbox")
    #expect(m.focusIndex == i, "the row the list was on is still the focused row")
}

@Test @MainActor func theBarsComposeControlOpensTheComposerAsASheetAndCancelClosesIt() throws {
    let m = try model()
    #expect(try click("iphone-inbox.compose", m))                                       // mutant: the click registers `compose` on the wrong route (`openSettings`)
    #expect(PhoneRouter.shared.nav.sheet == .composer)
    #expect(PhoneRouter.shared.nav.path == [.inbox], "the inbox stays under the sheet")
    #expect(m.overlay == "iphone-composer")
    let open = try #require(try rows("iphone-composer.header", m).first)
    #expect(open["is_open"] as? Bool == true)
    #expect(try click("iphone-composer.cancel", m))
    PhoneRouter.shared.overlayChanged()
    #expect(PhoneRouter.shared.nav.sheet == nil)
    #expect(PhoneRouter.shared.nav.path == [.inbox])
    #expect(m.overlay == nil)
}

@Test @MainActor func theComposeControlAnswersOnlyWhereItIsDrawn() throws {
    let m = try model()
    PhoneRouter.shared.send(.showMailboxes)
    #expect(try click("iphone-inbox.compose", m) == false, "Mailboxes draws no Compose")           // mutant: the guard `PhoneRouter.shared.onInbox` removed
    #expect(PhoneRouter.shared.nav.sheet == nil)
}

@Test @MainActor func thePalettesOwnDoorOpensItsSheetAndItsCloseClosesIt() throws {
    let m = try model()
    #expect(try click("iphone-palette.open", m))
    PhoneRouter.shared.overlayChanged()
    #expect(PhoneRouter.shared.nav.sheet == .palette)                                  // mutant: the reducer's palette case opens `.composer`
    #expect(m.overlay == "iphone-palette")
    #expect(try click("iphone-palette.close", m))
    PhoneRouter.shared.overlayChanged()
    #expect(PhoneRouter.shared.nav.sheet == nil)
    #expect(PhoneRouter.shared.nav.path == [.inbox])
}

@Test @MainActor func theSearchPillOpensTheSearchInPlaceAndItsCloseReturnsToTheInbox() throws {
    let m = try model()
    defer { if SearchPhoneModel.shared.active { _ = SearchPhoneModel.shared.close() } }
    #expect(try click("iphone-inbox.search", m))                                       // mutant: `openSearch()` skips `SearchPhoneModel.shared.open()`
    #expect(SearchPhoneModel.shared.active)
    #expect(PhoneRouter.shared.nav.searching)
    #expect(PhoneRouter.shared.nav.path == [.inbox], "in place: nothing is pushed")
    #expect(m.surface == "iphone-search")
    #expect(try click("iphone-search.close", m))
    PhoneRouter.shared.searchChanged(active: SearchPhoneModel.shared.active)
    #expect(!PhoneRouter.shared.nav.searching)
    #expect(m.surface == "iphone-inbox")
    #expect(PhoneRouter.shared.nav.path == [.inbox])
}

@Test @MainActor func thePalettesSearchCommandOpensTheSearchNotAnUnknownOverlay() throws {
    let m = try model()
    defer { if SearchPhoneModel.shared.active { _ = SearchPhoneModel.shared.close() } }
    m.overlay = "iphone-search"                                                        // what PalettePhoneModel.run does for its Search line
    PhoneRouter.shared.overlayChanged()
    #expect(SearchPhoneModel.shared.active)                                            // mutant: overlayChanged() has no `searchWord` branch
    #expect(PhoneRouter.shared.nav.searching)
}

// MARK: Mailboxes

@Test @MainActor func settingsAndLabelsOpenFromMailboxesByTheirIdsAndGoBackToMailboxes() throws {
    let m = try model()
    #expect(try click("iphone-inbox.back", m))
    #expect(PhoneRouter.shared.nav.path == [])                                         // mutant: `iphone-inbox.back` only sets the surface word
    #expect(m.surface == "iphone-mailboxes")

    #expect(try click("iphone-mailboxes.settings", m))                                 // mutant: the click opens `openLabelsContacts`
    #expect(PhoneRouter.shared.nav.path == [.settings])
    #expect(m.overlay == "iphone-settings")
    #expect(try rows("iphone-settings.title", m).count == 1, "Settings' rows answer once it is open")
    #expect(try click("iphone-settings.done", m))
    PhoneRouter.shared.overlayChanged()
    #expect(PhoneRouter.shared.nav.path == [], "Settings' Done lands on Mailboxes, where it came from")
    #expect(m.overlay == nil)

    #expect(try click("iphone-mailboxes.labels", m))                                   // mutant: the click opens `openSettings`
    #expect(PhoneRouter.shared.nav.path == [.labelsContacts])
    #expect(m.overlay == "iphone-labels-contacts")
    #expect(try #require(try rows("iphone-labels-contacts.page", m).first)["kind"] as? String == "list")
    #expect(try click("iphone-labels-contacts.back", m))
    PhoneRouter.shared.overlayChanged()
    #expect(PhoneRouter.shared.nav.path == [])
}

@Test @MainActor func mailboxesRowsAnswerOnlyOnMailboxes() throws {
    let m = try model()
    #expect(try click("iphone-mailboxes.settings", m) == false, "the launch lands on the inbox: Mailboxes draws no Settings row there")   // mutant: the guard `onMailboxes` removed
    #expect(try click("iphone-mailboxes.labels", m) == false)
    #expect(PhoneRouter.shared.nav.path == [.inbox])
}

@Test @MainActor func theInboxsMoreMenuSettingsPushesSettingsOverTheInboxAndBackReturnsToTheInbox() throws {
    let m = try model()
    PhoneRouter.shared.openSettings()
    #expect(PhoneRouter.shared.nav.path == [.inbox, .settings])
    #expect(m.overlay == "iphone-settings")
    #expect(try click("iphone-settings.done", m))
    PhoneRouter.shared.overlayChanged()
    #expect(PhoneRouter.shared.nav.path == [.inbox])
    #expect(m.overlay == nil)
}

@Test @MainActor func aThreadOpenedFromALabelsListPushesOverTheLabelsAndBackReturnsToThem() throws {
    let m = try model()
    defer { ThreadPhoneState.shared.reset(); m.overlay = nil }
    PhoneRouter.shared.send(.showMailboxes)
    PhoneRouter.shared.openLabelsContacts()
    let id = try #require(m.rows.first).messageId
    #expect(LabelsContactsPhoneState.shared.openThread(id, in: m))                     // the labels screen's own door: focus + overlay word
    PhoneRouter.shared.overlayChanged()
    #expect(PhoneRouter.shared.nav.path == [.labelsContacts, .thread(id)])             // mutant: `case .thread?: s.path[s.path.count - 1] = .thread(id)` -> `case .thread?, .labelsContacts?: ...` (a thread replaces the labels)
    #expect(try click("iphone-thread.back", m))
    PhoneRouter.shared.overlayChanged()
    #expect(PhoneRouter.shared.nav.path == [.labelsContacts])
    #expect(m.overlay == "iphone-labels-contacts", "the overlay word names the screen now on top")
}

// MARK: a notification's tap

@Test @MainActor func aNotificationsTapOpensTheMailsThreadOverTheInboxFromSettingsAndBackLandsOnTheInbox() throws {
    let m = try model()
    defer { ThreadPhoneState.shared.reset(); m.overlay = nil }
    PhoneRouter.shared.send(.showMailboxes)
    PhoneRouter.shared.openSettings()
    let id = try #require(m.rows.last).messageId
    let reply = try FixtureVerbs.run(m, ["notification-tap", id])                       // mutant: `notificationTapped` sends `.openThread` (keeps Settings below)
    #expect(reply["opened"] as? String == id)
    #expect(PhoneRouter.shared.nav.path == [.inbox, .thread(id)])
    #expect(m.overlay == "iphone-thread")
    #expect(m.focusedMessageId == id, "the thread screen reads the focused mail: it is this one")
    #expect(try click("iphone-thread.back", m))
    PhoneRouter.shared.overlayChanged()
    #expect(PhoneRouter.shared.nav.path == [.inbox])
}

@Test @MainActor func aMessageLinkOpensTheSameThreadAndAnUnknownMailIsRefused() throws {
    let m = try model()
    defer { ThreadPhoneState.shared.reset(); m.overlay = nil }
    let id = try #require(m.rows.first).messageId
    #expect(PhoneRouter.shared.open(url: try #require(URL(string: "whoosh://message/\(id)"))))
    #expect(PhoneRouter.shared.nav.path == [.inbox, .thread(id)])
    #expect(!PhoneRouter.shared.open(url: try #require(URL(string: "whoosh://message/no-such-mail"))), "a link to a mail the core does not list opens nothing")
    #expect(PhoneRouter.shared.nav.path == [.inbox, .thread(id)])
    #expect(throws: ControlFailure.self) { _ = try FixtureVerbs.run(m, ["notification-tap", "no-such-mail"]) }
}

// MARK: the test engine's verbs for the wiring

@Test @MainActor func theSpeechModelVerbPostsItsWordForTheComposersMicrophoneAndRefusesAnyOther() throws {
    let m = try model()
    var seen: [String] = []
    let token = NotificationCenter.default.addObserver(forName: FixtureEvents.name("speech-model"), object: nil, queue: nil) { n in
        seen.append((n.userInfo?["args"] as? [String])?.first ?? "")
    }
    defer { NotificationCenter.default.removeObserver(token) }
    _ = try FixtureVerbs.run(m, ["speech-model", "ready"])                              // mutant: the verb posts under the name `speech`
    _ = try FixtureVerbs.run(m, ["speech-model", "missing"])
    #expect(seen == ["ready", "missing"])
    #expect(throws: ControlFailure.self) { _ = try FixtureVerbs.run(m, ["speech-model", "half"]) }
}

@Test @MainActor func theSyncVerbSetsAndClearsTheBrokenStateAndRefusesAnyOtherWord() throws {
    let m = try model()
    defer { FixtureVerbs.syncBroken = false }
    let broken = try FixtureVerbs.run(m, ["sync", "broken"])
    #expect(broken["broken"] as? Bool == true)                                          // mutant: `syncBroken = a[1] == "broken"` -> `a[1] == "ok"`
    #expect(FixtureVerbs.syncBroken)
    let ok = try FixtureVerbs.run(m, ["sync", "ok"])
    #expect(ok["broken"] as? Bool == false)
    #expect(!FixtureVerbs.syncBroken)
    #expect(throws: ControlFailure.self) { _ = try FixtureVerbs.run(m, ["sync", "sideways"]) }
}
