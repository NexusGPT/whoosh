import Foundation
import Testing
@testable import WhooshUI

// mob-account-mark-iphone: the account mark on the phone says the same thing as the Mac's for the same account.
// Expected values come from the design fixture and from the Mac's own account list (`core.accounts()`, what the Mac sidebar reads).
// The SwiftUI modifier itself (the rendered accessibility tree at 402 x 874) needs the iOS 26.5 simulator; this proves the value it is given.

private func markFixturePath() -> String {
    URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path
}
@MainActor private func markModel() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    setenv("WHOOSH_FIXTURE_PATH", markFixturePath(), 1)
    let m = SharedApp.begin()
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}
@MainActor private func markRows(_ ax: String, _ m: AppModel) throws -> [ControlRow] {
    // two statements: calling the result of #require in one expression crashes the compiler (signal 6)
    let source = try #require(Registrations.rowSource(ax), "no rows are registered at \(ax)")
    return try source(m)
}

@MainActor @Test func theMailboxesMarksCarryTheMacsAccountNameColourAndIconInTheMacsOrder() throws {
    let m = try markModel()
    let mac = m.core.accounts()
    let phone = try markRows("iphone-mailboxes.accounts", m)
    #expect(mac.count == 30 && phone.count == mac.count)
    #expect(phone.map { $0["id"] as? String } == mac.map(\.id), "the marks come in the Mac's account order")   // mutant: the accounts list reversed
    #expect(phone.map { $0["mark_label"] as? String } == mac.map(\.name))                                       // mutant: PhoneAccount.markLabel answers the address
    #expect(phone.map { ($0["mark_colour"] as? String)?.lowercased() } == mac.map { $0.colour.lowercased() })
    #expect(phone.map { $0["mark_icon"] as? String } == mac.map { $0.iconId ?? "provider-default" })           // mutant: PhoneAccounts passes iconId nil
    #expect(Set(phone.compactMap { $0["mark_label"] as? String }).count == phone.count, "every mark has its own label")
}

@MainActor @Test func everyListRowMarkIsLabelledWithItsAccountNameAndAnAccountWithoutAPickedIconSaysProviderDefault() throws {
    let m = try markModel()
    let names = Dictionary(uniqueKeysWithValues: m.core.accounts().map { ($0.id, $0.name) })
    let list = try markRows("iphone-inbox.list", m)
    #expect(!list.isEmpty)
    for row in list {
        let account = try #require(row["account"] as? String)
        #expect(row["mark_label"] as? String == names[account], "row \(row["id"] ?? "?") is on \(account)")
        #expect(!((row["mark_label"] as? String) ?? "").isEmpty, "a mark with no label is silent to a screen reader")
    }
    // the rule for an account with no picked icon, on the type the probe reads (the Mac's rule, PhoneAccounts.swift)
    let none = PhoneAccount(id: "x", name: "X", address: "x@example.test", colour: "#112233", group: nil, iconId: nil, needsAttention: false)
    #expect(none.markIcon == "provider-default")
    #expect(none.markLabel == "X")
    let picked = PhoneAccount(id: "y", name: "Y", address: "y@example.test", colour: "#112233", group: nil, iconId: "code", needsAttention: false)
    #expect(picked.markIcon == "code")
    let unknown = PhoneAccount(id: "z", name: "Z", address: "z@example.test", colour: "#112233", group: nil, iconId: "not-in-the-library", needsAttention: false)
    #expect(unknown.markIcon == "provider-default", "an id the library does not hold falls back like the Mac")
}
