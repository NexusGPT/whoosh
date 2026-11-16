import Foundation
import Testing
@testable import WhooshUI

/// Probes for root delta 3: the body read and AddAccount on the fixture core.
private func core() throws -> FixtureCore {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    return try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
}

@Test func everyFixtureMessageHasABody() throws {
    let c = try core()
    for r in c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)) {
        let b = try #require(c.body(messageId: r.messageId), "no body for \(r.messageId)")
        #expect(!b.plainText.isEmpty)
        #expect(!(b.safeHtml ?? "").contains("<script"))
    }
    #expect(c.body(messageId: "no-such-message") == nil)
}

@Test func theQuotedPartIsMarkedInBothForms() throws {
    let c = try core()
    let b = try #require(c.body(messageId: "msg-03"))
    let html = try #require(b.safeHtml)
    let offset = Int(try #require(b.quoteOffset))
    #expect(String(decoding: Array(html.utf8)[offset...], as: UTF8.self).hasPrefix("<blockquote>"))
    let r = try #require(b.quotedRanges.first)
    let quoted = String(decoding: Array(b.plainText.utf8)[Int(r.start)..<Int(r.end)], as: UTF8.self)
    #expect(quoted.hasPrefix("> "))
    #expect(!String(decoding: Array(b.plainText.utf8)[..<Int(r.start)], as: UTF8.self).contains("> "))
    #expect(c.body(messageId: "msg-04")?.quotedRanges.isEmpty == true)
    #expect(c.body(messageId: "msg-04")?.trackersBlocked == 2)
}

@Test func addAccountStoresTheAddressAndRefusesADuplicate() throws {
    let c = try core()
    let out = try c.run(command: .addAccount(address: "New.Person@outlook-example.com", provider: .microsoft))
    #expect(out.createdId == "acc-31")
    #expect(c.state(path: "account/acc-31/address") == "\"new.person@outlook-example.com\"")
    #expect(c.state(path: "account/count") == "31")
    #expect(throws: CoreError.self) { try c.run(command: .addAccount(address: "new.person@outlook-example.com", provider: .microsoft)) }
    #expect(throws: CoreError.self) { try c.run(command: .addAccount(address: "not-an-address", provider: .imap)) }
}
