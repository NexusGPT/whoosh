import Foundation
import Testing
@testable import WhooshUI

// J4.10, J4.11: Welcome and Accounts on the iPhone, and Settings > Sync. Expected values: docs/JOURNEYS.md J4 and the
// design fixture (30 accounts, names, colours, order). Red on the root's stubs until welcomephone / settingsphone land.

@MainActor @Test func theScanControlAndTheArrivalRowsAreRegistered() {
    #expect(Registrations.click("iphone-welcome.scan") != nil)
    #expect(Registrations.click("iphone-welcome.allow") != nil)
    #expect(Registrations.rowSource("iphone-welcome.accounts") != nil)
    #expect(Registrations.rowSource("iphone-settings.sync") != nil)
}

// Broken copy: delete the `settings_sync/devices` or the `settings_sync/last_sync_ms` case in FixtureCore.state(path:) (both read nil).
@Test func theSyncLineNamesTheMacAndARecentTime() throws {
    let core = try fixtureCore()
    let s = core.state(path: "settings_sync/devices")
    #expect(s == "[\"Mac\"]", "the sync state must name the Mac, got \(s ?? "nil")")
    let last = core.state(path: "settings_sync/last_sync_ms")
    #expect(last != nil, "a last sync time must be present")
}

// Broken copy: make `credentialCarriedLocked` answer false (an account whose credential did not arrive: the count is 1 or more), or delete the `iphone/password_prompts` case (nil).
@Test func theIphoneNeverPromptsForAPasswordAfterTheScan() throws {
    let core = try fixtureCore()
    #expect(core.state(path: "account/acc-01/address") != nil, "control: the account list is readable")
    #expect(core.state(path: "iphone/password_prompts") == "0")
}

/// True when every account has its own address: the scan must carry thirty distinct accounts.
func addressesAreUnique(_ accounts: [[String: Any]]) -> Bool {
    Set(accounts.compactMap { $0["address"] as? String }).count == accounts.count
}

@Test func theFixtureHoldsThirtyAccountsWithUniqueAddressesAsTheScanMustCarry() throws {
    let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("docs/design/fixtures/mac-main-sample.json")
    let root = try #require(try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
    let accounts = try #require(root["accounts"] as? [[String: Any]])
    let thirty = 30      // J4: "30 accounts" (docs/JOURNEYS.md); the ledger holds no account count
    #expect(accounts.count == thirty)
    #expect(addressesAreUnique(accounts))
}

@Test func theUniquenessProbeSeesADuplicate() {      // positive control: the probe can say no
    let dup: [[String: Any]] = [["address": "a@x.test"], ["address": "a@x.test"]]
    #expect(!addressesAreUnique(dup))
    #expect(addressesAreUnique([["address": "a@x.test"], ["address": "b@x.test"]]))
}

// ---- the phone's own screen, driven through its registered clicks, `type` door and rows (the same doors the socket uses).
// Each test names, in its trailing comment, the mutation of the target that must turn it red.

private func welcomeFixturePath() -> String {
    URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path
}
/// The shared model on the design fixture, with the first launch started again (the test door a test copy offers).
@MainActor private func welcomeModel() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    setenv("WHOOSH_FIXTURE_PATH", welcomeFixturePath(), 1)
    let m = SharedApp.begin()
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    let restart = try #require(Registrations.click("iphone-welcome.test-restart"), "no test-restart door is registered")
    try #require(restart(m), "a test copy must accept the test restart")
    return m
}
@MainActor private func welcomeRows(_ ax: String, _ m: AppModel) throws -> [ControlRow] {
    // two statements: calling the result of #require in one expression crashes the compiler (signal 6)
    let source = try #require(Registrations.rowSource(ax), "no rows are registered at \(ax)")
    return try source(m)
}
@MainActor private func welcomeStep(_ m: AppModel) throws -> String {
    try #require(try welcomeRows("iphone-welcome.step", m).first?["step"] as? String)
}
@MainActor private func welcomeClick(_ ax: String, _ m: AppModel) throws {
    let c = try #require(Registrations.click(ax), "no click is registered at \(ax)")
    #expect(c(m), "the click \(ax) was refused")
}
/// Opens the scanning step and types one test word, as the socket's `type` does.
@MainActor private func welcomeScan(_ word: String, _ m: AppModel) throws {
    if try welcomeStep(m) != "scanning" { try welcomeClick("iphone-welcome.scan", m) }
    m.surface = WelcomePhoneFeature.surface
    #expect(WelcomePhoneFeature.typeText(word, m), "the scanning step did not take \(word)")
}

@MainActor @Test func aCodeIssuedAndLeftPastItsExpiryIsRefusedAsExpiredAndAsksForANewScan() throws {
    let m = try welcomeModel()
    defer { m.host.clock.advance(seconds: -(Double(WhooshConstants.QR_EXPIRY_S) + 1)) }   // the next test starts on the launch clock
    try welcomeScan("fixture:issue", m)
    #expect(try welcomeStep(m) == "scanning", "issuing a code reads nothing yet")
    #expect(try welcomeRows("iphone-welcome.refusal", m).isEmpty)
    m.host.clock.advance(seconds: Double(WhooshConstants.QR_EXPIRY_S) + 1)
    try welcomeScan("fixture:scan-issued", m)
    #expect(try welcomeStep(m) == "refused")                                                  // mutant: PairingService(expirySeconds: 100000)
    let refusal = try #require(try welcomeRows("iphone-welcome.refusal", m).first)
    #expect(refusal["reason"] as? String == "expired")
    #expect(refusal["scan_again"] as? Bool == true)
    #expect((refusal["text"] as? String)?.contains(String(WhooshConstants.QR_EXPIRY_S / 60)) == true, "the words name the minutes")
    let controls = try welcomeRows("iphone-welcome.controls", m).compactMap { $0["ax"] as? String }
    #expect(controls.contains("iphone-welcome.scan-again"), "a refused code offers a new scan, got \(controls)")
    try welcomeClick("iphone-welcome.scan-again", m)
    #expect(try welcomeStep(m) == "scanning")
}

@MainActor @Test func aCodeScannedInTimeIsNotRefusedAndAnUnknownCodeIsRefusedAsNotFromTheMac() throws {   // the controls of the expiry test
    let m = try welcomeModel()
    try welcomeScan("fixture:issue", m)
    try welcomeScan("fixture:scan-issued", m)
    #expect(try welcomeStep(m) == "arriving", "a code read at once pairs")
    #expect(try welcomeRows("iphone-welcome.refusal", m).isEmpty)
    try welcomeClick("iphone-welcome.test-restart", m)
    try welcomeScan("fixture:wrong", m)
    #expect(try welcomeRows("iphone-welcome.refusal", m).first?["reason"] as? String == "unknown")
}

@MainActor @Test func theArrivalRowsAreTheFixtureAccountsInNamesColoursAndOrderAndNoStepDrawsAField() throws {
    let m = try welcomeModel()
    let root = try #require(try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: welcomeFixturePath()))) as? [String: Any])
    let want = try #require(root["accounts"] as? [[String: Any]])
    var steps: [String] = []
    func noFields() throws {
        let step = try welcomeStep(m)
        steps.append(step)
        #expect(try welcomeRows("iphone-welcome.fields", m).isEmpty, "the \(step) step draws a field: the phone never asks for a password")
    }
    try noFields()                                                                              // first
    try welcomeClick("iphone-welcome.scan", m)
    try noFields()                                                                              // scanning
    try welcomeScan("fixture:wrong", m)
    try noFields()                                                                              // refused
    try welcomeClick("iphone-welcome.scan-again", m)
    try welcomeScan("fixture:valid", m)
    try noFields()                                                                              // arriving
    let arrived = try welcomeRows("iphone-welcome.accounts", m)
    #expect(arrived.count == want.count)                                                         // thirty
    #expect(arrived.map { $0["id"] as? String } == want.map { $0["id"] as? String })           // mutant: the accounts arrive reversed
    #expect(arrived.map { $0["name"] as? String } == want.map { $0["name"] as? String })
    #expect(arrived.map { ($0["mark_colour"] as? String)?.lowercased() } == want.map { ($0["colour"] as? String)?.lowercased() })   // mutant: one colour on every account
    #expect(arrived.map { $0["mark_label"] as? String } == want.map { $0["name"] as? String })
    let count = try #require(try welcomeRows("iphone-welcome.count", m).first)
    #expect(count["arrived"] as? Int == want.count && count["total"] as? Int == want.count && count["complete"] as? Bool == true)
    try welcomeClick("iphone-welcome.continue", m)
    try noFields()                                                                              // notify
    #expect(steps == ["first", "scanning", "refused", "arriving", "notify"], "every step was read, got \(steps)")   // mutant: fields(.scanning) returns a secure field
}
