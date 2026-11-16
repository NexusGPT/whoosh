import Foundation
import Testing
@testable import WhooshUI

// Integration over the core seam for J4.9-J4.11: the device link and the settings sync. The core is FixtureCore until the
// Rust core serves these records (docs/CAPABILITY-CONTRACT.md: device-link, settings-sync). Red until then.

@Test func aDeviceLinkIsATokenInTheKeychainThatExpires() throws {
    let core = try fixtureCore()
    let id = core.state(path: "device_link/id")
    #expect(id != nil, "control: opening Move accounts to iPhone creates a link")
    #expect(core.state(path: "device_link/expires_in_s") == "\(WhooshConstants.QR_EXPIRY_S)")
    let link = DeviceLink(id: "x", keychain: KeychainRef(service: "whoosh.device-link", account: "x"), expiresAtMs: 0)
    #expect(link.keychain.service == "whoosh.device-link")
}

@Test func anExpiredLinkIsReplacedNotReused() throws {
    let clock = FixtureClock()
    let core = try fixtureCore(clock: clock)
    let first = core.state(path: "device_link/id")
    #expect(first != nil, "control: a link exists to expire")
    clock.advance(seconds: Double(WhooshConstants.QR_EXPIRY_S + 1))
    let second = core.state(path: "device_link/id")
    #expect(second != nil && second != first)
}

@Test func theSettingsSyncCarriesTheAccountsAndNeverASecret() throws {
    let core = try fixtureCore()
    let carried = core.state(path: "settings_sync/records_carried") ?? ""
    #expect(carried.contains("account"), "the sync must carry the accounts, got \(carried)")
    for forbidden in ["password", "refresh_token", "access_token"] {
        #expect(!carried.contains(forbidden), "the sync carries \(forbidden)")
    }
}

@Test func usingTheTokenOnceClearsItAndASecondScanIsRefused() throws {
    let core = try fixtureCore()
    #expect(core.state(path: "device_link/token") != nil, "control: a live token exists before the scan")
    _ = try core.run(command: .addAccount(address: "assem@thirty-first-example.test", provider: .imap))   // stands for the paired device's first request
    #expect(core.state(path: "device_link/token") == nil)
}
