import Foundation
import Testing
@testable import WhooshUI

// Lane gap-phone-wire: the iPhone's navigation model (Sources/App/PhoneRoutes.swift), a value and a reducer. No simulator, no view, no core:
// each test drives `PhoneNavReducer.reduce` alone. Each test names, in its trailing comment, the one-line broken copy of PhoneRoutes.swift that
// must turn it red (the same copies are the `mutants` of checks/proofs/g5/7b25ddc2.json).

private func run(_ from: PhoneNav = PhoneNav(), _ actions: PhoneNavAction...) -> PhoneNav {
    var s = from
    for a in actions { PhoneNavReducer.reduce(&s, a) }
    return s
}
private let inboxOnly = PhoneNav()                                    // the launch state: Mailboxes root, the inbox pushed
private let mailboxes = run(inboxOnly, .showMailboxes)                // path []
private let overThread = run(inboxOnly, .openThread("m1"))

// MARK: each entry point reaches its screen

@Test func aTapOnAnInboxRowPushesItsThreadOverTheInbox() {
    let s = run(inboxOnly, .openThread("m1"))
    #expect(s.path == [.inbox, .thread("m1")])                        // mutant: `s.path.append(.thread(id))` -> `s.path.append(.inbox)`
    #expect(s.overlayName == "iphone-thread")
    #expect(s.surfaceName == "iphone-thread")
    #expect(s.sheet == nil)
}

@Test func settingsOpensFromTheMailboxesScreenAsAPush() {
    let s = run(mailboxes, .openSettings)
    #expect(s.path == [.settings])                                    // mutant: `if s.path.last != .settings { s.path.append(.settings) }` -> appends .labelsContacts
    #expect(s.overlayName == "iphone-settings")
    #expect(run(s, .openSettings).path == [.settings], "a second tap does not stack a second Settings")
}

@Test func labelsAndContactsOpensFromTheMailboxesScreenAsAPush() {
    let s = run(mailboxes, .openLabelsContacts)
    #expect(s.path == [.labelsContacts])                              // mutant: `if s.path.last != .labelsContacts { s.path.append(.labelsContacts) }` -> appends .settings
    #expect(s.overlayName == "iphone-labels-contacts")
    #expect(s.surfaceName == "iphone-labels-contacts")
}

@Test func theComposerOpensAsASheetOverWhateverIsOnScreenAndLeavesTheStackAlone() {
    for base in [inboxOnly, mailboxes, overThread] {
        let s = run(base, .openComposer)
        #expect(s.sheet == .composer)                                 // mutant: `s.sheet = .composer ` -> `s.sheet = .palette `
        #expect(s.path == base.path)
        #expect(s.overlayName == "iphone-composer")
    }
}

@Test func thePaletteOpensAsASheetOverWhateverIsOnScreenButNeverOverTheComposer() {
    for base in [inboxOnly, mailboxes, overThread] {
        let s = run(base, .openPalette)
        #expect(s.sheet == .palette)                                  // mutant: `if s.sheet != .composer { s.sheet = .palette }` -> `{ s.sheet = .composer }`
        #expect(s.path == base.path)
    }
    #expect(run(inboxOnly, .openComposer, .openPalette).sheet == .composer, "the composer holds a draft: the palette does not cover it")
    #expect(run(inboxOnly, .openPalette, .openComposer).sheet == .composer, "a palette command that composes hands its place to the composer")
}

@Test func searchOpensInPlaceInTheInboxAndNothingIsPushedForIt() {
    let s = run(inboxOnly, .openSearch)
    #expect(s.searching)                                              // mutant: `s.searching = true` -> `s.searching = false`
    #expect(s.path == [.inbox])
    #expect(s.surfaceName == "iphone-search")
    #expect(run(mailboxes, .openSearch).path == [.inbox], "opened from the socket on Mailboxes: the inbox comes first")
    #expect(run(overThread, .openSearch).path == [.inbox], "a thread above the inbox gives way to the search")
    #expect(!run(s, .closeSearch).searching)
}

// MARK: Back returns to where it came from

@Test func backFromAThreadReturnsToTheInboxWithTheInboxStillBelow() {
    let s = run(overThread, .back)
    #expect(s.path == [.inbox])                                       // mutant: `else if !s.path.isEmpty { s.path.removeLast() }` -> `{ s.path = [] }`
    #expect(run(mailboxes, .openSettings, .back).path == [])
    #expect(run(mailboxes, .openLabelsContacts, .openThread("m2"), .back).path == [.labelsContacts], "a thread opened from a label's list goes back to the labels")
    #expect(run(mailboxes, .back).path == [], "Back on the root has nowhere to go")
}

@Test func backClosesTheSheetFirstThenTheSearchThenOnePush() {
    let sheetOverThread = run(overThread, .openComposer)
    #expect(run(sheetOverThread, .back) == overThread)
    let searchOverInbox = run(inboxOnly, .openSearch)
    #expect(run(searchOverInbox, .back) == inboxOnly)
    #expect(run(searchOverInbox, .back, .back).path == [])
}

@Test func backFromAThreadOpenedOnASearchLandsOnTheSearchStillOpen() {
    let s = run(inboxOnly, .openSearch, .openThread("m3"), .back)
    #expect(s.path == [.inbox])
    #expect(s.searching, "the results the thread was opened from are still there")
    #expect(s.surfaceName == "iphone-search")
}

@Test func theSystemsOwnSwipeBackSetsTheStackAndKeepsTheOthers() {
    let s = run(inboxOnly, .openThread("m1"), .pathChanged([.inbox]))
    #expect(s.path == [.inbox])                                       // mutant: `s.path = path` -> `s.path = s.path`
    #expect(run(inboxOnly, .openSearch, .pathChanged([])).searching == false, "leaving the inbox ends its search")
}

// MARK: a deep link from a notification

@Test func aNotificationsTapOpensTheThreadOverTheInboxFromWhateverWasOnScreen() {
    let from: [PhoneNav] = [inboxOnly, mailboxes, run(mailboxes, .openSettings), run(inboxOnly, .openSearch), run(overThread, .openComposer), run(inboxOnly, .openPalette)]
    for base in from {
        let s = run(base, .deepLinkThread("m9"))
        #expect(s.path == [.inbox, .thread("m9")])                    // mutant: `case .deepLinkThread(let id):` -> `case .deepLinkThread(let id): _ = id; return`
        #expect(s.sheet == nil)
        #expect(!s.searching)
        #expect(s.overlayName == "iphone-thread")
    }
    #expect(run(overThread, .deepLinkThread("m9"), .back).path == [.inbox], "Back from the opened thread lands on the inbox")
}

@Test func aMessageLinkNamesItsMailAndNothingElseDoes() {
    #expect(PhoneDeepLink.messageId(URL(string: "whoosh://message/msg-07")!) == "msg-07")
    #expect(PhoneDeepLink.messageId(URL(string: "whoosh://message/")!) == nil)
    #expect(PhoneDeepLink.messageId(URL(string: "whoosh://settings/msg-07")!) == nil)
    #expect(PhoneDeepLink.messageId(URL(string: "https://example.com/message/msg-07")!) == nil)
}

// MARK: a screen's own door sets AppModel.overlay (the router feeds the word in)

@Test func aScreensOverlayWordBecomesItsRouteAndNilLeavesIt() {
    let h = run(mailboxes, .overlayChanged("iphone-settings", focused: nil))
    #expect(h.path == [.settings])                                    // mutant: `case settings: reduce(&s, .openSettings)` -> `.openLabelsContacts`
    #expect(run(h, .overlayChanged(nil, focused: nil)).path == [], "nil pops the screen the word named")
    #expect(run(inboxOnly, .overlayChanged("iphone-thread", focused: "m4")).path == [.inbox, .thread("m4")])
    #expect(run(inboxOnly, .overlayChanged("iphone-composer", focused: nil)).sheet == .composer)
    #expect(run(inboxOnly, .overlayChanged("iphone-palette", focused: nil)).sheet == .palette)
    #expect(run(run(inboxOnly, .openComposer), .overlayChanged(nil, focused: nil)).sheet == nil, "nil closes the sheet before it pops a push")
}

@Test func anOverlayWordWithNoScreenHereAndANilOnTheInboxChangeNothing() {
    #expect(run(inboxOnly, .overlayChanged("iphone-mailbox:VIPs", focused: nil)) == inboxOnly)
    #expect(run(inboxOnly, .overlayChanged("iphone-label-picker", focused: "m1")) == inboxOnly)
    #expect(run(inboxOnly, .overlayChanged(nil, focused: nil)) == inboxOnly, "the router writes nil after a pop: it must not pop twice")
    #expect(run(mailboxes, .overlayChanged(nil, focused: nil)) == mailboxes)
}

@Test func theSameOverlayWordTwiceChangesNothingTheSecondTime() {
    let once = run(inboxOnly, .overlayChanged("iphone-thread", focused: "m1"))
    #expect(run(once, .overlayChanged("iphone-thread", focused: "m1")) == once, "the router writes the word back: it must not push again")
    let settings = run(mailboxes, .overlayChanged("iphone-settings", focused: nil))
    #expect(run(settings, .overlayChanged("iphone-settings", focused: nil)) == settings)
}

@Test func resetPutsTheStackBackOnTheInbox() {
    #expect(run(overThread, .openComposer, .openSearch, .reset) == inboxOnly)
}

// MARK: the first window

@Test func theWelcomeShowsWhenNoAccountExistsAndLeavesWhenOneDoes() {
    #expect(PhoneRoot.showsWelcome(welcomeDone: false, midArrival: false, accounts: 0, testCopy: false))        // mutant: `return accounts == 0 || midArrival` -> `return false`
    #expect(!PhoneRoot.showsWelcome(welcomeDone: false, midArrival: false, accounts: 1, testCopy: false))       // mutant: `return accounts == 0 || midArrival` -> `return true`
    #expect(PhoneRoot.showsWelcome(welcomeDone: false, midArrival: true, accounts: 30, testCopy: false), "the accounts have just landed: the person finishes the steps")
    #expect(!PhoneRoot.showsWelcome(welcomeDone: true, midArrival: false, accounts: 0, testCopy: false))        // mutant: `if welcomeDone { return false }` -> `if false { return false }` (reds only with accounts 0 and testCopy false)
}

@Test func aTestCopyFollowsTheFlowAloneBecauseItsFixtureAlwaysHasAccounts() {
    #expect(PhoneRoot.showsWelcome(welcomeDone: false, midArrival: false, accounts: 30, testCopy: true))        // mutant: `if testCopy { return true }` -> `if testCopy { return false }`
    #expect(!PhoneRoot.showsWelcome(welcomeDone: true, midArrival: false, accounts: 30, testCopy: true))
}

@Test func aPhoneThatFinishedTheWelcomeButLostEveryAccountShowsTheWelcomeAgainAtLaunch() {
    #expect(PhoneRoot.launchIsDone(flagDone: true, accounts: 3, testCopy: false))
    #expect(!PhoneRoot.launchIsDone(flagDone: true, accounts: 0, testCopy: false))                              // mutant: `!testCopy && flagDone && accounts > 0` -> `!testCopy && flagDone`
    #expect(!PhoneRoot.launchIsDone(flagDone: false, accounts: 3, testCopy: false))
    #expect(!PhoneRoot.launchIsDone(flagDone: true, accounts: 3, testCopy: true))
}
