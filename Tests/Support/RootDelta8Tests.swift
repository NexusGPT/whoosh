import Foundation
import Testing
@testable import WhooshUI

/// Probes for root delta 8: add account by IMAP, the search round's ports and types, the state migration's reads,
/// smart folders and labels of a corpus, star/mute/spam/snooze state, the composer's draft, the four not-served items.
private let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func core() throws -> FixtureCore {
    try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
}
private func j2core() throws -> FixtureCore {
    let c = try core()
    try c.loadCorpus(from: root.appendingPathComponent("fixtures/tests-j2.json"))
    return c
}
private func words(_ body: () throws -> Void) -> String? {
    do { try body(); return nil } catch CoreError.Invalid(_, let w) { return w } catch CoreError.Action(let f) { return f.reasonWords } catch { return "\(error)" }
}
private func state(_ c: FixtureCore, _ path: String) -> String? { c.state(path: path) }
private final class Catch: EventSink, @unchecked Sendable {
    private let lock = NSLock()
    private var all: [CoreEvent] = []
    var events: [CoreEvent] { lock.withLock { all } }
    func onEvent(event: CoreEvent) { lock.withLock { all.append(event) } }
}
private func settle() { if !Thread.isMainThread { DispatchQueue.main.sync {} } }

// MARK: group 1, add account by IMAP

@Test func d8DetectionIsRecordedPerFixtureDomain() throws {
    let c = try core()
    // gmail: found by the provider database, OAuth2 only, "coming soon" beside the app-password route
    let g = c.detectProvider(email: "assem.fixture@gmail.com")
    guard case .found(let gc) = g.outcome else { Issue.record("gmail not found"); return }
    #expect(gc.authMethod == .oauth2 && gc.source == .ispdb && gc.imapHost == "imap.gmail.com" && g.stepReached == .ispdb)
    #expect(g.signInComingSoon == "Sign in with Google: coming soon")
    // outlook.com: oauth2 only
    guard case .found(let oc) = c.detectProvider(email: "x@outlook.com").outcome else { Issue.record("outlook"); return }
    #expect(oc.authMethod == .oauth2)
    // icloud: the IMAP login is the local part, the SMTP login the full address
    guard case .found(let ic) = c.detectProvider(email: "assem.fixture@icloud.com").outcome else { Issue.record("icloud"); return }
    #expect(ic.imapHost == "imap.mail.me.com" && ic.username == "assem.fixture" && ic.smtpUsername == "assem.fixture@icloud.com" && ic.smtpSecurity == .starttls)
    // fastmail: no provider-database entry, so detection reaches the SRV step: 465 implicit TLS
    let f = c.detectProvider(email: "me@fastmail.com")
    guard case .found(let fc) = f.outcome else { Issue.record("fastmail"); return }
    #expect(fc.source == .srv && fc.imapHost == "imap.fastmail.com" && fc.smtpHost == "smtp.fastmail.com" && fc.smtpPort == 465 && fc.smtpSecurity == .ssl)
    #expect(f.stepReached == .srv && f.progress.map(\.found) == [false, false, true])
    // the SRV-only and guess-only domains
    #expect(c.detectProvider(email: "a@srv.example.test").stepReached == .srv)
    let gu = c.detectProvider(email: "a@guess-only.example.test")
    #expect(gu.stepReached == .guess && gu.progress.count == 5)
    // a domain nobody knows: a refusal in words, every step tried
    let n = c.detectProvider(email: "a@nothing.example.test")
    guard case .refused(let w) = n.outcome else { Issue.record("nothing"); return }
    #expect(w.contains("Manual setup") && n.stepReached == .guess && n.progress.allSatisfy { !$0.found })
    // an address that is not one is refused before any step
    let bad = c.detectProvider(email: "not-an-address")
    guard case .refused = bad.outcome else { Issue.record("bad"); return }
    #expect(bad.stepReached == nil && bad.progress.isEmpty)
    // a Microsoft 365 tenant is named
    #expect(c.detectProvider(email: "a@m365.example.test").microsoft365Tenant)
}

@Test func d8DetectionSendsOneProgressEventPerStep() throws {
    let c = try core()
    let sink = Catch()
    let sub = c.subscribe(stream: .syncStatus, sink: sink)
    defer { c.unsubscribe(subscription: sub) }
    let r = c.detectProvider(email: "me@fastmail.com")
    settle()
    let steps: [(UInt32, DetectionStep)] = sink.events.compactMap { if case .detectionProgress(_, let n, let p) = $0 { return (n, p.step) } else { return nil } }
    #expect(steps.map(\.0) == [1, 2, 3] && steps.map(\.1) == r.progress.map(\.step))
    #expect(state(c, "detection/last/step") == "\"srv\"" && state(c, "detection/count") == "1" && state(c, "detection_cache/fastmail.com/source") == "\"srv\"")
}

@Test func d8ValidationNamesTheServerThatRefusedAndMapsTheFolders() throws {
    let c = try core()
    func cfg(_ email: String) -> AccountConfig { if case .found(let x) = c.detectProvider(email: email).outcome { return x }; fatalError("no config for \(email)") }
    let good = c.validateAccount(config: cfg("assem.fixture@icloud.com"), password: SecretText(value: "fixture-password"))
    #expect(good.ok && good.refusedBy == nil && good.steps.map(\.kind).contains(.imapList) && good.steps.map(\.kind).contains(.smtpAuth))
    #expect(good.roles.first { $0.role == "sent" }?.folder == "Sent Messages" && good.roles.allSatisfy(\.bySpecialUse))
    // the wrong-password case: the server named, the cause in words
    let bad = c.validateAccount(config: cfg("assem.fixture@me.com"), password: SecretText(value: "fixture-wrong-password"))
    #expect(!bad.ok && bad.refusedBy == "imap.mail.me.com" && bad.summaryWords.contains("wrong password") && bad.summaryWords.contains("https://"))
    #expect(!bad.steps.contains { $0.kind == .imapList }, "no step runs after the login was refused")
    // no SPECIAL-USE on the guess-only server: the name heuristic maps the folders; SRV names only SPECIAL-USE can map
    let guess = c.validateAccount(config: cfg("a@guess-only.example.test"), password: SecretText(value: "fixture-password"))
    #expect(guess.ok && guess.roles.allSatisfy { !$0.bySpecialUse })
    let srv = c.validateAccount(config: cfg("a@srv.example.test"), password: SecretText(value: "fixture-password"))
    #expect(srv.ok && srv.roles.first { $0.role == "sent" }?.folder == "Verzonden" && srv.roles.allSatisfy(\.bySpecialUse))
    // an M365 tenant that blocks password IMAP says so in Microsoft's words, and the report names the cause (SPEC A87)
    let blocked = c.validateAccount(config: cfg("a@blocked-m365.example.test"), password: SecretText(value: "fixture-password"))
    #expect(!blocked.ok && blocked.summaryWords.contains("password") && blocked.summaryWords.contains("Basic authentication is disabled."))
    #expect(blocked.cause == .basicAuthDisabled && bad.cause == .badPassword && good.cause == nil)
    // a login without a password is refused, not tried
    #expect(!c.validateAccount(config: cfg("assem.fixture@icloud.com"), password: SecretText(value: "")).ok)
}

@Test func d8AddAccountKeepsTheConfigAndNeverTheCleartextPassword() throws {
    let c = try core()
    guard case .found(let config) = c.detectProvider(email: "assem.fixture@icloud.com").outcome else { Issue.record("icloud"); return }
    let before = state(c, "account/count").flatMap { Int($0) } ?? 0
    // a refused password adds nothing
    let refusal = words { _ = try c.run(command: .addAccountConfigured(address: "assem.fixture@icloud.com", displayName: "iCloud", config: config, password: SecretText(value: "nope"))) }
    #expect(refusal?.contains("wrong password") == true && state(c, "account/count") == "\(before)")
    let secret = "fixture-password"
    let out = try c.run(command: .addAccountConfigured(address: " Assem.Fixture@iCloud.com ", displayName: "iCloud", config: config, password: SecretText(value: secret)))
    let id = try #require(out.createdId)
    #expect(state(c, "account/count") == "\(before + 1)")
    #expect(state(c, "account/\(id)/imap_host") == "\"imap.mail.me.com\"" && state(c, "account/\(id)/imap_port") == "993" && state(c, "account/\(id)/smtp_security") == "\"starttls\"")
    #expect(state(c, "account/\(id)/username") == "\"assem.fixture\"" && state(c, "account/\(id)/smtp_username") == "\"assem.fixture@icloud.com\"" && state(c, "account/\(id)/auth_method") == "\"password\"")
    #expect(state(c, "account/\(id)/detection_source") == "\"ispdb\"" && state(c, "account/\(id)/password_stored") == "true")
    for path in ["address", "imap_host", "username", "smtp_username", "auth_method", "detection_source", "password", "password_stored", "colour"] {
        #expect(state(c, "account/\(id)/\(path)")?.contains(secret) != true, "the password reached account/\(id)/\(path)")
    }
    #expect(state(c, "validation/last/summary")?.contains(secret) != true)
    // the same address twice is refused
    #expect(words { _ = try c.run(command: .addAccountConfigured(address: "assem.fixture@icloud.com", displayName: nil, config: config, password: SecretText(value: secret))) }?.contains("already added") == true)
}

@Test func d8TheSwiftTwinsDeriveWhatTheContractCrateDerives() {
    func step(_ k: ValidationStepKind, _ ok: Bool) -> ValidationStep { ValidationStep(kind: k, ok: ok, words: "w-\(k)") }
    #expect(ValidationReport.fromSteps(steps: [step(.imapConnect, true), step(.imapLogin, true)], roles: [], folderCount: 0, failingHost: nil).ok == false)
    #expect(ValidationReport.fromSteps(steps: [step(.imapConnect, true), step(.imapList, true)], roles: [], folderCount: 3, failingHost: "h").ok)
    let failed = ValidationReport.fromSteps(steps: [step(.imapConnect, true), step(.imapLogin, false)], roles: [], folderCount: 0, failingHost: "imap.example.test")
    #expect(!failed.ok && failed.refusedBy == "imap.example.test" && failed.summaryWords == "w-imapLogin")
    let p = [DetectionProgress(step: .ispdb, found: false, words: "a"), DetectionProgress(step: .srv, found: true, words: "b")]
    #expect(DetectionResult.make(outcome: .refused(words: "x"), progress: p, tenant: false, comingSoon: nil).stepReached == .srv)
    #expect(DetectionResult.make(outcome: .refused(words: "x"), progress: [], tenant: false, comingSoon: nil).stepReached == nil)
}

// MARK: group 2, the search round's ports and types

private func cosine(_ a: [Float], _ b: [Float]) -> Float { zip(a, b).reduce(0) { $0 + $1.0 * $1.1 } }
/// An Embedder written before `embed_query`: it implements only what existed, and still compiles through the default.
private final class DocumentOnlyEmbedder: Embedder, @unchecked Sendable {
    func dimension() -> UInt32 { 4 }
    func embed(texts: [String]) throws -> [[Float]] { texts.map { _ in [1, 0, 0, 0] } }
}

@Test func d8QueriesAndDocumentsTakeDifferentPrefixesAndMediaLandsNearItsWords() throws {
    let e = FixtureEmbedder()
    #expect(Int(e.dimension()) == WhooshConstants.EMBED_DIM)
    let doc = try #require(try e.embed(texts: ["supplier price quote"]).first)
    let query = try e.embedQuery(text: "supplier price quote")
    #expect(doc != query, "a query embedded as a document ranks worse: the prefixes must make them differ")
    #expect(cosine(doc, query) > cosine(doc, try e.embedQuery(text: "invoice payment terms")) && cosine(doc, query) > 0.3, "same words, near vectors")
    #expect(abs(cosine(doc, doc) - 1) < 0.001 && query.count == WhooshConstants.EMBED_DIM)
    let photo = try e.embedImage(path: "/Downloads/warehouse-photo.jpg")
    #expect(cosine(photo, try e.embedQuery(text: "the warehouse photo")) > cosine(photo, try e.embedQuery(text: "invoice payment terms")))
    let note = try e.embedAudio(path: "/Downloads/voice-note-delivery.m4a")
    #expect(cosine(note, try e.embedQuery(text: "delivery voice note")) > cosine(note, try e.embedQuery(text: "warehouse photo")))
    #expect(words { _ = try e.embedImage(path: "") } != nil)
    #expect(try DocumentOnlyEmbedder().embedQuery(text: "anything") == [1, 0, 0, 0], "the default embeds a query like a document")
}

@Test func d8RecordedSearchesAreNewestFirstDistinctAndCapped() throws {
    let c = try core()
    #expect(state(c, "search/recent") == "[\"\\\"0.84 EUR\\\"\",\"from Linea Supply before June has attachment\"]", "the J2.3 exact query, then the J2.2 filter text")
    #expect(words { _ = try c.run(command: .recordSearch(query: "   ")) } != nil)
    for w in ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india", "juliet"] { _ = try c.run(command: .recordSearch(query: w)) }
    #expect(state(c, "search/recent/count") == "\(WhooshConstants.SEARCH_RECENT_MAX)")
    #expect(state(c, "search/recent")?.hasPrefix("[\"juliet\",\"india\"") == true)
    // the cap drops the oldest, so the repeat must come from the middle of the list to prove it is not simply pushed out
    _ = try c.run(command: .recordSearch(query: "  FOXTROT "))
    let after = state(c, "search/recent") ?? ""
    #expect(after.hasPrefix("[\"FOXTROT\",\"juliet\"") && !after.contains("\"foxtrot\""), "the same words again move to the front once")
    #expect(state(c, "search/recent/count") == "\(WhooshConstants.SEARCH_RECENT_MAX)" && after.components(separatedBy: "\"").filter { $0.lowercased() == "foxtrot" }.count == 1)
}

@Test func d8TheSupplierQuestionFindsRecordOneFirstAndAPhotoByWhatItShows() throws {
    let c = try j2core()
    let sink = Catch()
    let sub = c.subscribe(stream: .search, sink: sink)
    defer { c.unsubscribe(subscription: sub) }
    _ = try c.run(command: .search(query: "what did suppliers quote me this spring?"))
    settle()
    let hits: [SearchHit] = sink.events.compactMap { if case .searchResults(_, let h) = $0 { return h } else { return nil } }.last ?? []
    // item 3bb3d817: a match holds EVERY typed word (search-text query.rs). No mail holds "suppliers", "quote" and "spring", so the match group is the photo
    // alone (found by what it shows, the `near` read of its file); record 1 holds some of the words and is brought by the related group, after msg-s05
    // (closeness 3.0 against record 1's 2.0 and msg-s06's 2.0, record 1 the newer; computed by _captures/every-word, not run)
    let exact = hits.filter { $0.matchedIn != .meaningOnly && $0.matchedIn != .related }.map(\.messageId)
    #expect(exact == ["msg-s02"], "the match group: \(exact)")
    let related = hits.filter { $0.matchedIn == .related }.map(\.messageId)
    #expect(related == ["msg-s05", "msg-01", "msg-s06"], "the related group: \(related)")
    #expect(hits.map(\.messageId).prefix(4) == ["msg-s02", "msg-s05", "msg-01", "msg-s06"] && c.message(messageId: "msg-01").map { $0.threadId } == "thr-01")
    let photo = try #require(hits.first { $0.messageId == "msg-s02" })
    #expect(photo.matchedIn == .attachmentContent && photo.attachmentName == "warehouse-photo.jpg")
    let meaning = try #require(hits.first { $0.messageId == "msg-s06" })
    // A150 (SPEC.md A150: "Related: the mails with none of the letters whose closeness stands clearly above the average"): a mail found by meaning alone is
    // `.related` when the test engine's cut-off (FixtureSearchBook.relatedShare) puts it above the search's mean, else `.meaningOnly`; either way no word is in it
    #expect((meaning.matchedIn == .meaningOnly || meaning.matchedIn == .related) && meaning.attachmentName == nil, "no word of the question is in it: meaning only or related")
    // SPEC A141: the search lists every mail; the lake photos are in the list after every exact match, as meaning only, never among the exact ones
    let lake = try #require(hits.firstIndex { $0.messageId == "msg-s08" }, "the lake photos are in the list")
    let byMeaning = { (h: SearchHit) in h.matchedIn == .meaningOnly || h.matchedIn == .related }   // A150: related mail is meaning alone too, listed before the rest
    #expect(byMeaning(hits[lake]) && hits.indices.filter { !byMeaning(hits[$0]) }.allSatisfy { $0 < lake })
    #expect(state(c, "search/last/hit/2/message_id") == "\"msg-01\"" && state(c, "search/last/hit/2/matched_in") == "\"related\"")
    #expect(state(c, "search/last/hit/\(hits.firstIndex { $0.messageId == "msg-s02" } ?? 99)/attachment_name") == "\"warehouse-photo.jpg\"")
    #expect(words { _ = try c.run(command: .search(query: "  ")) } != nil, "an empty search is refused in words")
    // a name of an attachment is its own kind of match
    _ = try c.run(command: .search(query: "invoice"))
    #expect(state(c, "search/last/hit/0/matched_in") == "\"subject\"" || state(c, "search/last/hit/0/matched_in") == "\"attachment_name\"")
}

@Test func d8ContactsAnswerOnePersonWithEveryAddress() throws {
    for c in [try core(), try j2core()] {
        let linea = c.contacts().filter { $0.name == "Linea Supply" }
        #expect(linea.count == 1, "one person, not one card per address")
        #expect(Set(linea.first?.addresses ?? []) == ["orders@linea-supply-example.com", "billing@linea-supply-example.com"])
        #expect(c.contacts().allSatisfy { !$0.addresses.isEmpty } && Set(c.contacts().map(\.name)).count == c.contacts().count)
    }
}

// MARK: group 4, smart folders and labels of a corpus, index count

@Test func d8ACorpusBringsItsOwnLabelsAndSmartFoldersOrNone() throws {
    let base = try core()
    let owners = { (x: FixtureCore) in x.labels().filter { $0.kind == .owner } }   // delta 9: the five built-in categories always stay
    #expect(!owners(base).isEmpty && !base.smartFolders().isEmpty)
    let c = try j2core()   // tests-j2.json lists neither
    #expect(owners(c).isEmpty && c.smartFolders().isEmpty, "a corpus that lists none leaves none")
    let corpus = FileManager.default.temporaryDirectory.appendingPathComponent("d8-corpus-\(UUID().uuidString).json")
    try #"{"schema":"whoosh-search-corpus/1","messages":[],"labels":[{"name":"Clients","children":[{"name":"Linea"}]}],"smart_folders":[{"name":"Quotes","rule":"quote"}]}"#.write(to: corpus, atomically: true, encoding: .utf8)
    defer { try? FileManager.default.removeItem(at: corpus) }
    try base.loadCorpus(from: corpus)
    #expect(owners(base).map(\.path) == ["Clients", "Clients > Linea"] && base.smartFolders().map(\.name) == ["Quotes"])
    #expect(state(base, "index/message_count") == "0")
    #expect(state(try j2core(), "index/message_count") == "11", "the J2 corpus holds eleven messages")
}

@Test func d8ASmartFolderIsUpdatedAndRefusedInWords() throws {
    let c = try core()
    let f = try #require(c.smartFolders().first)
    _ = try c.run(command: .updateSmartFolder(folderId: f.id, ruleWords: "from Linea Supply", parentId: nil))
    #expect(c.smartFolders().first?.ruleWords == "from Linea Supply" && state(c, "smart_folder/\(f.id)/rule") == "\"from Linea Supply\"")
    let other = try #require(c.smartFolders().dropFirst().first)
    _ = try c.run(command: .updateSmartFolder(folderId: f.id, ruleWords: "x y", parentId: other.id))
    #expect(state(c, "smart_folder/\(f.id)/parent") == "\"\(other.id)\"")
    #expect(words { _ = try c.run(command: .updateSmartFolder(folderId: f.id, ruleWords: "  ", parentId: nil)) } != nil)
    #expect(words { _ = try c.run(command: .updateSmartFolder(folderId: f.id, ruleWords: "x", parentId: f.id)) } != nil)
    #expect(words { _ = try c.run(command: .updateSmartFolder(folderId: "sf-nope", ruleWords: "x", parentId: nil)) } != nil)
}

// MARK: group 5, star, mute, spam, snooze

@Test func d8StarMuteSpamAndTheSnoozedList() throws {
    let c = try core()
    let ids = ["msg-01", "msg-02"]
    let before = c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).count
    _ = try c.run(command: .star(messageIds: ids, starred: true))
    #expect(state(c, "message/msg-01/starred") == "true")
    _ = try c.run(command: .star(messageIds: ["msg-01"], starred: false))
    #expect(state(c, "message/msg-01/starred") == "false" && state(c, "message/msg-02/starred") == "true")
    _ = try c.run(command: .mute(messageIds: ["msg-01"], muted: true))
    #expect(state(c, "message/msg-01/muted") == "true" && c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).count == before - 1)
    _ = try c.run(command: .mute(messageIds: ["msg-01"], muted: false))
    #expect(c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).count == before)
    _ = try c.run(command: .markSpam(messageIds: ["msg-02"]))
    #expect(state(c, "message/msg-02/spam") == "true" && c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).count == before - 1)
    _ = try c.run(command: .snooze(messageIds: ["msg-05"], when: .at(atMs: 4_000_000_000_000)))
    _ = try c.run(command: .snooze(messageIds: ["msg-04"], when: .at(atMs: 3_000_000_000_000)))
    #expect(c.snoozedMessages().map(\.messageId) == ["msg-04", "msg-05"], "soonest first")
    #expect(state(c, "snooze/msg-05/return_ms") == "4000000000000" && state(c, "snooze/msg-06/return_ms") == nil)
}

// MARK: group 6, drafts and send now

@Test func d8ADraftIsSavedAndASendNowLeavesTheWindow() throws {
    let c = try core()
    let out = try c.run(command: .saveDraft(fromAccountId: nil, to: ["a@example.test"], cc: [], subject: "Hello", body: "Body", replyToThreadId: nil))
    let id = try #require(out.createdId)
    #expect(state(c, "draft/\(id)/subject") == "\"Hello\"" && state(c, "draft/count") == "1" && state(c, "draft/\(id)/from_account_id") != nil)
    #expect(words { _ = try c.run(command: .saveDraft(fromAccountId: nil, to: [" "], cc: [], subject: "", body: "", replyToThreadId: nil)) } != nil)
    #expect(words { _ = try c.run(command: .saveDraft(fromAccountId: "acc-nope", to: ["a@b.test"], cc: [], subject: "s", body: "", replyToThreadId: nil)) } != nil)
    let sink = Catch()
    let sub = c.subscribe(stream: .outbox, sink: sink)
    defer { c.unsubscribe(subscription: sub) }
    let send = try c.run(command: .quickSend(threadId: "msg-01", body: "On my way"))
    let entry = try #require(send.createdId)
    #expect(c.outbox().count == 1)
    _ = try c.run(command: .sendNow(outboxId: entry))
    settle()
    #expect(c.outbox().isEmpty && sink.events.contains { if case .sendCompleted(let o, _) = $0 { return o == entry } else { return false } })
    #expect(words { _ = try c.run(command: .sendNow(outboxId: entry)) } != nil, "a sent message cannot be sent again")
    #expect(words { _ = try c.run(command: .sendNow(outboxId: "nope")) } != nil)
}

// MARK: group 8, the four items delta 7 did not serve

@Test func d8ACustomSplitTabIsRemovedAndItsRuleCountedBeforeSaving() throws {
    let c = try core()
    let rule = SplitTabRule.sender(address: "orders@linea-supply-example.com")
    let n = c.splitTabRuleMatchCount(rule: rule)
    #expect(n > 0 && c.splitTabRuleMatchCount(rule: .sender(address: "nobody@nowhere.test")) == 0)
    let made = try c.run(command: .addSplitTab(name: "Linea", rule: rule))
    let tab = try #require(made.createdId)
    #expect(state(c, "split_tab/\(tab)/count") == "\(n)", "the count before saving equals the tab's count after")
    let tabs = c.splitTabs().count
    _ = try c.run(command: .removeSplitTab(id: tab))
    #expect(c.splitTabs().count == tabs - 1 && !c.splitTabs().contains { $0.id == tab } && c.splitTabs().map(\.order) == Array(0..<UInt32(tabs - 1)))
    let first = try #require(c.splitTabs().first)
    #expect(words { _ = try c.run(command: .removeSplitTab(id: first.id)) } != nil, "a default tab is hidden, not removed")
    #expect(words { _ = try c.run(command: .removeSplitTab(id: tab)) } != nil)
}

@Test func d8TheMcpApprovalPromptIsAnEventAndTheAnswerIsOnce() throws {
    let c = try core()
    let app = try #require(c.mcpGrants().first)
    let sink = Catch()
    let sub = c.subscribe(stream: .agent, sink: sink)
    defer { c.unsubscribe(subscription: sub) }
    #expect(words { _ = try c.requestMcpTool(app: app.appName, tool: "send_mail", words: "send a mail") } != nil, "an app with no grant cannot ask")
    _ = try c.run(command: .setMcpGrant(appName: app.appName, allowed: true))
    let req = try c.requestMcpTool(app: app.appName, tool: "send_mail", words: "send a mail to Karen")
    settle()
    #expect(sink.events.contains { if case .mcpApprovalNeeded(let r, let a, let t, _) = $0 { return r == req && a == app.appName && t == "send_mail" } else { return false } })
    #expect(state(c, "mcp_request/\(req)/state") == "\"pending\"" && state(c, "mcp_request/pending_count") == "1")
    _ = try c.run(command: .answerMcpApproval(requestId: req, allowed: false))
    #expect(state(c, "mcp_request/\(req)/state") == "\"refused\"" && state(c, "mcp_request/pending_count") == "0")
    #expect(words { _ = try c.run(command: .answerMcpApproval(requestId: req, allowed: true)) } != nil, "a request is answered once")
}

@Test func d8NotesAreUndone() throws {
    let c = try core()
    let thread = TextRangeFreeTarget.thread
    let a = try c.run(command: .addNote(target: thread, text: "first"))
    let noteId = try #require(a.createdId)
    _ = try c.run(command: .undo)
    #expect(c.notes(target: thread).isEmpty, "undo of an add removes the note")
    let b = try c.run(command: .addNote(target: thread, text: "second"))
    let id2 = try #require(b.createdId)
    _ = try c.run(command: .editNote(noteId: id2, text: "second, edited"))
    _ = try c.run(command: .undo)
    #expect(c.notes(target: thread).first?.text == "second", "undo of an edit restores the words")
    let written = c.notes(target: thread).first?.writtenAtMs
    _ = try c.run(command: .deleteNote(noteId: id2))
    #expect(c.notes(target: thread).isEmpty)
    _ = try c.run(command: .undo)
    #expect(c.notes(target: thread).first?.id == id2 && c.notes(target: thread).first?.writtenAtMs == written, "undo of a delete brings the note back as it was")
    _ = noteId
}
private enum TextRangeFreeTarget { static let thread = NoteTarget.thread(threadId: "thr-01") }

// MARK: group 7, the second Linea Supply address

@Test func d8TheSecondLineaAddressMakesTwoLookAlikeCardsThatMerge() throws {
    let c = try j2core()
    let orders = try #require(c.contact(address: "orders@linea-supply-example.com"))
    let billing = try #require(c.contact(address: "billing@linea-supply-example.com"))
    #expect(orders.id != billing.id && orders.name == billing.name, "two cards with the same name")
    #expect(c.contactMessages(contactId: billing.id).map(\.messageId) == ["msg-s09"])
    // the J2.7 expectation for the first address is untouched
    #expect(c.contactMessages(contactId: orders.id).map(\.messageId) == ["msg-s02", "msg-01", "msg-01b", "msg-s03", "msg-s04"])
    #expect(c.contacts().filter { $0.name == "Linea Supply" }.count == 1, "the dropdown shows one person with both addresses")
    _ = try c.run(command: .mergeContacts(keepId: orders.id, mergeId: billing.id))
    #expect(c.contact(address: "billing@linea-supply-example.com")?.id == orders.id && c.contactMessages(contactId: orders.id).count == 6)
    #expect(state(c, "contact/count") == "\(c.contacts().count)", "after the merge one card is one person")
}
