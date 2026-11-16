import Foundation
import Testing
@testable import WhooshUI

/// Probes for contracts delta 14: the fixture's bring-your-own OAuth2 sign-in, and a smart folder opening its list.
private let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func core() throws -> FixtureCore {
    try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
}
/// A Microsoft-shaped client ID built from parts (the repository holds no real one: checks/no-vendor-id-check.py).
private let clientId = [8, 4, 4, 4, 12].map { String(repeating: "d", count: $0) }.joined(separator: "-")

@Test func d14TheFixtureSignInWaitsOnTheFakeAuthorityAndTheAnswerFinishesIt() throws {
    let c = try core()
    let a = "Assem@Northlight-Example.DE"
    #expect(c.oauthSignInState(accountDraftId: a) == .needsClientId)
    #expect(throws: CoreError.self) { try c.run(command: .startOAuthSignIn(accountDraftId: a)) }
    #expect(throws: CoreError.self) { try c.run(command: .setOAuthClient(accountDraftId: a, provider: .microsoft, clientId: "not an id")) }
    _ = try c.run(command: .setOAuthClient(accountDraftId: a, provider: .microsoft, clientId: clientId))
    #expect(c.oauthSignInState(accountDraftId: a) == .ready)
    _ = try c.run(command: .startOAuthSignIn(accountDraftId: a))
    guard case .waiting(let url) = c.oauthSignInState(accountDraftId: a) else { Issue.record("expected waiting"); return }
    #expect(url.hasPrefix(FixtureOAuthBook.authority) && url.contains("code_challenge_method=S256"), "the fake authority, never the real one")
    try c.answerOAuth(address: a, approve: true, words: "")
    #expect(c.oauthSignInState(accountDraftId: "assem@northlight-example.de") == .done)
    _ = try c.run(command: .setOAuthClient(accountDraftId: "b@x.example", provider: .microsoft, clientId: clientId))
    _ = try c.run(command: .startOAuthSignIn(accountDraftId: "b@x.example"))
    try c.answerOAuth(address: "b@x.example", approve: false, words: "AADSTS700016")
    #expect(c.oauthSignInState(accountDraftId: "b@x.example") == .refused(words: "the provider refused the sign-in: AADSTS700016"))
}

@Test func d14ASmartFolderListsItsSavedSearchAndAnArchiveTakesARowOut() throws {
    let c = try core()
    let f = try #require(c.smartFolders().first)
    let q = InboxQuery(tabId: "primary", accountId: nil, labelId: nil, smartFolderId: f.id)
    let rows = c.inbox(query: q).map(\.messageId)
    let threads = try #require(c.state(path: "smart_folder/\(f.id)/threads"))
    let listed = try JSONDecoder().decode([String].self, from: Data(threads.utf8))
    #expect(!rows.isEmpty && rows == listed, "the list's rows are the core's answer: \(rows) vs \(listed)")
    #expect(c.state(path: "smart_folder/\(f.id)/count") == "\(rows.count)")
    _ = try c.run(command: .archive(messageIds: [rows[0]]))
    #expect(c.inbox(query: q).map(\.messageId) == Array(rows.dropFirst()))
    #expect(c.state(path: "smart_folder/\(f.id)/count") == "\(rows.count - 1)", "the count follows the archive")
    #expect(c.state(path: "smart_folder/sf-none/threads") == nil)
}
