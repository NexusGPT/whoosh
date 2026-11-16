/// Defaults for the two reads added to `WhooshCore` on 2026-11-07 (outbox, sync states), so a Swift
/// conformer written before them (a test double) still compiles. FixtureCore and the Rust core
/// implement both; a default returns the empty answer.
public extension WhooshCore {
    func unsubscribeMethod(messageId: String) -> UnsubscribeMethod? { nil }
    func attachments(messageId: String) -> [AttachmentFile] { [] }
    func drafts(threadId: String) -> [DraftSummary] { [] }
    func accountRemoval(accountId: String) -> AccountRemoval? { nil }
    func outbox() -> [OutboxEntry] { [] }
    func syncStates() -> [SyncState] { [] }
    func body(messageId: String) -> MessageBody? { nil }
    func labels() -> [MailLabel] { [] }
    func accounts() -> [AccountSummary] { [] }
    func smartFolders() -> [SmartFolder] { [] }
    func message(messageId: String) -> Message? { nil }
    func contact(address: String) -> Contact? { nil }
    func contactMessages(contactId: String) -> [InboxRow] { [] }
    func ruleMatchCount(ruleWords: String) -> UInt32 { 0 }
    func ruleSetMatches(ruleSet: RuleSet, limit: UInt32) -> [String] { [] }   // delta 20
    func decisionConsent(provider: DecisionProviderKind) -> Bool { false }   // delta 23: no consent until the owner gives it
}

/// Delta 7 reads: a conformer written before them answers with nothing.
public extension WhooshCore {
    func answer(conversationId: String) -> AskAnswer? { nil }
    func proposals(conversationId: String) -> [AgentProposal] { [] }
}

/// Sugar over `run` for the Ask panel: the contract has one `Ask` and one `ApproveProposal`; the panel's words are three.
public extension WhooshCore {
    /// Asks a question; the outcome's `createdId` is the conversation. Read the result with `answer(conversationId:)`.
    @discardableResult
    func ask(_ question: String, agentId: String? = nil) throws -> CommandOutcome { try run(command: .ask(agentId: agentId, question: question)) }
    @discardableResult
    func approveProposal(_ id: String) throws -> CommandOutcome { try run(command: .approveProposal(proposalId: id, approve: true)) }
    @discardableResult
    func dismissProposal(_ id: String) throws -> CommandOutcome { try run(command: .approveProposal(proposalId: id, approve: false)) }
    func giveConsent(providerId: String) throws { _ = try run(command: .giveAgentConsent(providerId: providerId)) }
}

/// Delta 7, group 2: a conformer written before the settings reads answers with the documented defaults.
public extension WhooshCore {
    func notchSetting() -> NotchSetting { NotchSetting(show: true, buttons: ["mark-read", "archive", "reply"], hiddenInScreenShare: true, followFocus: true, listShows: .latest) }
    func notificationSettings() -> [NotificationSetting] { [] }
    /// The notch's alerts (owner's picks of 14 Nov 2026): a conformer written before them answers the picked start values, and every mail is Primary.
    func notchAlerts() -> NotchAlerts { notchAlertsDefault() }
    func notchKind(messageId: String) -> NotchKind { .primary }
}

/// Delta 7, group 3: a conformer written before the AI-settings reads answers with nothing.
public extension WhooshCore {
    func aiProviders() -> [AiProvider] { [] }
    func agentConfigs() -> [AgentConfig] { [] }
    func decisionModel() -> DecisionModel? { nil }
    func decisionPreview() -> [DecisionVerdict] { [] }
    func speechEngine() -> SpeechEngine { SpeechEngine(kind: .local, ready: false) }
    func mcpGrants() -> [McpGrant] { [] }
}

/// Delta 7, group 4.
public extension WhooshCore {
    func shortcutPreset() -> ShortcutPreset { ShortcutPreset(preset: "Superhuman", overrides: [:]) }
}

/// Delta 7, group 6: a conformer written before split-tab edits answers with no tabs.
public extension WhooshCore {
    func splitTabs() -> [SplitTab] { [] }
}

/// Delta 7, group 7 and 8: a conformer written before them answers with nothing.
public extension WhooshCore {
    func signature(address: String) -> Signature? { nil }
    func notes(target: NoteTarget) -> [Note] { [] }
}

/// Delta 14 default: a core written before the bring-your-own sign-in knows no client ID. Detection and the sign-in check
/// have NO default (real core slice 1, 2026-11-09): a stub answer here is how the installed app fell to manual setup with
/// "not available here"; every conformer now answers them itself, so a missing one is a build error.
public extension WhooshCore {
    func oauthSignInState(accountDraftId: String) -> OAuthSignInState { .needsClientId }
}

/// Delta 8, group 2: a conformer written before the contacts read answers with nobody; an Embedder written before
/// `embed_query` embeds the query like a document.
public extension WhooshCore {
    func contacts() -> [Contact] { [] }
    func snoozedMessages() -> [Snooze] { [] }
    func splitTabRuleMatchCount(rule: SplitTabRule) -> UInt32 { 0 }
}

public extension Embedder {
    func embedQuery(text: String) throws -> [Float] { try embed(texts: [text]).first ?? [] }
}

/// Delta 9: a conformer written before labels-as-tabs answers with nothing, and an Embedder written before `embed_media` refuses in words.
public extension WhooshCore {
    func pinnedLabels() -> [MailLabel] { [] }
    func ruleSetCount(ruleSet: RuleSet) -> RuleSetCount { RuleSetCount(count: 0, unread: 0, newestAtMs: nil) }
    func labelRerunCount(labelId: String) -> UInt32 { 0 }
    func suggestions() -> [Suggestion] { [] }
    func blocks() -> [Block] { [] }
}

public extension Embedder {
    /// Delta 12 default: a conformer written before the title slot ignores the title and embeds as before ("title: none").
    func embedTitled(title: String?, texts: [String]) throws -> [[Float]] { try embed(texts: texts) }

    func embedMedia(kind: MediaKind, path: String) throws -> [Float] {
        throw CoreError.Invalid(field: "path", reasonWords: "this embedder reads text only: it cannot embed an image, a sound or a video frame")
    }
}
