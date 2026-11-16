import Foundation
import Testing
@testable import WhooshUI

/// Probes for root delta 9: labels as the tabs, rules that target labels with the re-run, suggestions and blocks, the mailbox
/// selector, the media port. Each test names the rule it holds; _captures/contracts/mutate-d9.out records the mutant that turns it red.
private let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func core() throws -> FixtureCore {
    try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
}
private func words(_ body: () throws -> Void) -> String? {
    do { try body(); return nil } catch CoreError.Invalid(_, let w) { return w } catch CoreError.Action(let f) { return f.reasonWords } catch { return "\(error)" }
}
private func state(_ c: FixtureCore, _ path: String) -> String? { c.state(path: path) }
private func labelId(_ c: FixtureCore, _ path: String) -> String { c.labels().first { $0.path == path }?.id ?? "missing:\(path)" }
private func primary(_ c: FixtureCore, mailbox: Mailbox? = nil) -> [InboxRow] { c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil, mailbox: mailbox)) }
private func spec(_ name: String, parent: String? = nil, pinned: Bool = false, rules: RuleSet? = nil, description: String = "") -> LabelSpec {
    LabelSpec(name: name, parentId: parent, colour: "#2F6FED", description: description, scope: .all,
              rules: rules ?? RuleSet(conditions: [], matchMode: .any), pinned: pinned)
}

// MARK: group 1, the label record

@Test func d9TheFiveCategoriesAreBuiltInLabelsAndTheFirstFourAreTheTabs() throws {
    let c = try core()
    let all = c.labels()
    // delta 16: Primary is a view of the inbox, not a label; the built-in labels are the four other categories
    #expect(all.prefix(4).map(\.name) == ["Needs Reply", "Updates", "Promotions", "Social"])
    #expect(all.prefix(4).allSatisfy { $0.kind == .builtIn && $0.parentId == nil })
    #expect(!all.contains { inboxViewFromId(id: $0.id) != nil }, "no view is served as a label")
    #expect(c.pinnedLabels().map(\.name) == ["Needs Reply", "Updates", "Promotions", "Provanta"], "the fixture pins Provanta; Social is not pinned")
    #expect(state(c, "labels/pinned").map { $0.contains("\"Promotions\"") && !$0.contains("\"Social\"") } == true)
    #expect(state(c, "label/social/pinned") == "false" && state(c, "label/needs-reply/kind") == "\"built_in\"" && state(c, "label/primary/kind") == nil)
    // the fixture's own labels follow the built-ins in the tree and keep their ids
    #expect(all.first { $0.name == "Work" }?.id == "lbl-1")
}

@Test func d9TheFixturesLabelsCarryTheirMeaningColourScopeAndRules() throws {
    let c = try core()
    let prox = labelId(c, "Work > Clients > Provanta"), sup = labelId(c, "Work > Suppliers"), inv = labelId(c, "Finance > Invoices")
    #expect(state(c, "label/\(prox)/colour") == "\"#2F6FED\"" && state(c, "label/\(prox)/pinned") == "true")
    #expect(state(c, "label/\(prox)/description")?.contains("Provanta account") == true)
    #expect(state(c, "label/\(inv)/scope") == "\"account:acc-02\"" && state(c, "label/\(sup)/scope") == "\"all\"")
    #expect(state(c, "label/\(sup)/rules")?.contains("\"linea-supply-example.com\"") == true)
    #expect(c.labels().first { $0.id == inv }?.accountIds == ["acc-02"])
}

@Test func d9ABuiltInCannotBeRenamedOrDeletedAndKeepsItsMeaningEditable() throws {
    let c = try core()
    // delta 16: Primary is a view, refused at the view door before the built-in guard; the guard is probed on Needs Reply
    #expect(words { _ = try c.run(command: .renameLabel(labelId: "primary", name: "Main")) }?.contains("view of the inbox") == true)
    #expect(words { _ = try c.run(command: .renameLabel(labelId: "needs-reply", name: "Main")) }?.contains("built-in") == true, "the older rename command is refused too")
    let rename = words { _ = try c.run(command: .updateLabel(labelId: "needs-reply", spec: spec("Main", pinned: true))) }
    #expect(rename?.contains("built-in") == true && rename?.contains("renamed") == true)
    let del = words { _ = try c.run(command: .deleteLabel(labelId: "needs-reply")) }
    #expect(del?.contains("cannot be deleted") == true && del?.contains("Unpin") == true)
    _ = try c.run(command: .updateLabel(labelId: "needs-reply", spec: spec("Needs Reply", pinned: true, description: "mail I must answer first")))
    #expect(state(c, "label/needs-reply/description") == "\"mail I must answer first\"")
    let inside = words { _ = try c.run(command: .reorderLabel(labelId: "needs-reply", newParentId: "lbl-1", newIndex: 0)) }
    #expect(inside?.contains("top level") == true)
}

@Test func d9ASheetCreatesALabelAndAnUpdateReplacesEveryFieldSoClearingWorks() throws {
    let c = try core()
    let rules = RuleSet(conditions: [.domain(domain: "linea-supply-example.com")], matchMode: .any)
    let id = try c.run(command: .createLabelFull(spec: spec("Quotes", parent: labelId(c, "Work"), pinned: true, rules: rules, description: "supplier quotes"))).createdId!
    #expect(c.labels().first { $0.id == id }?.path == "Work > Quotes")
    #expect(state(c, "label/\(id)/pinned") == "true" && state(c, "label/\(id)/rule_sets") == "1")
    // the clear path: no colour, no meaning, no rules, unpinned are VALUES
    var clear = spec("Quotes", parent: labelId(c, "Work"), pinned: false)
    clear.colour = nil
    _ = try c.run(command: .updateLabel(labelId: id, spec: clear))
    #expect(state(c, "label/\(id)/colour") == nil && state(c, "label/\(id)/description") == "\"\"")
    #expect(state(c, "label/\(id)/rules") == nil && state(c, "label/\(id)/pinned") == "false")
    #expect(c.pinnedLabels().allSatisfy { $0.id != id })
    // refused in words
    var bad = spec("Bad"); bad.colour = "red"
    #expect(words { _ = try c.run(command: .createLabelFull(spec: bad)) }?.contains("#RRGGBB") == true)
    let dup = words { _ = try c.run(command: .createLabelFull(spec: spec("quotes", parent: labelId(c, "Work")))) }
    #expect(dup?.contains("already a label named") == true)
}

@Test func d9ADragSetsTheTabOrderAndRefusesALoopAndTheLastPinCannotGo() throws {
    let c = try core()
    let prox = labelId(c, "Work > Clients > Provanta")
    _ = try c.run(command: .reorderLabel(labelId: prox, newParentId: nil, newIndex: 1))
    #expect(c.pinnedLabels().map(\.name) == ["Needs Reply", "Provanta", "Updates", "Promotions"], "the saved order is the tab order")
    #expect(c.labels().first { $0.id == prox }?.path == "Provanta")
    #expect(c.splitTabs().filter(\.enabled).map(\.id).prefix(2) == ["needs-reply", prox], "the older split-tab read follows the pins")
    let work = labelId(c, "Work"), clients = labelId(c, "Work > Clients")
    #expect(words { _ = try c.run(command: .reorderLabel(labelId: work, newParentId: clients, newIndex: 0)) }?.contains("inside itself") == true)
    #expect(c.labels().first { $0.id == clients }?.path == "Work > Clients", "a refused drag changes nothing")
    for l in c.pinnedLabels().dropLast() { _ = try c.run(command: .pinLabel(labelId: l.id, pinned: false)) }
    let last = c.pinnedLabels()
    #expect(last.count == 1)
    #expect(words { _ = try c.run(command: .pinLabel(labelId: last[0].id, pinned: false)) }?.contains("at least one label") == true)
}

// MARK: group 2, rules target labels, the live count and the re-run

private let lineaRules = #"{"match":"any","conditions":[{"kind":"domain","domain":"linea-supply-example.com"}]}"#

@Test func d9TheLiveCountOfARuleSetIsReadOverTheCorpus() throws {
    let c = try core()
    #expect(state(c, "rules/\(lineaRules)/count") == "1" && state(c, "rules/\(lineaRules)/unread") == "1")
    let two = #"{"match":"any","conditions":[{"kind":"domain","domain":"linea-supply-example.com"},{"kind":"subject","words":"invoice"}]}"#
    #expect(state(c, "rules/\(two)/count") == "2")
    let all = #"{"match":"all","conditions":[{"kind":"domain","domain":"linea-supply-example.com"},{"kind":"subject","words":"invoice"}]}"#
    #expect(state(c, "rules/\(all)/count") == "0", "all needs every condition")
    let empty = #"{"match":"all","conditions":[]}"#
    #expect(state(c, "rules/\(empty)/count") == "0", "an empty rule set matches nothing")
    #expect(c.ruleSetCount(ruleSet: RuleSet(conditions: [.hasAttachment], matchMode: .any)).count > 0)
}

@Test func d9AReRunFilesByTheRulesKeepsTheOwnersFilingsAndMarksTheModelPart() throws {
    let c = try core()
    let rules = RuleSet(conditions: [.domain(domain: "linea-supply-example.com")], matchMode: .any)
    let id = try c.run(command: .createLabelFull(spec: spec("Linea", rules: rules, description: "everything from Linea Supply"))).createdId!
    #expect(c.labelRerunCount(labelId: id) == 0 && state(c, "labels/\(id)/rerun_count") == "0")
    // the owner filed an unrelated message by hand: a re-run never takes it away
    _ = try c.run(command: .label(messageIds: ["msg-02"], labelName: "Linea", createIfMissing: false))
    _ = try c.run(command: .rerunLabel(labelId: id))
    // delta 13: under --fixture-core the fixture model speaks in the re-run, so the meaning part is no longer pending
    #expect(state(c, "label/\(id)/rerun/refiled") == "1" && state(c, "label/\(id)/rerun/model_pending") == "false")
    #expect(state(c, "message/msg-01/label/\(id)/decided_by") == "\"rule:rule-\(id)\"", "a filing by the rules says which rule")
    #expect(state(c, "message/msg-02/label/\(id)/decided_by") == "\"owner\"")
    #expect(c.labelRerunCount(labelId: id) == 2)
    // the rule changes to another domain: the re-run unfiles the old match, files the new one, keeps the owner's
    let newRules = RuleSet(conditions: [.domain(domain: "provanta-example.be")], matchMode: .any)
    _ = try c.run(command: .updateLabel(labelId: id, spec: spec("Linea", rules: newRules, description: "")))
    _ = try c.run(command: .rerunLabel(labelId: id))
    #expect(state(c, "label/\(id)/rerun/unfiled") == "1" && state(c, "label/\(id)/rerun/refiled") == "1" && state(c, "label/\(id)/rerun/model_pending") == "false")
    #expect(state(c, "message/msg-01/label/\(id)/decided_by") == nil, "msg-01 left")
    #expect(state(c, "message/msg-03/label/\(id)/decided_by") == "\"rule:rule-\(id)\"")
    #expect(c.labelRerunCount(labelId: id) == 2)
    // a label with no rules: a re-run changes nothing
    let bare = try c.run(command: .createLabelFull(spec: spec("Bare"))).createdId!
    _ = try c.run(command: .label(messageIds: ["msg-04"], labelName: "Bare", createIfMissing: false))
    _ = try c.run(command: .rerunLabel(labelId: bare))
    #expect(state(c, "label/\(bare)/rerun/unfiled") == "0" && c.labelRerunCount(labelId: bare) == 1)
}

@Test func d9ReFilingABuiltInCategoryMovesTheMessageAndItsUnreadBetweenTabs() throws {
    let c = try core()
    let before = Dictionary(uniqueKeysWithValues: c.counts().tabs.map { ($0.tabId, $0.unread) })
    _ = try c.run(command: .updateLabel(labelId: "updates", spec: LabelSpec(name: "Updates", parentId: nil, colour: nil, description: "", scope: .all,
        rules: RuleSet(conditions: [.domain(domain: "hostcloud-example.net")], matchMode: .any), pinned: true)))
    _ = try c.run(command: .rerunLabel(labelId: "updates"))
    let after = Dictionary(uniqueKeysWithValues: c.counts().tabs.map { ($0.tabId, $0.unread) })
    #expect(state(c, "message/msg-04/category") == "\"updates\"")
    #expect(after["updates"] == (before["updates"] ?? 0) + 1 && after["primary"] == (before["primary"] ?? 0) - 1, "unread moved from Primary to Updates, once")
    #expect(c.inbox(query: InboxQuery(tabId: "updates", accountId: nil, labelId: nil, mailbox: nil)).map(\.messageId).contains("msg-04"))
    #expect(!primary(c).map(\.messageId).contains("msg-04"))
    #expect(c.inbox(query: InboxQuery(tabId: "updates", accountId: nil, labelId: nil, mailbox: nil)).first { $0.messageId == "msg-04" }?.decidedBy == .rule(ruleId: "rule-updates"))
}

// MARK: group 3, tabs are pinned labels

@Test func d9TheTabReadReturnsThePinnedLabelsInTreeOrderWithCountsThatRollUpTheirSubLabels() throws {
    let c = try core()
    #expect(c.counts().tabs.map(\.tabId) == ["primary", "all", "needs-reply", "updates", "promotions", labelId(c, "Work > Clients > Provanta")], "delta 16: the two views, then the tabs")
    let prox = labelId(c, "Work > Clients > Provanta"), q4 = labelId(c, "Work > Clients > Provanta > RFP 2026 > Q4")
    let start = c.counts().tabs.first { $0.tabId == prox }!.unread
    _ = try c.run(command: .label(messageIds: ["msg-05"], labelName: "Q4", createIfMissing: false))
    #expect(c.counts().tabs.first { $0.tabId == prox }!.unread == start + 1, "a message under a sub-label counts on the parent tab, once")
    #expect(c.inbox(query: InboxQuery(tabId: prox, accountId: nil, labelId: nil, mailbox: nil)).map(\.messageId) == ["msg-05"])
    _ = try c.run(command: .label(messageIds: ["msg-05"], labelName: "RFP 2026", createIfMissing: false))
    #expect(c.counts().tabs.first { $0.tabId == prox }!.unread == start + 1, "two labels under one tab still count the message once")
    _ = q4
    // Social unpinned becomes a tab by pinning, in its tree place
    _ = try c.run(command: .pinLabel(labelId: "social", pinned: true))
    #expect(c.counts().tabs.map(\.tabId).prefix(6) == ["primary", "all", "needs-reply", "updates", "promotions", "social"])
}

@Test func d9TheOlderSplitTabCommandsMoveThePinsAndRestoreGoesBackToTheDefaults() throws {
    let c = try core()
    _ = try c.run(command: .setSplitTabEnabled(id: "promotions", on: false))
    #expect(state(c, "label/promotions/pinned") == "false" && !c.pinnedLabels().map(\.id).contains("promotions"))
    _ = try c.run(command: .reorderSplitTabs(ids: ["updates"] + c.splitTabs().map(\.id).filter { $0 != "updates" }))
    #expect(c.pinnedLabels().first?.id == "updates")
    _ = try c.run(command: .restoreDefaultSplitTabs)
    #expect(c.pinnedLabels().map(\.id) == ["needs-reply", "updates", "promotions", "lbl-6"], "restore returns to the pins the file started with: the three pinned categories and the fixture's Provanta")
    _ = try c.run(command: .pinLabel(labelId: "lbl-2", pinned: true))
    _ = try c.run(command: .restoreDefaultSplitTabs)
    #expect(!c.pinnedLabels().map(\.id).contains("lbl-2"), "a tab the owner pinned since is unpinned by the restore")
}

@Test func d9ACorporaOwnLabelsAndPinsAreAuthoritative() throws {
    let c = try core()
    let file = FileManager.default.temporaryDirectory.appendingPathComponent("d9-corpus-\(UUID().uuidString).json")
    defer { try? FileManager.default.removeItem(at: file) }
    let json = """
    {"schema":"whoosh-search-corpus/1","messages":[{"id":"x1","account":"acc-01","from_name":"A","from":"a@x.example","date":"2026-04-16T09:12:00","subject":"s","body":"b"}],
     "labels":[{"name":"Clients","children":[{"name":"Acme","pinned":true}]},{"name":"Misc"}],"pinned_labels":["updates","Misc"]}
    """
    try json.write(to: file, atomically: true, encoding: .utf8)
    try c.loadCorpus(from: file)
    #expect(c.labels().prefix(4).allSatisfy { $0.kind == .builtIn }, "the categories survive a corpus")
    #expect(c.labels().dropFirst(4).map(\.path) == ["Clients", "Clients > Acme", "Misc"])
    #expect(c.pinnedLabels().map(\.name) == ["Updates", "Misc"], "the corpus' list decides what is a tab: Acme's own flag and the default four are overruled")
    #expect(state(c, "label/updates/pinned") == "true" && state(c, "label/needs-reply/pinned") == "false")
}

// MARK: group 4, suggestions and blocks

@Test func d9JunkOffersABlockAndAcceptingWritesItOnce() throws {
    let c = try core()
    #expect(c.suggestions().isEmpty && c.blocks().isEmpty)
    let out = try c.run(command: .markSpam(messageIds: ["msg-01"]))
    let sid = try #require(out.createdId)
    let s = try #require(c.suggestions().first)
    #expect(s.id == sid && s.kind == .blockSender && s.subject == "orders@linea-supply-example.com" && !s.accepted)
    #expect(state(c, "suggestion/\(sid)/words") == "\"Block orders@linea-supply-example.com\"" && state(c, "suggestion/count") == "1")
    _ = try c.run(command: .acceptSuggestion(suggestionId: sid))
    #expect(c.suggestions().isEmpty && state(c, "suggestion/\(sid)/accepted") == "true")
    #expect(c.blocks() == [Block(target: .sender(address: "orders@linea-supply-example.com"), sinceMs: c.blocks()[0].sinceMs)])
    #expect(state(c, "block/orders@linea-supply-example.com/blocked") == "true" && state(c, "block/someone@else.example/blocked") == "false")
    #expect(words { _ = try c.run(command: .acceptSuggestion(suggestionId: sid)) }?.contains("already accepted") == true)
    // junk from a blocked sender offers nothing more
    #expect(try c.run(command: .markSpam(messageIds: ["msg-01"])).createdId == nil)
}

@Test func d9ADismissedSuggestionWritesNothingAndBlocksAreIdempotentAndChecked() throws {
    let c = try core()
    let sid = try #require(try c.run(command: .markSpam(messageIds: ["msg-04"])).createdId)
    _ = try c.run(command: .dismissSuggestion(suggestionId: sid))
    #expect(c.suggestions().isEmpty && c.blocks().isEmpty)
    #expect(words { _ = try c.run(command: .dismissSuggestion(suggestionId: sid)) } != nil)
    _ = try c.run(command: .blockSender(address: "Spam@Bad.Example"))
    _ = try c.run(command: .blockSender(address: "spam@bad.example"))
    _ = try c.run(command: .blockDomain(domain: "@News.Example"))
    _ = try c.run(command: .blockDomain(domain: "news.example"))
    #expect(state(c, "block/count") == "2", "a second block of the same target changes nothing")
    #expect(state(c, "block/a@sub.news.example/blocked") == "true" && state(c, "block/a@notnews.example/blocked") == "false")
    #expect(words { _ = try c.run(command: .blockSender(address: "not-an-address")) }?.contains("full address") == true)
    #expect(words { _ = try c.run(command: .blockDomain(domain: "nodot")) }?.contains("example.com") == true)
}

@Test func d9AcceptingAnAlwaysLabelSuggestionAddsTheSenderToTheLabelsRules() throws {
    let c = try core()
    let lab = try c.run(command: .createLabelFull(spec: spec("Suppliers 2", rules: RuleSet(conditions: [.hasAttachment], matchMode: .all)))).createdId!
    let sug = Suggestion(id: "x", kind: .alwaysLabel, subject: "orders@linea-supply-example.com", labelId: lab, offeredAtMs: 1, accepted: false)
    #expect(try suggestionEffect(suggestion: sug) == .labelRule(address: "orders@linea-supply-example.com", labelId: lab))
    #expect(words { _ = try suggestionEffect(suggestion: Suggestion(id: "y", kind: .alwaysLabel, subject: "a@b.c", labelId: nil, offeredAtMs: 1, accepted: false)) } != nil)
}

// MARK: group 5, the mailbox selector

@Test func d9EveryMailboxServesRowsAndCountsAndTheOutboxListsWhatWaits() throws {
    let c = try core()
    let all: [Mailbox] = [.outbox, .vips, .pinned, .snoozed, .drafts, .sent, .junk, .trash, .archive]
    for m in all { _ = primary(c, mailbox: m) }
    #expect(primary(c, mailbox: .pinned).allSatisfy(\.pinned) && !primary(c, mailbox: .pinned).isEmpty)
    #expect(primary(c, mailbox: .outbox).isEmpty && state(c, "mailbox/outbox/count") == "0")
    _ = try c.run(command: .quickSend(threadId: "thr-01", body: "Thanks, will do."))
    let waiting = primary(c, mailbox: .outbox)
    #expect(waiting.count == 1 && waiting[0].preview == "Thanks, will do." && c.counts().mailboxes[mailboxKey(mailbox: .outbox)] == 1)
    _ = try c.run(command: .archive(messageIds: ["msg-02"]))
    _ = try c.run(command: .markSpam(messageIds: ["msg-07"]))
    _ = try c.run(command: .deleteMessage(messageId: "msg-04"))
    #expect(primary(c, mailbox: .archive).map(\.messageId) == ["msg-02"] && c.counts().mailboxes["archive"] == 1)
    #expect(primary(c, mailbox: .junk).map(\.messageId) == ["msg-07"] && c.counts().mailboxes["junk"] == 1)
    #expect(primary(c, mailbox: .trash).map(\.messageId) == ["msg-04"] && c.counts().mailboxes["trash"] == 1)
    _ = try c.run(command: .snooze(messageIds: ["msg-05"], when: .tomorrowMorning))
    _ = try c.run(command: .snooze(messageIds: ["msg-06"], when: .laterToday))
    #expect(primary(c, mailbox: .snoozed).map(\.messageId) == ["msg-06", "msg-05"], "soonest return first")
    // the inbox list no longer holds them, and a mailbox read ignores the tab
    #expect(!primary(c).map(\.messageId).contains("msg-02"))
    #expect(c.inbox(query: InboxQuery(tabId: "nonsense", accountId: nil, labelId: nil, mailbox: .archive)).count == 1)
    #expect(mailboxKey(mailbox: .outbox) == "outbox" && mailboxKey(mailbox: .vips) == "vips")
}

// MARK: group 6, the media port

private final class TextOnly: Embedder, @unchecked Sendable {
    func dimension() -> UInt32 { 4 }
    func embed(texts: [String]) throws -> [[Float]] { texts.map { _ in [1, 0, 0, 0] } }
    func embedQuery(text: String) throws -> [Float] { [1, 0, 0, 0] }
}

@Test func d9AnEmbedderEmbedsAFileOfEveryKindAndATextOnlyOneRefusesInWords() throws {
    let e = FixtureEmbedder()
    for kind in [MediaKind.image, .audio, .videoFrame] {
        let v = try e.embedMedia(kind: kind, path: "/tmp/warehouse-photo.jpg")
        #expect(v.count == WhooshConstants.EMBED_DIM)
        #expect(abs(v.reduce(0) { $0 + $1 * $1 } - 1) < 0.001, "unit length")
    }
    #expect(try e.embedMedia(kind: .image, path: "/tmp/warehouse-photo.jpg") == (try e.embedImage(path: "/tmp/warehouse-photo.jpg")), "the new entry and the delta 8 entry agree")
    #expect(words { _ = try e.embedMedia(kind: .image, path: "") } != nil)
    #expect(words { _ = try TextOnly().embedMedia(kind: .audio, path: "/x.m4a") }?.contains("text only") == true)
    let content = AttachmentContent(attachmentId: "a1", text: nil, ocrText: nil, labels: [], transcript: nil, embedded: true, updatedAtMs: 1)
    #expect(content.vectorKey == nil, "a writer from before delta 9 keeps compiling and the key is absent")
    #expect(AttachmentContent(attachmentId: "a1", text: nil, ocrText: nil, labels: [], transcript: nil, embedded: true, vectorKey: "msg-02/a1", updatedAtMs: 1).vectorKey == "msg-02/a1")
}

// MARK: group 8, fixture insert doors

@Test func d9AMutedMessageArrivesMutedAndStaysOutOfTheInboxAndTheCounts() throws {
    let c = try core()
    let before = c.counts().tabs.first { $0.tabId == "primary" }!.unread
    try c.insertMutedMessage(id: "ins-m1", account: "acc-04", from: "loud@list.example", sender: "Loud List", subject: "Weekly digest", body: "news")
    #expect(state(c, "message/ins-m1/muted") == "true")
    #expect(!primary(c).map(\.messageId).contains("ins-m1") && c.counts().tabs.first { $0.tabId == "primary" }!.unread == before)
    #expect(c.message(messageId: "ins-m1")?.subject == "Weekly digest")
    #expect(words { try c.insertMutedMessage(id: "ins-m1", account: "acc-04", from: "a@b.c", sender: "x", subject: "s", body: "b") } != nil, "an id is used once")
    #expect(words { try c.insertMutedMessage(id: "ins-m2", account: "acc-99", from: "a@b.c", sender: "x", subject: "s", body: "b") } != nil)
}

@Test func d9AnArchiveTheServerRefusesThrowsItsReasonAndTheMessageStays() throws {
    let c = try core()
    try c.insertArchiveRefusedMessage(id: "ins-a1", account: "acc-04", from: "boss@work.example", sender: "Boss", subject: "Keep this", body: "b", reasonWords: "the server keeps messages under legal hold")
    #expect(primary(c).first { !$0.pinned }?.messageId == "ins-a1", "it arrives at the top of the unpinned rows")
    let reason = words { _ = try c.run(command: .archive(messageIds: ["ins-a1"])) }
    #expect(reason == "the server keeps messages under legal hold")
    #expect(state(c, "message/ins-a1/archived") == "false" && primary(c).map(\.messageId).contains("ins-a1"))
    _ = try c.run(command: .archive(messageIds: ["msg-02"]))   // other messages archive as before
    #expect(state(c, "message/msg-02/archived") == "true")
}

@Test func d9ASendThatFailedSitsInTheOutboxInTheFailedStateWithItsReason() throws {
    let c = try core()
    let id = try c.insertFailedSend(to: ["tom@provanta-example.be"], subject: "Re: Clarification", body: "Thanks.", reasonWords: "the mail server closed the connection")
    #expect(state(c, "outbox/\(id)/state") == "\"failed\"" && state(c, "outbox/\(id)/reason") == "\"the mail server closed the connection\"" && state(c, "outbox/\(id)/attempts") == "1")
    let e = try #require(c.outbox().first { $0.id == id })
    if case .failed(let f, _) = e.state { #expect(f.retryable) } else { Issue.record("not failed") }
    #expect(primary(c, mailbox: .outbox).map(\.messageId) == [e.draft.id], "a failed send still waits in the Outbox")
    #expect(words { _ = try c.insertFailedSend(account: "acc-99", to: [], subject: "", body: "", reasonWords: "x") } != nil)
}

// MARK: group 9, reply all and forward

@Test func d9ReplyAllAddressesTheSenderAndEveryRecipientButTheOwner() throws {
    let c = try core()
    let id = try #require(try c.run(command: .replyAll(messageId: "msg-01")).createdId)
    #expect(state(c, "draft/\(id)/to") == "[\"orders@linea-supply-example.com\",\"karen@linea-supply-example.com\"]", "the sender, then the others; the owner's address is gone")
    #expect(state(c, "draft/\(id)/cc") == "[\"finance@linea-supply-example.com\"]", "the owner's cc, in capitals, is gone too")
    let subject = state(c, "draft/\(id)/subject") ?? ""
    #expect(subject.hasPrefix("\"Re: Quote for the Q1") == true)
    #expect((state(c, "draft/\(id)/body") ?? "").contains("wrote:\\n> "))
    #expect(state(c, "draft/\(id)/reply_to_thread_id") != nil)
    #expect(words { _ = try c.run(command: .replyAll(messageId: "nope")) } != nil)
    // a message with no other recipients: only the sender
    let solo = try #require(try c.run(command: .replyAll(messageId: "msg-02")).createdId)
    #expect(state(c, "draft/\(solo)/to") == "[\"marie@merel-atelier-example.fr\"]" && state(c, "draft/\(solo)/cc") == "[]")
}

@Test func d9ForwardOpensADraftWithNoRecipientsAndTheOriginalWithItsAttachmentsListed() throws {
    let c = try core()
    let id = try #require(try c.run(command: .forward(messageId: "msg-01")).createdId)
    #expect(state(c, "draft/\(id)/to") == "[]" && state(c, "draft/\(id)/reply_to_thread_id") == nil)
    #expect((state(c, "draft/\(id)/subject") ?? "").hasPrefix("\"Fwd: Quote for the Q1"))
    let body = state(c, "draft/\(id)/body") ?? ""
    #expect(body.contains("Forwarded message") && body.contains("Attachments: quote-Q1-print-run.pdf") && body.contains("orders@linea-supply-example.com"))
    #expect(state(c, "draft/count").map { Int($0) ?? 0 } ?? 0 >= 1)
}

// MARK: smart folders' menu

@Test func d9ASmartFolderIsRenamedAndDeletedAndItsChildrenMoveUp() throws {
    let c = try core()
    let f = c.smartFolders()
    _ = try c.run(command: .renameSmartFolder(folderId: f[0].id, name: "  Quotes  "))
    #expect(c.smartFolders()[0].name == "Quotes" && c.smartFolders()[0].ruleWords == f[0].ruleWords)
    #expect(words { _ = try c.run(command: .renameSmartFolder(folderId: f[1].id, name: "quotes")) }?.contains("already") == true)
    #expect(words { _ = try c.run(command: .renameSmartFolder(folderId: f[1].id, name: " ")) }?.contains("needs a name") == true)
    _ = try c.run(command: .updateSmartFolder(folderId: f[2].id, ruleWords: "bookings", parentId: f[1].id))
    _ = try c.run(command: .deleteSmartFolder(folderId: f[1].id))
    #expect(c.smartFolders().map(\.id) == [f[0].id, f[2].id] && c.smartFolders().last?.parentId == nil, "the child moves up to the deleted folder's parent")
    #expect(words { _ = try c.run(command: .deleteSmartFolder(folderId: f[1].id)) } != nil)
}

@Test func d9DeletingALabelDropsItsFilingsAndKeepsTheMessages() throws {
    let c = try core()
    let sup = labelId(c, "Work > Suppliers")
    _ = try c.run(command: .label(messageIds: ["msg-01"], labelName: "Suppliers", createIfMissing: false))
    _ = try c.run(command: .deleteLabel(labelId: sup))
    #expect(!c.labels().contains { $0.id == sup } && c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil, mailbox: nil)).first { $0.messageId == "msg-01" }?.labelNames == [], "the message stays and loses the label; nothing moves to the parent")
    // rename and colour go through UpdateLabel
    let prox = labelId(c, "Work > Clients > Provanta")
    var s = spec("Provanta Group", parent: labelId(c, "Work > Clients"), pinned: true); s.colour = "#112233"
    _ = try c.run(command: .updateLabel(labelId: prox, spec: s))
    #expect(c.labels().first { $0.id == prox }?.path == "Work > Clients > Provanta Group" && state(c, "label/\(prox)/colour") == "\"#112233\"")
    #expect(c.labels().first { $0.name == "RFP 2026" }?.path == "Work > Clients > Provanta Group > RFP 2026", "the sub-labels' paths follow a rename")
}
