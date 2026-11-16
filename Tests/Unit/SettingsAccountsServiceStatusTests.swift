import Foundation
import Testing
@testable import WhooshUI

// Settings > Accounts, "Mail services": one row per service (Gmail, Outlook.com, iCloud, Fastmail, Zoho), the state in words from THIS Mac's own
// accounts of that service. Nothing is fetched from a service. Expected values come from the design fixture (docs/design/fixtures/mac-main-sample.json:
// acc-18 Provanta RFP, a Microsoft account, needs a sign-in; no account is Zoho) and from the rule in Sources/Features/SettingsAccountsMac/ServiceStatus.swift.
// MUTANTS: each test names in its trailing comment the one-line break of the target that must turn it red.

private func sync(_ id: String, error: ActionFailed? = nil) -> SyncState {
    SyncState(accountId: id, historyFetched: true, messagesIndexed: 10, lastError: error, providerStatusWords: nil, lastSyncedMs: nil)
}
private func entry(_ id: String, _ service: MailService?, paused: Bool = false) -> AccountEntry {
    AccountEntry(id: id, name: id, address: id + "@example.test", provider: "p", colour: "#000000", iconId: nil, imagePath: nil, group: nil, paused: paused, backfill: 1, signature: "", service: service)
}
private let signInFailure = ActionFailed(accountName: "x", reasonWords: "needs you to sign in again", retryable: true)
private let serverFailure = ActionFailed(accountName: "x", reasonWords: "could not fetch the inbox", retryable: true, code: "sync.failed?step=fetch&folder=INBOX")

private func state(_ service: MailService, _ accounts: [AccountEntry], _ states: [SyncState]) -> ServiceState {
    let rows = ServiceStatusRow.rows(accounts: accounts, states: Dictionary(uniqueKeysWithValues: states.map { ($0.accountId, $0) }))
    return rows.first { $0.service == service }!.state
}

@Test func theFiveServicesAreListedInTheFixedOrder() {
    let rows = ServiceStatusRow.rows(accounts: [], states: [:])
    #expect(rows.map(\.service.rawValue) == ["gmail", "outlook", "icloud", "fastmail", "zoho"])   // MUTANT: allCases lists Zoho first
    #expect(rows.map(\.service.name) == [Words.text("Gmail"), Words.text("Outlook.com"), Words.text("iCloud"), Words.text("Fastmail"), Words.text("Zoho")])
}

@Test func aServiceWithNoAccountOnThisMacSaysSo() {
    #expect(state(.zoho, [entry("a", .gmail)], [sync("a")]) == .noAccount)   // MUTANT: combine([]) returns .connected
    #expect(ServiceState.noAccount.words == Words.text("No account on this Mac"))
}

@Test func aWorkingAccountMakesItsServiceConnected() {
    #expect(state(.gmail, [entry("a", .gmail)], [sync("a")]) == .connected)   // MUTANT: a healthy account reads .noAccount
    #expect(state(.outlook, [entry("a", .gmail)], [sync("a")]) == .noAccount, "an account of another service does not count")
}

@Test func aFailureWithoutASyncCodeNeedsSignIn() {
    #expect(state(.outlook, [entry("a", .outlook), entry("b", .outlook)], [sync("a"), sync("b", error: signInFailure)]) == .needsSignIn)   // MUTANT: combine ignores .needsSignIn
    #expect(ServiceState.needsSignIn.words == Words.text("Needs sign-in"))
}

@Test func aFailureWithASyncCodeIsAServerOutOfReach() {
    #expect(state(.icloud, [entry("a", .icloud)], [sync("a", error: serverFailure)]) == .unreachable)   // MUTANT: every failure reads .needsSignIn
    #expect(state(.icloud, [entry("a", .icloud), entry("b", .icloud)], [sync("a", error: serverFailure), sync("b", error: signInFailure)]) == .needsSignIn, "a sign-in that is needed comes first")
    #expect(ServiceState.unreachable.words == Words.text("Cannot reach the server"))
}

@Test func onlyPausedAccountsMakeTheServicePaused() {
    #expect(state(.fastmail, [entry("a", .fastmail, paused: true)], [sync("a")]) == .paused)   // MUTANT: the paused flag is ignored
    #expect(state(.fastmail, [entry("a", .fastmail, paused: true), entry("b", .fastmail)], [sync("a"), sync("b")]) == .connected, "one account still working")
    #expect(state(.fastmail, [entry("a", .fastmail, paused: true)], [sync("a", error: signInFailure)]) == .paused, "a paused account stops fetching: nothing to sign in")
    #expect(ServiceState.paused.words == Words.text("Paused"))
}

@Test func everyStateHasItsOwnWords() {
    let words = ServiceState.allCases.map(\.words)
    #expect(Set(words).count == words.count && !words.contains(""))   // MUTANT: two states share a sentence
}

@Test func aZohoAddressThatCameInAsOtherMailIsZoho() {
    #expect(MailService.of(provider: .imap, address: "me@zoho.com") == .zoho)   // MUTANT: imap maps to nil for every host
    #expect(MailService.of(provider: .imap, address: "me@zohomail.eu") == .zoho)
    #expect(MailService.of(provider: .imap, address: "me@srv.example.test") == nil)
    #expect(MailService.of(provider: .google, address: "me@zoho.com") == .gmail, "the core's route wins")
    #expect([MailService.of(provider: .google, address: "a@b.c"), .of(provider: .microsoft, address: "a@b.c"), .of(provider: .icloud, address: "a@b.c"),
             .of(provider: .fastmail, address: "a@b.c")] == [.gmail, .outlook, .icloud, .fastmail])
}

@MainActor @Test func theFixtureShowsOutlookNeedingSignInAndNoZohoAccount() throws {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let core = try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
    let accounts = core.accounts().map { a in
        AccountEntry(id: a.id, name: a.id, address: a.address, provider: "p", colour: a.colour, iconId: nil, imagePath: nil, group: a.group, paused: a.paused,
                     backfill: a.backfill, signature: "", service: MailService.of(provider: a.provider, address: a.address))
    }
    let states = Dictionary(uniqueKeysWithValues: core.syncStates().map { ($0.accountId, $0) })
    let got = Dictionary(uniqueKeysWithValues: ServiceStatusRow.rows(accounts: accounts, states: states).map { ($0.service, $0.state) })
    #expect(got[.outlook] == .needsSignIn, "acc-18 needs a sign-in and is a Microsoft account")   // MUTANT: every service reads .connected
    #expect(got[.gmail] == .connected && got[.icloud] == .connected && got[.fastmail] == .connected)
    #expect(got[.zoho] == .noAccount)
}
