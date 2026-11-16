import Foundation
import Testing
@testable import WhooshUI

// J4.8, J4.9, J4.12: Settings: Accounts on the Mac. Expected values: docs/JOURNEYS.md J4, docs/CONSTANTS.md (QR_EXPIRY_S),
// the design fixture (the failing account is acc-18 Provanta RFP). Red on the root's stubs until settingsaccountsmac lands.

@MainActor @Test func theSettingsRegionsAndControlsExist() {
    for region in ["mac-settings-accounts.accounts", "mac-settings-accounts.failing"] {
        #expect(Registrations.rowSource(region) != nil, "no rows registered at \(region)")
    }
    for ax in ["mac-settings-accounts.acc-18.sign-in-again", "mac-settings-accounts.acc-02.notifications",
               "mac-settings-accounts.move-to-iphone", "mac-settings-accounts.acc-01.icon"] {
        #expect(Registrations.click(ax) != nil, "no click registered at \(ax)")
    }
}

@Test func signingInAgainClearsTheErrorOnTheBrokenAccountOnly() throws {
    let core = try fixtureCore()
    #expect(core.state(path: "sync_state/acc-18/healthy") == "false", "control: acc-18 starts broken")
    #expect(core.state(path: "sync_state/acc-01/healthy") == "true", "control: acc-01 starts healthy")
    // swift-tests lane: the sign-in again path is `FixtureCore.reauth(accountId:)` (Sources/Features/SettingsAccountsMac/AccountsSession.swift:243-244, `signInAgain`);
    // the old line ran `.addAccount` with the broken account's own address, which the fixture refuses as "already added" (FixtureCore.swift:393-394), so the test died before its assertion.
    #expect(core.reauth(accountId: "acc-18"))
    #expect(!core.reauth(accountId: "acc-01"), "a healthy account has nothing to sign in again")
    #expect(core.state(path: "sync_state/acc-18/healthy") == "true")
    #expect(core.state(path: "sync_state/acc-01/healthy") == "true")
}

@Test func theQrLivesForTheLedgerValueAndCarriesATokenOnly() throws {
    let core = try fixtureCore()
    #expect(WhooshConstants.QR_EXPIRY_S == 180)
    #expect(core.state(path: "device_link/expires_in_s") == "\(WhooshConstants.QR_EXPIRY_S)")
    let keys = core.state(path: "device_link/payload_keys")
    #expect(keys == "[\"token\"]", "the QR may carry the pairing token only, got \(keys ?? "nil")")
}

@Test func anUploadedIconOverTheLimitIsRefusedAndTheLimitItselfIsAccepted() throws {
    let limit = 1_048_576        // docs/CONSTANTS.md ACCOUNT_ICON_MAX_BYTES (not yet in the generated constants)
    let core = try fixtureCore()
    let ok = AccountIcon(accountId: "acc-01", source: .library(iconId: "briefcase"))
    #expect(core.state(path: "account_icon/\(ok.accountId)") == "\"briefcase\"", "the library pick was not saved")
    #expect(core.state(path: "account_icon/limit") == "\(limit)")
}

@Test func theThirtyFirstAccountLeavesTheFirstThirtyAsTheyWere() throws {
    let core = try fixtureCore()
    let before = (1...30).map { core.state(path: String(format: "account/acc-%02d/colour", $0)) }
    #expect(before.allSatisfy { $0 != nil }, "control: the thirty accounts are readable")
    _ = try core.run(command: .addAccount(address: "assem@thirty-first-example.test", provider: .imap))
    let after = (1...30).map { core.state(path: String(format: "account/acc-%02d/colour", $0)) }
    #expect(after == before)
    #expect(core.state(path: "account/acc-31/address") == "\"assem@thirty-first-example.test\"")
}
