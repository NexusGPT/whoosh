import Foundation
import Testing
@testable import WhooshUI

/// Probes for contracts delta 11.
private let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func core(_ clock: FixtureClock = FixtureClock()) throws -> FixtureCore {
    try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: clock)
}

@Test func d11DraftsOfAThreadComeNewestFirstWithTheirFirstLines() throws {
    let clock = FixtureClock()
    let c = try core(clock)
    let first = try #require(try c.run(command: .saveDraft(fromAccountId: nil, to: ["a@x.example"], cc: [], subject: "Re", body: "\n  Thanks Tom.\n\nNoted for 4.2.\nThird line", replyToThreadId: "msg-03")).createdId)
    clock.advance(seconds: 5)
    let second = try #require(try c.run(command: .saveDraft(fromAccountId: "acc-02", to: [], cc: [], subject: "", body: "One line", replyToThreadId: "msg-03")).createdId)
    _ = try c.run(command: .saveDraft(fromAccountId: nil, to: ["b@x.example"], cc: [], subject: "elsewhere", body: "x", replyToThreadId: "msg-05"))
    let d = c.drafts(threadId: "msg-03")
    #expect(d.map(\.id) == [second, first])
    #expect(d[1].firstLines == "Thanks Tom.\nNoted for 4.2.")
    #expect(d[0].accountId == "acc-02" && d.allSatisfy { $0.threadId == "msg-03" })
    #expect(d[0].savedAt > d[1].savedAt)
    #expect(c.drafts(threadId: "msg-01").isEmpty)
}

@Test func d11DeletingADraftIsUndoneByZ() throws {
    let c = try core()
    let id = try #require(try c.run(command: .saveDraft(fromAccountId: nil, to: ["a@x.example"], cc: [], subject: "Re", body: "Keep me", replyToThreadId: "msg-03")).createdId)
    let out = try c.run(command: .deleteDraft(draftId: id))
    #expect(out.undoEntryId != nil)
    #expect(c.drafts(threadId: "msg-03").isEmpty)
    _ = try c.run(command: .undo)
    #expect(c.drafts(threadId: "msg-03").map(\.id) == [id])
    #expect(c.drafts(threadId: "msg-03").first?.firstLines == "Keep me")
    #expect(throws: CoreError.self) { try c.run(command: .deleteDraft(draftId: "no-such-draft")) }
}

@Test func d11RenamePauseAndSignAnAccount() throws {
    let c = try core()
    _ = try c.run(command: .renameAccount(accountId: "acc-02", name: "  Home  "))
    _ = try c.run(command: .pauseAccount(accountId: "acc-02", paused: true))
    _ = try c.run(command: .setSignature(accountId: "acc-02", text: "Assem\nsent from home"))
    let a = try #require(c.accounts().first { $0.id == "acc-02" })
    #expect(a.name == "Home" && a.paused && a.signature == "Assem\nsent from home")
    #expect(c.signature(address: a.address)?.text == "Assem\nsent from home")
    _ = try c.run(command: .setSignature(accountId: "acc-02", text: ""))
    #expect(c.accounts().first { $0.id == "acc-02" }?.signature == "", "an empty text clears the signature")
    _ = try c.run(command: .pauseAccount(accountId: "acc-02", paused: false))
    #expect(c.accounts().first { $0.id == "acc-02" }?.paused == false)
    #expect(throws: CoreError.self) { try c.run(command: .renameAccount(accountId: "acc-02", name: "   ")) }
    #expect(throws: CoreError.self) { try c.run(command: .pauseAccount(accountId: "acc-99", paused: true)) }
}

@Test func d11RemovingAnAccountSaysWhatGoesAndTakesIt() throws {
    let c = try core()
    let rows = c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil))
    let acc = try #require(rows.first { !$0.pinned && $0.accountId != "acc-18" }?.accountId)
    _ = try c.run(command: .saveDraft(fromAccountId: acc, to: ["a@x.example"], cc: [], subject: "s", body: "b", replyToThreadId: nil))
    let r = try #require(c.accountRemoval(accountId: acc))
    #expect(r.messages >= 1 && r.drafts == 1)
    #expect(r.words.hasPrefix("Removes ") && r.words.hasSuffix("Nothing is deleted on the server."))
    let out = try c.run(command: .removeAccount(accountId: acc))
    #expect(out.createdId == acc && out.undoEntryId == nil)
    #expect(!c.accounts().contains { $0.id == acc })
    #expect(!c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).contains { $0.accountId == acc && !$0.pinned })
    #expect(c.accountRemoval(accountId: acc) == nil)
}

@Test func d11UndoOfADeleteRestoresTheArchivedMarkAndSpam() throws {
    let c = try core()
    let rows = c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).filter { !$0.pinned && $0.accountId != "acc-18" }
    let (a, b, d) = (rows[0].messageId, rows[1].messageId, rows[2].messageId)
    _ = try c.run(command: .archive(messageIds: [a]))
    _ = try c.run(command: .markSpam(messageIds: [b]))
    _ = try c.run(command: .deleteMessages(messageIds: [a, b, d]))
    _ = try c.run(command: .archive(messageIds: [d]))     // a later change the undo must undo only for the delete's own fields
    _ = try c.run(command: .undo)                         // undoes the archive of d
    _ = try c.run(command: .undo)                         // undoes the delete
    #expect(c.state(path: "message/\(a)/archived") == "true" && c.state(path: "message/\(a)/deleted") == "false")
    #expect(c.state(path: "message/\(b)/spam") == "true")
    #expect(c.state(path: "message/\(d)/archived") == "false" && c.state(path: "message/\(d)/deleted") == "false")
}

@Test func d11ALabelTheOwnerRemovedIsNeverFiledBackByARule() throws {
    let c = try core()
    let target = try #require(c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).first { !$0.pinned && $0.accountId != "acc-18" })
    let from = try #require(c.message(messageId: target.messageId)?.fromAddress)
    let domain = String(from.split(separator: "@").last ?? "")
    let spec = LabelSpec(name: "From them", parentId: nil, colour: nil, description: "",
                         scope: .all, rules: RuleSet(conditions: [.domain(domain: domain)], matchMode: .any), pinned: false)
    let id = try #require(try c.run(command: .createLabelFull(spec: spec)).createdId)
    _ = try c.run(command: .rerunLabel(labelId: id))
    #expect(c.state(path: "label_assignment/\(target.messageId)").map { $0.contains("From them") } == true, "the rule files it")
    _ = try c.run(command: .removeLabel(messageIds: [target.messageId], labelId: id))
    #expect(c.state(path: "label_removed/\(target.messageId)/\(id)") == "true")
    _ = try c.run(command: .rerunLabel(labelId: id))
    #expect(c.state(path: "label_assignment/\(target.messageId)").map { !$0.contains("From them") } == true, "the rule does not file it back")
    _ = try c.run(command: .label(messageIds: [target.messageId], labelName: "From them", createIfMissing: false))
    #expect(c.state(path: "label_removed/\(target.messageId)/\(id)") == "false", "the owner's own label lifts the removal")
}

@Test func d11UpdatingAReopenedDraftKeepsItsIdAndPushesNoUndo() throws {
    let clock = FixtureClock()
    let c = try core(clock)
    let a = try #require(try c.run(command: .saveDraft(fromAccountId: nil, to: ["a@x.example"], cc: [], subject: "Re", body: "First words", replyToThreadId: "msg-03")).createdId)
    let b = try #require(try c.run(command: .saveDraft(fromAccountId: nil, to: ["b@x.example"], cc: [], subject: "Re", body: "Other", replyToThreadId: "msg-03")).createdId)
    _ = try c.run(command: .deleteDraft(draftId: b))
    let savedBefore = try #require(c.drafts(threadId: "msg-03").first).savedAt
    clock.advance(seconds: 5)
    let out = try c.run(command: .updateDraft(draftId: a, fromAccountId: "acc-02", to: ["a@x.example"], cc: [], bcc: [], subject: "Re", body: "Second words"))
    #expect(out.undoEntryId == nil && out.createdId == a)
    let d = c.drafts(threadId: "msg-03")
    #expect(d.map(\.id) == [a], "one draft, same id, no second copy")
    #expect(d.first?.firstLines == "Second words" && d.first?.accountId == "acc-02")
    #expect(try #require(d.first).savedAt > savedBefore)
    // Z still reverses the delete, the last undoable act: the update put nothing on the undo list
    _ = try c.run(command: .undo)
    #expect(Set(c.drafts(threadId: "msg-03").map(\.id)) == [a, b])
    #expect(c.drafts(threadId: "msg-03").first { $0.id == a }?.firstLines == "Second words")
    #expect(throws: CoreError.self) { try c.run(command: .updateDraft(draftId: "no-such-draft", fromAccountId: nil, to: ["x@x.example"], cc: [], bcc: [], subject: "s", body: "b")) }
    #expect(throws: CoreError.self) { try c.run(command: .updateDraft(draftId: a, fromAccountId: nil, to: [" "], cc: [], bcc: [], subject: " ", body: "")) }
    #expect(c.drafts(threadId: "msg-03").first { $0.id == a }?.firstLines == "Second words", "a refused update leaves the draft as it was")
}

@Test func d11AnAccountsColourIconAndNotificationsAreKeptAndReadBack() throws {
    let c = try core()
    let img = FileManager.default.temporaryDirectory.appendingPathComponent("d11-icon-\(UUID().uuidString).png")
    try Data(count: 2048).write(to: img)
    defer { try? FileManager.default.removeItem(at: img) }
    func acc() throws -> AccountSummary { try #require(c.accounts().first { $0.id == "acc-01" }) }
    let glyphBefore = try acc().iconId
    _ = try c.run(command: .setAccountColour(accountId: "acc-01", colour: "#2b8a3e"))
    _ = try c.run(command: .setAccountIcon(accountId: "acc-01", source: .library(iconId: "rocket")))
    #expect(glyphBefore != "rocket")
    #expect(try acc().colour == "#2B8A3E" && acc().iconId == "rocket" && acc().iconFile == nil)
    // an upload is read back as a path; the glyph underneath stays
    _ = try c.run(command: .setAccountIcon(accountId: "acc-01", source: .uploaded(filePath: img.path)))
    #expect(try acc().iconFile == img.path && acc().iconId == "rocket")
    // back to the library: the image goes, the new glyph shows
    _ = try c.run(command: .setAccountIcon(accountId: "acc-01", source: .library(iconId: "star")))
    #expect(try acc().iconFile == nil && acc().iconId == "star" && glyphBefore != "star")
    // notifications: off and back on, the chosen delivery kept, read through notificationSettings
    _ = try c.run(command: .setNotificationSetting(setting: NotificationSetting(accountId: "acc-01", enabled: true, delivery: .notch)))
    _ = try c.run(command: .setNotification(accountId: "acc-01", enabled: false))
    var n = try #require(c.notificationSettings().first { $0.accountId == "acc-01" })
    #expect(n.enabled == false && n.delivery == .notch)
    _ = try c.run(command: .setNotification(accountId: "acc-01", enabled: true))
    n = try #require(c.notificationSettings().first { $0.accountId == "acc-01" })
    #expect(n.enabled && n.delivery == .notch)
    #expect(c.notificationSettings().first { $0.accountId == "acc-02" }?.enabled == true, "another account is untouched")
}
