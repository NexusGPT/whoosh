import Foundation
import Testing
@testable import WhooshUI

/// Probes for root delta 7: Ask through the core, settings setters and state, Decisions and MCP, Welcome state,
/// undo cases, split tabs, signature and reply-to, notes.
private let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func core() throws -> FixtureCore {
    try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
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
/// Events reach a sink through the main queue when the caller is not on the main thread; wait for them.
private func settle() { if !Thread.isMainThread { DispatchQueue.main.sync {} } }
private let question = "what did suppliers quote me this spring?"

// MARK: group 1, Ask through the core

/// A core with a key for the default provider and the consent given, the way the Ask panel leaves it after "Allow".
private func readyToAsk() throws -> FixtureCore {
    let c = try core()
    c.agents.credentialStored.insert("anthropic")
    try c.giveConsent(providerId: "anthropic")
    return c
}

@Test func d7AskStopsForConsentBeforeAnyMailIsRead() throws {
    let c = try core()
    c.agents.credentialStored.insert("anthropic")
    let sink = Catch()
    let sub = c.subscribe(stream: .agent, sink: sink)
    defer { c.unsubscribe(subscription: sub) }
    let out = try c.ask(question)
    settle()
    let conv = try #require(out.createdId)
    #expect(c.answer(conversationId: conv) == nil && state(c, "agent_conversation/last/steps") == "0")
    #expect(state(c, "agent_consent/anthropic") == "false" && state(c, "agent_conversation/last/awaiting_consent") == "true")
    let asked = sink.events.contains { if case .agentConsentNeeded(_, let p, _) = $0 { return p == "anthropic" } else { return false } }
    let read = sink.events.contains { if case .agentStep = $0 { return true } else { return false } }
    #expect(asked && !read)
    try c.giveConsent(providerId: "anthropic")
    _ = try c.ask(question)
    #expect(state(c, "agent_consent/anthropic") == "true" && state(c, "agent_conversation/last/awaiting_consent") == "false")
    #expect(c.answer(conversationId: conv) != nil)
}

@Test func d7ConsentIsAskedOncePerProvider() throws {
    let c = try readyToAsk()
    let sink = Catch()
    let sub = c.subscribe(stream: .agent, sink: sink)
    defer { c.unsubscribe(subscription: sub) }
    _ = try c.ask(question); _ = try c.ask(question)
    settle()
    #expect(!sink.events.contains { if case .agentConsentNeeded = $0 { return true } else { return false } })
}

@Test func d7EveryCitationNamesAccountSenderAndDate() throws {
    let c = try readyToAsk()
    let conv = try #require(try c.ask(question).createdId)
    let a = try #require(c.answer(conversationId: conv))
    #expect(a.kind == .answer && a.citations.map(\.messageId) == ["msg-01", "msg-10"])
    for cite in a.citations {
        let m = try #require(c.message(messageId: cite.messageId))
        #expect(cite.accountId == m.accountId && !cite.accountId.isEmpty && cite.senderName == m.senderName && cite.receivedAtMs == m.receivedAtMs && cite.receivedAtMs > 0 && !cite.quote.isEmpty)
    }
    #expect(a.text.contains("Linea Supply"))
}

@Test func d7TheRunStaysWithinTheStepAndContextLimitsAndStreamsItsText() throws {
    let c = try readyToAsk()
    let sink = Catch()
    let sub = c.subscribe(stream: .agent, sink: sink)
    defer { c.unsubscribe(subscription: sub) }
    let conv = try #require(try c.ask(question).createdId)
    settle()
    let steps = try #require(Int(state(c, "agent_conversation/last/steps") ?? ""))
    #expect(steps >= 1 && steps <= WhooshConstants.AGENT_MAX_STEPS)
    #expect(try #require(c.answer(conversationId: conv)).citations.count <= WhooshConstants.AGENT_CONTEXT_CHUNKS)
    let streamed = sink.events.compactMap { e -> String? in if case .agentDelta(_, let t) = e { return t } else { return nil } }
    #expect(streamed.joined() == c.answer(conversationId: conv)?.text && Int(state(c, "agent_conversation/last/chunks") ?? "") == streamed.count)
    let numbers = sink.events.compactMap { e -> UInt32? in if case .agentStep(_, let n, _) = e { return n } else { return nil } }
    #expect(numbers == Array(1...UInt32(steps)))
    #expect(state(c, "agent_conversation/last/turns") == "2")
}

@Test func d7NothingFoundIsSaidInWordsWithNoCitation() throws {
    let c = try readyToAsk()
    let conv = try #require(try c.ask("what about zebra crossings?").createdId)
    let a = try #require(c.answer(conversationId: conv))
    #expect(a.text == "I found nothing in your mail about that." && a.citations.isEmpty)
}

@Test func d7ALabelProposalChangesNothingUntilApprovedAndAppliesOnce() throws {
    let c = try readyToAsk()
    let conv = try #require(try c.ask(question).createdId)
    _ = try c.ask("label these as Supplier quotes")
    let p = try #require(c.proposals(conversationId: conv).last)
    #expect(p.messageIds == ["msg-01", "msg-10"] && p.label == "Supplier quotes" && p.decision == .pending && p.draftId == nil)
    #expect(state(c, "agent_proposal/last/state") == "\"pending\"" && state(c, "label_assignment/Supplier quotes/count") == "0")
    #expect(c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).allSatisfy { $0.labelNames.isEmpty })
    try c.approveProposal(p.id)
    #expect(state(c, "agent_proposal/last/state") == "\"approved\"" && state(c, "label_assignment/Supplier quotes/count") == "2")
    #expect(words { _ = try c.approveProposal(p.id) }?.contains("decided already") == true)
    #expect(state(c, "label_assignment/Supplier quotes/count") == "2")
}

@Test func d7ADismissedProposalLabelsNothing() throws {
    let c = try readyToAsk()
    _ = try c.ask(question); _ = try c.ask("label these as Supplier quotes")
    let id = try #require(state(c, "agent_proposal/last/id")).trimmingCharacters(in: CharacterSet(charactersIn: "\""))
    try c.dismissProposal(id)
    #expect(state(c, "agent_proposal/last/state") == "\"rejected\"" && state(c, "label_assignment/Supplier quotes/count") == "0")
}

@Test func d7AnAgentWithoutTheLabelActionIsNeverOfferedALabel() throws {
    let c = try readyToAsk()
    #expect(c.setAgentAllowed(words: "readMail,search"))
    let conv = try #require(try c.ask(question).createdId)
    _ = try c.ask("label these as Supplier quotes")
    #expect(c.proposals(conversationId: conv).isEmpty)
    #expect(c.answer(conversationId: conv)?.text.contains("not allowed") == true)
    #expect(!c.setAgentAllowed(words: "readMail,fly"))
}

@Test func d7AFailingProviderSaysWhyInWords() throws {
    let c = try readyToAsk()
    c.setProviderFails("quota")
    #expect(words { _ = try c.ask(question) } == "The provider says your quota is used up. Switch agent or add credit.")
    c.setProviderFails(nil)
    #expect(words { _ = try c.ask(question) } == nil)
}

@Test func d7CitingTheOpenedMessageIsQuoted() throws {
    let c = try readyToAsk()
    _ = try c.ask(question)
    #expect(state(c, "agent_conversation/last/quoted_message") == nil)
    _ = try c.run(command: .openThread(threadId: "msg-01"))
    #expect(state(c, "agent_conversation/last/quoted_message") == "\"msg-01\"")
}

@Test func d7AskWithoutAKeyNamesTheSettingsPage() throws {
    let c = try core()
    try c.giveConsent(providerId: "anthropic")
    #expect(words { _ = try c.ask(question) }?.contains("Settings: AI providers") == true)
}

// MARK: group 2, settings setters and state

private func jsonList(_ s: String?) -> [String] { (s.flatMap { try? JSONSerialization.jsonObject(with: Data($0.utf8)) } as? [String]) ?? [] }

@Test func d7NotchSettingsSaveAndShowInState() throws {
    let c = try core()
    #expect(state(c, "notch_setting/hidden_in_screen_share") == "true" && state(c, "notch_setting/follow_focus") == "false" && state(c, "notch_setting/show") == "\"latest\"")
    var n = c.notchSetting()
    n.hiddenInScreenShare = false; n.followFocus = true; n.listShows = .unread; n.buttons = ["delete", "archive", "archive"]
    _ = try c.run(command: .setNotchSetting(setting: n))
    #expect(state(c, "notch_setting/hidden_in_screen_share") == "false" && state(c, "notch_setting/follow_focus") == "true" && state(c, "notch_setting/show") == "\"unread\"")
    #expect(jsonList(state(c, "notch_setting/buttons")) == ["delete", "archive"])
    n.buttons = []
    _ = try c.run(command: .setNotchSetting(setting: n))
    #expect(state(c, "notch_setting/buttons") == "\"none\"")
}

@Test func d7ANotchButtonOutsideTheSetIsRefusedInWords() throws {
    let c = try core()
    var n = c.notchSetting(); n.buttons = ["archive", "explode"]
    #expect(words { _ = try c.run(command: .setNotchSetting(setting: n)) }?.contains("explode") == true)
    #expect(jsonList(state(c, "notch_setting/buttons")) == ["mark-read", "archive", "reply"])
}

@Test func d7ARefusedSaveLeavesTheSettingAndNamesTheReason() throws {
    let c = try core()
    #expect(c.failNext("notch-setting-save", reason: "the preferences file is locked"))
    var n = c.notchSetting(); n.followFocus = true
    #expect(words { _ = try c.run(command: .setNotchSetting(setting: n)) } == "the preferences file is locked")
    #expect(state(c, "notch_setting/follow_focus") == "false")
    _ = try c.run(command: .setNotchSetting(setting: n))      // the refusal was one-shot
    #expect(state(c, "notch_setting/follow_focus") == "true")
    #expect(!c.failNext("weather-save", reason: "x"))
    #expect(c.failNext("notification-setting-save", reason: "iCloud is not reachable"))
    #expect(words { _ = try c.run(command: .setNotification(accountId: "acc-05", enabled: false)) } == "iCloud is not reachable")
    #expect(state(c, "notification_setting/acc-05/enabled") == "true")
}

@Test func d7MutingAnAccountShowsInTheOffList() throws {
    let c = try core()
    #expect(state(c, "notification_setting/acc-05/enabled") == "true" && jsonList(state(c, "notification_setting/off")) == [])
    _ = try c.run(command: .setNotification(accountId: "acc-05", enabled: false))
    _ = try c.run(command: .setNotificationSetting(setting: NotificationSetting(accountId: "acc-02", enabled: false, delivery: .notch)))
    #expect(state(c, "notification_setting/acc-05/enabled") == "false" && jsonList(state(c, "notification_setting/off")) == ["acc-02", "acc-05"])
    #expect(state(c, "notification_setting/acc-02/delivery") == "\"notch\"")
    _ = try c.run(command: .setNotification(accountId: "acc-02", enabled: true))
    #expect(state(c, "notification_setting/acc-02/delivery") == "\"notch\"" && c.notificationSettings().count == 30)
    #expect(words { _ = try c.run(command: .setNotification(accountId: "acc-99", enabled: false)) } != nil)
}

@Test func d7AccountIconAndColourReachTheListsAndState() throws {
    let c = try core()
    _ = try c.run(command: .setAccountIcon(accountId: "acc-01", source: .library(iconId: "briefcase")))
    _ = try c.run(command: .setAccountColour(accountId: "acc-01", colour: "#2b8a3e"))
    let a = try #require(c.accounts().first { $0.id == "acc-01" })
    #expect(a.iconId == "briefcase" && a.colour == "#2B8A3E")
    #expect(state(c, "account_icon/acc-01") == "\"briefcase\"" && state(c, "account/acc-01/colour") == "\"#2B8A3E\"")
    #expect(words { _ = try c.run(command: .setAccountColour(accountId: "acc-01", colour: "green")) }?.contains("#RRGGBB") == true)
    #expect(state(c, "account/acc-01/colour") == "\"#2B8A3E\"")
}

@Test func d7AnImageOverTheLimitIsRefusedAndTheIconStays() throws {
    let c = try core()
    let big = FileManager.default.temporaryDirectory.appendingPathComponent("d7-big-\(UUID().uuidString).png")
    let ok = FileManager.default.temporaryDirectory.appendingPathComponent("d7-ok-\(UUID().uuidString).png")
    try Data(count: WhooshConstants.ACCOUNT_ICON_MAX_BYTES + 1).write(to: big)
    try Data(count: 2048).write(to: ok)
    defer { try? FileManager.default.removeItem(at: big); try? FileManager.default.removeItem(at: ok) }
    let before = state(c, "account_icon/acc-01")
    #expect(words { _ = try c.run(command: .setAccountIcon(accountId: "acc-01", source: .uploaded(filePath: big.path))) }?.contains("too big") == true)
    #expect(state(c, "account_icon/acc-01") == before)
    #expect(words { _ = try c.run(command: .setAccountIcon(accountId: "acc-01", source: .uploaded(filePath: "/nonexistent/x.png"))) }?.contains("could not be read") == true)
    _ = try c.run(command: .setAccountIcon(accountId: "acc-01", source: .uploaded(filePath: ok.path)))
    #expect(state(c, "account_icon/acc-01") == "\"uploaded\"")
}

@Test func d7SyncStateCredentialAndLinkPayloadAreReadable() throws {
    let c = try core()
    #expect(state(c, "sync_state/acc-18/healthy") == "false" && state(c, "sync_state/acc-01/healthy") == "true")
    #expect(c.reauth(accountId: "acc-18"))
    #expect(state(c, "sync_state/acc-18/healthy") == "true")
    #expect(state(c, "account_credential/acc-18/stored") == "true" && state(c, "account_credential/acc-99/stored") == nil)
    let added = try #require(try c.run(command: .addAccount(address: "t31@example.org", provider: .imap)).createdId)
    #expect(state(c, "account_credential/\(added)/stored") == "true" && state(c, "sync_state/\(added)/healthy") == "true")
    #expect(jsonList(state(c, "device_link/payload_keys")) == ["token"])
}

// MARK: group 3, Decisions settings and MCP

@Test func d7AProviderKeyIsStoredAndNeverEchoed() throws {
    let c = try core()
    let key = "sk-test-anthropic-0001"
    #expect(state(c, "provider_credential/anthropic/stored") == "false" && state(c, "provider_credential/local/stored") == nil)
    #expect(words { _ = try c.run(command: .saveProviderKey(providerId: "anthropic", key: SecretText(value: "bad key"))) }?.contains("did not accept") == true)
    #expect(state(c, "provider_credential/anthropic/stored") == "false")
    _ = try c.run(command: .saveProviderKey(providerId: "anthropic", key: SecretText(value: key)))
    #expect(state(c, "provider_credential/anthropic/stored") == "true" && state(c, "ai_provider/anthropic/key") == nil)
    let p = try #require(c.aiProviders().first { $0.id == "anthropic" })
    #expect(p.connected && p.keychain != nil)
    let everything = ["provider_credential/anthropic/stored", "ai_provider/anthropic/key", "ai_provider/anthropic/connected", "decision_model/key", "agent_conversation/last/id"]
    #expect(everything.allSatisfy { !(state(c, $0) ?? "").contains(key) })
    #expect(!"\(p)".contains(key))
    try c.giveConsent(providerId: "anthropic")
    #expect(words { _ = try c.ask(question) } == nil)        // the saved key is what lets Ask run
}

@Test func d7AnAgentIsSavedWithOnlyTheActionsItWasGiven() throws {
    let c = try core()
    let config = AgentConfig(id: "", name: "Supplier desk", purpose: "Supplier mail", providerId: "anthropic", model: "claude-sonnet-5-5", instructions: "", allowed: [.readMail, .search, .label], builtIn: true)
    let made = try #require(try c.run(command: .saveAgentConfig(config: config)).createdId)
    #expect(jsonList(state(c, "agent_config/Supplier desk/allowed")) == ["readMail", "search", "label"])
    #expect(c.agentConfigs().first { $0.id == made }?.builtIn == false)
    #expect(words { _ = try c.run(command: .saveAgentConfig(config: config)) }?.contains("exists already") == true)
    var noProvider = config; noProvider.name = "Other"; noProvider.providerId = "nope"
    #expect(words { _ = try c.run(command: .saveAgentConfig(config: noProvider)) }?.contains("provider") == true)
}

@Test func d7TheDecisionModelNeedsItsKeyAndPreviewsBeforeItApplies() throws {
    let c = try core()
    let jev = DecisionModel(provider: .jev, model: nil, keychain: nil, decisionsEnabled: ["which-tab", "vips"], questionWords: [:], lastRunMs: nil)
    #expect(words { _ = try c.run(command: .saveDecisionModel(model: jev)) }?.contains("Jev key") == true)
    #expect(state(c, "decision_model/provider") == nil)
    _ = try c.run(command: .saveProviderKey(providerId: "jev", key: SecretText(value: "tsa-test-jev-key-0001")))
    _ = try c.run(command: .saveDecisionModel(model: jev))
    #expect(state(c, "decision_model/provider") == "\"jev\"" && state(c, "decision_model/key") == nil && c.decisionModel()?.decisionsEnabled == ["which-tab", "vips"])
    #expect(words { _ = try c.run(command: .applyDecisionPreview) }?.contains("Run the decision model") == true)
    let before = c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).map(\.decidedBy)
    _ = try c.run(command: .previewDecisions)
    let verdicts = c.decisionPreview()
    #expect(verdicts.count == (try FixtureFileFigures.decisionPreviewCount(limit: WhooshConstants.DECISION_PREVIEW_N)) && verdicts.contains { $0.differs } && state(c, "decision_preview/applied") == "false")
    #expect(c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).map(\.decidedBy) == before)
    #expect(c.decisionModel()?.lastRunMs != nil)
    _ = try c.run(command: .applyDecisionPreview)
    #expect(state(c, "decision_preview/applied") == "true")
    let moved = try #require(verdicts.first { $0.differs })
    #expect(c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).first { $0.messageId == moved.messageId }?.decidedBy == .jev)
}

@Test func d7TheSpeechEngineReportsReadyOrFailsInWords() throws {
    let c = try core()
    #expect(state(c, "speech_engine/chosen/state") == nil)
    #expect(c.failNext("speech-download", reason: "The download stopped: the network dropped."))
    #expect(words { _ = try c.run(command: .chooseSpeechEngine(kind: .local)) } == "The download stopped: the network dropped.")
    #expect(state(c, "speech_engine/chosen/state") == "\"failed\"" && c.speechEngine().ready == false)
    _ = try c.run(command: .chooseSpeechEngine(kind: .local))
    #expect(state(c, "speech_engine/chosen/state") == "\"ready\"" && c.speechEngine().ready)
    #expect(words { _ = try c.run(command: .chooseSpeechEngine(kind: .cloud(providerId: "openai"))) }?.contains("openai key") == true)
}

@Test func d7AnAppThatIsNotAllowedHasNoGrant() throws {
    let c = try core()
    #expect(state(c, "mcp_grant/Claude Desktop") == "false" && state(c, "mcp_server/on") == "true" && state(c, "mcp_grant/Unknown App") == nil)
    _ = try c.run(command: .setMcpGrant(appName: "Claude Desktop", allowed: true))
    #expect(state(c, "mcp_grant/Claude Desktop") == "true")
    let g = try #require(c.mcpGrants().first { $0.appName == "Claude Desktop" })
    #expect(g.allowed && g.grantedAtMs != nil && c.mcpGrants().filter(\.allowed).count == 1)
    _ = try c.run(command: .setMcpGrant(appName: "Claude Desktop", allowed: false))
    #expect(state(c, "mcp_grant/Claude Desktop") == "false" && c.mcpGrants().allSatisfy { !$0.allowed && $0.grantedAtMs == nil })
    #expect(words { _ = try c.run(command: .setMcpGrant(appName: "Nope", allowed: true)) } != nil)
    _ = try c.run(command: .setMcpServer(on: false))
    #expect(state(c, "mcp_server/on") == "false")
}

// MARK: group 4, Welcome state

@Test func d7AnAddedAccountHasACredentialAHealthySyncAndAFirstScreen() throws {
    let c = try core()
    let id = try #require(try c.run(command: .addAccount(address: "t31@example.org", provider: .imap)).createdId)
    #expect(state(c, "account_credential/\(id)/stored") == "true" && state(c, "account/\(id)/password_stored") == "true")
    #expect(state(c, "account/\(id)/password") == nil)             // the password itself is never a state
    #expect(state(c, "sync_state/\(id)/healthy") == "true" && state(c, "sync_state/\(id)/history_fetched") == "false")
    #expect(state(c, "sync_state/\(id)/first_screen_at") != nil && state(c, "sync_state/\(id)/status") == "\"fetching the inbox\"")
    #expect(state(c, "sync_state/acc-18/first_screen_at") == nil && state(c, "sync_state/acc-18/error") == "\"needs you to sign in again\"")
    #expect(c.syncStates().count == 31 && c.syncStates().last?.accountId == id)
}

@Test func d7ThePresetAndTheOrderAreTheOwners() throws {
    let c = try core()
    #expect(state(c, "shortcut_preset/name") == "\"Superhuman\"" && c.shortcutPreset().preset == "Superhuman")
    _ = try c.run(command: .setShortcutPreset(preset: "GotEmail"))
    #expect(state(c, "shortcut_preset/name") == "\"GotEmail\"")
    #expect(words { _ = try c.run(command: .setShortcutPreset(preset: "Emacs")) } != nil && state(c, "shortcut_preset/name") == "\"GotEmail\"")
    let ids = c.accounts().map(\.id)
    #expect(jsonList(state(c, "account/order")) == ids)
    let flipped = Array(ids.reversed())
    _ = try c.run(command: .reorderAccounts(ids: flipped))
    #expect(c.accounts().map(\.id) == flipped && jsonList(state(c, "account/order")) == flipped)
    #expect(words { _ = try c.run(command: .reorderAccounts(ids: Array(ids.dropFirst()))) }?.contains("every account once") == true)
    #expect(c.accounts().map(\.id) == flipped)
}

// MARK: group 5, undo cases for labels and delete

private func label(_ c: FixtureCore, _ path: String) -> MailLabel? { c.labels().first { $0.path == path } }
private func undoLast(_ c: FixtureCore) throws { _ = try c.run(command: .undo) }

@Test func d7ZUndoesALabelRename() throws {
    let c = try core()
    let prox = try #require(label(c, "Work > Clients > Provanta"))
    let out = try c.run(command: .renameLabel(labelId: prox.id, name: "Provanta Global"))
    #expect(out.undoEntryId != nil && label(c, "Work > Clients > Provanta Global > RFP 2026 > Q4") != nil)
    try undoLast(c)
    #expect(label(c, "Work > Clients > Provanta > RFP 2026 > Q4")?.depth == 5 && label(c, "Work > Clients > Provanta Global") == nil)
    #expect(state(c, "undo_entry/consumed_last") == "true")
}

@Test func d7ZUndoesALabelMoveWithItsSubtree() throws {
    let c = try core()
    let rfp = try #require(label(c, "Work > Clients > Provanta > RFP 2026"))
    let nord = try #require(label(c, "Work > Clients > Nordlicht"))
    let out = try c.run(command: .moveLabel(labelId: rfp.id, parentId: nord.id))
    #expect(out.undoEntryId != nil && label(c, "Work > Clients > Nordlicht > RFP 2026 > Q4")?.depth == 5)
    try undoLast(c)
    #expect(label(c, "Work > Clients > Provanta > RFP 2026 > Q4")?.depth == 5 && label(c, "Work > Clients > Nordlicht > RFP 2026") == nil)
    // a move that changes nothing pushes no entry
    let same = try c.run(command: .moveLabel(labelId: rfp.id, parentId: rfp.parentId))
    #expect(same.undoEntryId == nil)
}

@Test func d7ZRemovesALabelItCreatedAndItsAssignments() throws {
    let c = try core()
    let made = try #require(try c.run(command: .createLabel(name: "Archive 2026", parentId: nil)).createdId)
    _ = try c.run(command: .label(messageIds: ["msg-02"], labelName: "Archive 2026", createIfMissing: false))
    #expect(state(c, "label_assignment/msg-02") == "[\"Archive 2026\"]")
    try undoLast(c)                          // the assignment first (last in, first out)
    try undoLast(c)                          // then the label itself
    #expect(label(c, "Archive 2026") == nil && c.labels().allSatisfy { $0.id != made })
    #expect(state(c, "label_assignment/msg-02") == "[]")
}

@Test func d7ZBringsADeletedMessageBack() throws {
    let c = try core()
    let n = c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).count
    let out = try c.run(command: .deleteMessage(messageId: "msg-02"))
    #expect(out.undoEntryId != nil && c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).count == n - 1 && state(c, "message/msg-02/deleted") == "true")
    try undoLast(c)
    #expect(c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).count == n && state(c, "message/msg-02/deleted") == "false")
}

// MARK: group 6, split tabs

private func tabIds(_ c: FixtureCore) -> [String] { c.splitTabs().map(\.id) }

@Test func d7ASeventhTabHoldsTheMessagesItsRuleMatches() throws {
    let c = try core()
    #expect(tabIds(c) == ["primary", "needs-reply", "updates", "promotions", "social", "lbl-6"], "delta 9: the five categories (Social off) and the fixture's pinned Provanta; VIPs is a mailbox")
    _ = try c.run(command: .label(messageIds: ["msg-02"], labelName: "Suppliers", createIfMissing: true))
    let suppliers = try #require(c.labels().first { $0.name == "Suppliers" })
    #expect(state(c, "split_tab/count") == "6")
    let id = try #require(try c.run(command: .addSplitTab(name: "Supplier mail", rule: .label(labelId: suppliers.id))).createdId)
    #expect(id == "supplier-mail" && tabIds(c).count == 7 && tabIds(c).last == id)
    #expect(c.inbox(query: InboxQuery(tabId: id, accountId: nil, labelId: nil)).map(\.messageId) == ["msg-02"])
    #expect(state(c, "split_tab/\(id)/count") == "1" && state(c, "split_tab/\(id)/name") == "\"Supplier mail\"" && state(c, "split_tab/\(id)/rule") == "\"label:\(suppliers.id)\"")
    #expect(c.counts().tabs.map(\.tabId) == c.splitTabs().filter(\.enabled).map(\.id) && c.counts().tabs.last?.unread == 0)   // msg-02 is read in the fixture; Social is off
}

@Test func d7ATabRuleThatNamesNothingRealIsRefusedInWords() throws {
    let c = try core()
    #expect(words { _ = try c.run(command: .addSplitTab(name: "", rule: .domain(domain: "example.com"))) }?.contains("needs a name") == true)
    #expect(words { _ = try c.run(command: .addSplitTab(name: "primary", rule: .domain(domain: "example.com"))) }?.contains("exists already") == true)
    #expect(words { _ = try c.run(command: .addSplitTab(name: "X", rule: .sender(address: "nobody"))) }?.contains("full address") == true)
    #expect(words { _ = try c.run(command: .addSplitTab(name: "X", rule: .domain(domain: "a@b.com"))) }?.contains("domain") == true)
    #expect(words { _ = try c.run(command: .addSplitTab(name: "X", rule: .account(accountId: "acc-99"))) } != nil)
    #expect(words { _ = try c.run(command: .addSplitTab(name: "X", rule: .label(labelId: "lbl-999"))) } != nil)
    #expect(words { _ = try c.run(command: .addSplitTab(name: "X", rule: .builtin(name: "primary"))) }?.contains("pick a rule") == true)
    #expect(tabIds(c).count == 6)
}

@Test func d7ASenderTabCountsItsUnreadAndFollowsArchive() throws {
    let c = try core()
    let id = try #require(try c.run(command: .addSplitTab(name: "Linea", rule: .sender(address: "orders@linea-supply-example.com"))).createdId)
    #expect(c.inbox(query: InboxQuery(tabId: id, accountId: nil, labelId: nil)).map(\.messageId) == ["msg-01"])
    let unread = c.counts().tabs.first { $0.tabId == id }?.unread
    #expect(unread == 1)
    _ = try c.run(command: .archive(messageIds: ["msg-01"]))
    #expect(c.inbox(query: InboxQuery(tabId: id, accountId: nil, labelId: nil)).isEmpty && c.counts().tabs.first { $0.tabId == id }?.unread == 0 && state(c, "split_tab/\(id)/count") == "0")
}

@Test func d7TabsReorderHideAndComeBackToTheDefaults() throws {
    let c = try core()
    let ids = tabIds(c)
    let flipped = Array(ids.dropLast().reversed()) + [ids.last!]   // delta 9: a pinned sub-label keeps its tree place; the categories are the ones that move
    _ = try c.run(command: .reorderSplitTabs(ids: flipped))
    let on = Set(c.splitTabs().filter(\.enabled).map(\.id))
    #expect(tabIds(c) == flipped && c.tabIds == flipped.filter(on.contains) && c.counts().tabs.map(\.tabId) == flipped.filter(on.contains) && c.splitTabs().map(\.order) == [0, 1, 2, 3, 4, 5])
    #expect(words { _ = try c.run(command: .reorderSplitTabs(ids: Array(ids.dropLast()))) }?.contains("every tab once") == true)
    _ = try c.run(command: .setSplitTabEnabled(id: "promotions", on: false))
    #expect(state(c, "split_tab/promotions/enabled") == "false" && !c.counts().tabs.map(\.tabId).contains("promotions") && c.splitTabs().count == 6 && !c.tabIds.contains("promotions"))
    _ = try c.run(command: .renameSplitTab(tabId: "updates", name: "Receipts"))
    _ = try c.run(command: .addSplitTab(name: "Extra", rule: .domain(domain: "example.com")))
    _ = try c.run(command: .restoreDefaultSplitTabs)
    #expect(tabIds(c) == ids && state(c, "split_tab/updates/name") == "\"Updates\"" && state(c, "split_tab/social/enabled") == "false" && state(c, "split_tab/promotions/enabled") == "true" && c.counts().tabs.count == 5, "delta 9: Social starts off, Provanta starts pinned")
    for id in ids.dropLast() { _ = try c.run(command: .setSplitTabEnabled(id: id, on: false)) }
    #expect(words { _ = try c.run(command: .setSplitTabEnabled(id: ids.last!, on: false)) }?.contains("at least one") == true)
}

// MARK: group 7, composer and quick reply

@Test func d7TheSignatureIsTheAccountsOwn() throws {
    let c = try core()
    let s = try #require(c.signature(address: "assem@northlight-example.de"))
    // delta 15: every fixture account carries its own two-line signature, which wins over the quick reply's one-liner
    #expect(s.text == "Best,\nAssem Chammah · Nordlicht" && s.address == "assem@northlight-example.de")
    #expect(c.signature(address: " ASSEM@northlight-example.de ")?.text == "Best,\nAssem Chammah · Nordlicht")
    #expect(c.signature(address: "someone@else.example") == nil)
}

@Test func d7AnOrganisationsMailIsRepliedToThePersonWhoSignedIt() throws {
    let c = try core()
    #expect(c.message(messageId: "msg-01")?.replyToName == "Karen")                  // Linea Supply, signed Karen Bosch
    #expect(c.message(messageId: "msg-02")?.replyToName == nil)                      // From is Marie herself
    #expect(c.message(messageId: "msg-10")?.replyToName == nil)                      // LHG Procurement signs as itself
    #expect(c.message(messageId: "thr-01-m1")?.replyToName == nil)                   // no body to read
    #expect(FixtureCore.replyToName(sender: "Linea Supply", body: ["Hi", "Thanks.", "Karen Bosch, Linea Supply"]) == nil)   // no closing line
    #expect(FixtureCore.replyToName(sender: "Shop", body: ["Hi", "Best,", "this is a long sentence that is no name at all"]) == nil)
}

@Test func d7AQuickReplyIsSentOnceItsUndoWindowEnds() throws {
    let clock = FixtureClock()
    let c = try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: clock)
    let sink = Catch()
    let sub = c.subscribe(stream: .outbox, sink: sink)
    defer { c.unsubscribe(subscription: sub) }
    let out = try c.run(command: .quickSend(threadId: "thr-01", body: "Thanks Karen, yes."))
    let id = try #require(out.createdId)
    #expect(state(c, "outbox_entry/0/state") == "\"undoable\"" && c.outbox().map(\.id) == [id])
    #expect(clock.advance(seconds: Double(WhooshConstants.UNDO_SEND_DEFAULT_S) - 1))
    settle()
    #expect(state(c, "outbox_entry/0/state") == "\"undoable\"" && c.outbox().count == 1)
    #expect(clock.advance(seconds: 2))
    settle()
    #expect(state(c, "outbox_entry/0/state") == "\"sent\"" && c.outbox().isEmpty)
    let sent = sink.events.compactMap { e -> String? in if case .sendCompleted(let oid, _) = e { return oid } else { return nil } }
    #expect(sent == [id])                                                            // exactly one delivery
    _ = c.outbox(); _ = state(c, "outbox_entry/0/state")
    settle()
    #expect(sink.events.filter { if case .sendCompleted = $0 { return true } else { return false } }.count == 1)
}

@Test func d7AnUndoneQuickReplyNeverSends() throws {
    let clock = FixtureClock()
    let c = try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: clock)
    let id = try #require(try c.run(command: .quickSend(threadId: "thr-01", body: "Oops")).createdId)
    _ = try c.run(command: .cancelSend(outboxId: id))
    #expect(clock.advance(seconds: Double(WhooshConstants.UNDO_SEND_DEFAULT_S) + 5))
    #expect(state(c, "outbox_entry/0/state") == "\"cancelled\"" && c.outbox().isEmpty)
}

// MARK: group 8, notes

@Test func d7ANoteIsWrittenEditedAndDeletedOnItsTarget() throws {
    let c = try core()
    let thread = NoteTarget.thread(threadId: "thr-01"), message = NoteTarget.message(messageId: "msg-01")
    #expect(c.notes(target: thread).isEmpty && state(c, "note/count") == "0")
    let a = try #require(try c.run(command: .addNote(target: thread, text: "  Ask Karen about the varnish.  ")).createdId)
    let b = try #require(try c.run(command: .addNote(target: message, text: "Price is without VAT")).createdId)
    #expect(a != b && c.notes(target: thread).map(\.text) == ["Ask Karen about the varnish."] && c.notes(target: message).map(\.id) == [b])
    #expect(state(c, "note/count") == "2" && state(c, "note/\(a)/target") == "\"thread:thr-01\"" && c.notes(target: thread).first?.writtenAtMs ?? 0 > 0)
    let written = c.notes(target: thread).first?.writtenAtMs
    _ = try c.run(command: .editNote(noteId: a, text: "Ask Karen about the spot varnish."))
    #expect(state(c, "note/\(a)/text") == "\"Ask Karen about the spot varnish.\"" && c.notes(target: thread).first?.writtenAtMs == written)
    _ = try c.run(command: .deleteNote(noteId: a))
    #expect(c.notes(target: thread).isEmpty && c.notes(target: message).count == 1 && state(c, "note/count") == "1")
    let c2 = try #require(try c.run(command: .addNote(target: thread, text: "again")).createdId)
    #expect(c2 != a && c2 != b)                                                      // an id is not handed out twice
}

@Test func d7ANoteNeedsWordsAndARealTarget() throws {
    let c = try core()
    #expect(words { _ = try c.run(command: .addNote(target: .thread(threadId: "thr-01"), text: "   ")) } == "a note needs words")
    #expect(words { _ = try c.run(command: .addNote(target: .thread(threadId: "nope"), text: "x")) } != nil)
    #expect(words { _ = try c.run(command: .addNote(target: .message(messageId: "nope"), text: "x")) } != nil)
    let n = try #require(try c.run(command: .addNote(target: .message(messageId: "thr-01-m1"), text: "older message")).createdId)
    #expect(words { _ = try c.run(command: .editNote(noteId: n, text: "")) } == "a note needs words" && state(c, "note/\(n)/text") == "\"older message\"")
    #expect(words { _ = try c.run(command: .editNote(noteId: "note-99", text: "x")) } != nil && words { _ = try c.run(command: .deleteNote(noteId: "note-99")) } != nil)
}
