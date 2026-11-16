import Foundation
import Testing
@testable import WhooshUI

/// Probes for root delta 6: label commands, smart folders and rule counts, contacts, one message, delete.
private let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func core() throws -> FixtureCore {
    try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
}
private func words(_ body: () throws -> Void) -> String? {
    do { try body(); return nil } catch CoreError.Invalid(_, let w) { return w } catch { return "\(error)" }
}
private func label(_ c: FixtureCore, _ path: String) -> MailLabel? { c.labels().first { $0.path == path } }

// MARK: labels

@Test func createLabelFollowsTheDepthRule() throws {
    let c = try core()
    let q4 = try #require(label(c, "Work > Clients > Provanta > RFP 2026 > Q4"))
    let w = words { _ = try c.run(command: .createLabel(name: "Too deep", parentId: q4.id)) }
    #expect(w?.contains("at most 5 levels deep") == true && w?.contains("has 6") == true)
    let rfp = try #require(label(c, "Work > Clients > Provanta > RFP 2026"))
    let out = try c.run(command: .createLabel(name: "Q1", parentId: rfp.id))
    let made = try #require(c.labels().first { $0.id == out.createdId })
    #expect(made.path == "Work > Clients > Provanta > RFP 2026 > Q1" && made.depth == 5 && made.parentId == rfp.id)
    let top = try c.run(command: .createLabel(name: "Archive 2026", parentId: nil))
    #expect(c.labels().first { $0.id == top.createdId }?.depth == 1)
    #expect(words { _ = try c.run(command: .createLabel(name: "q1", parentId: rfp.id)) }?.contains("exists already") == true)
    #expect(words { _ = try c.run(command: .createLabel(name: "A > B", parentId: nil)) } != nil)
}

@Test func renameLabelCarriesTheWholeSubtree() throws {
    let c = try core()
    let prox = try #require(label(c, "Work > Clients > Provanta"))
    _ = try c.run(command: .renameLabel(labelId: prox.id, name: "Provanta Global"))
    #expect(label(c, "Work > Clients > Provanta Global > RFP 2026 > Q4")?.depth == 5)
    #expect(label(c, "Work > Clients > Provanta") == nil)
    let nord = try #require(label(c, "Work > Clients > Nordlicht"))
    #expect(words { _ = try c.run(command: .renameLabel(labelId: nord.id, name: "lhg")) }?.contains("exists already") == true)
    #expect(words { _ = try c.run(command: .renameLabel(labelId: nord.id, name: "  ")) } != nil)
}

@Test func moveLabelKeepsTheDepthLimitAndTheTreeOrder() throws {
    let c = try core()
    let clients = try #require(label(c, "Work > Clients"))
    let family = try #require(label(c, "Personal > Family"))
    let w = words { _ = try c.run(command: .moveLabel(labelId: clients.id, parentId: family.id)) }
    #expect(w?.contains("at most 5 levels deep") == true && w?.contains("has 6") == true)
    #expect(label(c, "Work > Clients")?.id == clients.id)
    let prox = try #require(label(c, "Work > Clients > Provanta"))
    #expect(words { _ = try c.run(command: .moveLabel(labelId: prox.id, parentId: prox.id)) }?.contains("inside itself") == true)
    _ = try c.run(command: .moveLabel(labelId: prox.id, parentId: nil))
    #expect(label(c, "Provanta > RFP 2026 > Q4")?.depth == 3)
    let rows = c.labels()
    for (i, r) in rows.enumerated() { if let p = r.parentId { #expect(rows[..<i].contains { $0.id == p }) } }   // a parent before its children
}

// MARK: delete

@Test func deleteMessageLeavesTheListAndTheCount() throws {
    let c = try core()
    let before = c.counts().tabs.first { $0.tabId == "primary" }?.unread ?? 0
    #expect(c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).contains { $0.messageId == "msg-04" })
    _ = try c.run(command: .deleteMessage(messageId: "msg-04"))
    #expect(!c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).contains { $0.messageId == "msg-04" })
    #expect(c.state(path: "message/msg-04/deleted") == "true")
    #expect((c.counts().tabs.first { $0.tabId == "primary" }?.unread ?? 0) == before - 1)
    #expect(c.message(messageId: "msg-04")?.folder == "Trash")
    #expect(throws: CoreError.self) { try c.run(command: .deleteMessage(messageId: "msg-none")) }
}

// MARK: smart folders

@Test func saveSmartFolderRefusesADuplicateAndServesItsRule() throws {
    let c = try core()
    #expect(words { _ = try c.run(command: .saveSmartFolder(name: "Supplier quotes", ruleWords: "quote")) } == "the name exists already")
    #expect(words { _ = try c.run(command: .saveSmartFolder(name: " supplier QUOTES ", ruleWords: "quote")) } == "the name exists already")
    let out = try c.run(command: .saveSmartFolder(name: "Q1. quotes", ruleWords: "quote"))
    #expect(out.createdId == "sf-4" && c.smartFolders().count == 4)
    #expect(c.state(path: "smart_folder/Q1. quotes/rule") == "\"quote\"")
    #expect(c.state(path: "smart_folder/Supplier quotes/rule") == "\"attachments from suppliers mentioning a quote\"")
    #expect(c.state(path: "smart_folder/count") == "4")
}

@Test func ruleMatchCountReadsTheRows() throws {
    let c = try core()
    #expect(c.ruleMatchCount(ruleWords: "quote") == 1)
    #expect(c.ruleMatchCount(ruleWords: "invoice") >= 1)
    #expect(c.ruleMatchCount(ruleWords: "zzqxj") == 0)
    #expect(c.ruleMatchCount(ruleWords: "  of a ") == 0)
}

// MARK: contacts

@Test func aContactCardListsItsMailNewestFirstAndMerges() throws {
    let c = try core()
    let linea = try #require(c.contact(address: "Orders@Linea-Supply-Example.com"))
    #expect(linea.name == "Linea Supply" && linea.mailCountPerAccount == ["acc-04": 1])
    let mine = c.contactMessages(contactId: linea.id)
    #expect(mine.map(\.messageId) == ["msg-01"] && mine.first?.accountId == "acc-04" && mine.first?.senderAddress == "orders@linea-supply-example.com")
    let marie = try #require(c.contact(address: "marie@merel-atelier-example.fr"))
    _ = try c.run(command: .mergeContacts(keepId: linea.id, mergeId: marie.id))
    let merged = try #require(c.contact(address: "marie@merel-atelier-example.fr"))
    #expect(merged.id == linea.id && merged.addresses.count == 2 && merged.mailCountPerAccount == ["acc-04": 1, "acc-05": 1])
    let rows = c.contactMessages(contactId: linea.id)
    #expect(rows.count == 2 && rows[0].receivedAtMs >= rows[1].receivedAtMs)
    #expect(Set(rows.map(\.accountId)) == ["acc-04", "acc-05"])
    #expect(c.contactMessages(contactId: marie.id).isEmpty)
    #expect(c.contact(address: "nobody@example.org") == nil)
}

// MARK: one message

@Test func rowsCarryTheSenderAddress() throws {
    let rows = try core().inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil))
    #expect(rows.count == (try FixtureFileFigures.primaryRows()) && rows.allSatisfy { $0.senderAddress.contains("@") })
    #expect(rows.first { $0.messageId == "msg-05" }?.senderAddress == "nina@northlight-example.de")
}

@Test func messageReadsFromNameAddressTimeAttachmentsCodeAndTrackers() throws {
    let c = try core()
    let m = try #require(c.message(messageId: "msg-01"))
    #expect(m.senderName == "Linea Supply" && m.fromAddress == "orders@linea-supply-example.com" && m.receivedAtMs > 0)
    #expect(m.attachmentNames == ["quote-Q1-print-run.pdf"] && m.loginCode == nil && m.trackersBlocked == 0 && m.accountId == "acc-04")
    #expect(c.message(messageId: "msg-09")?.loginCode == "482913")
    #expect(c.message(messageId: "msg-04")?.trackersBlocked == 2)
    #expect(c.message(messageId: "msg-none") == nil)
}

@Test func theOpenThreadServesRealMessageIds() throws {
    let c = try core()
    let t = try #require(c.thread(threadId: "thr-01"))
    #expect(t.messageIds == ["thr-01-m1", "thr-01-m2", "msg-01"])
    let all = t.messageIds.compactMap { c.message(messageId: $0) }
    #expect(all.count == 3 && all.allSatisfy { $0.receivedAtMs > 0 && $0.threadId == "thr-01" || $0.id == "msg-01" })
    #expect(all[0].fromAddress == "assem@northlight-example.de" && all[1].fromAddress == "orders@linea-supply-example.com")
}

@Test func rowsCarryWhoPlacedThem() throws {
    let rows = try core().inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil))
    func by(_ id: String) -> DecidedBy? { rows.first { $0.messageId == id }?.decidedBy }
    #expect(by("msg-02") == .laya && by("msg-03") == .owner && by("msg-06") == .jev && by("msg-15") == .rule(ruleId: "Newsletters"))
    #expect(by("msg-01") == nil)
}
