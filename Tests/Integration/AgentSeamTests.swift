import Foundation
import Testing
@testable import WhooshUI

// AgentSeamTests: the agent run and the proposal gate over the core seam (docs/CAPABILITY-CONTRACT.md C-J5.6 gate rule).
// Round 2 (2026-11-08): the Ask my inbox panel is retired (docs/PRD.md "One search"), so the Ask CONVERSATION seam is dropped:
// askAnswersAndSavesTheConversation and recordOneIsAmongTheMessagesTheAnswerQuotes are deleted (an answer with citations is no
// longer a product surface; search returns messages, tested in journeys/J5.4 and Tests/Integration/SearchSeamTests.swift).
// Kept, because the core command `.ask` stays the agent entry that drafts, decisions and notes use: the step limit, the
// "nothing changes before approve" gate, a rejected proposal, and a missing proposal refused in words.
// The core is the one every view reads: CoreHost.shared.core. In a test run it is the UnavailableCore until the Rust core
// lands, so each test fails in words ("the local engine is not built yet") and turns green on the real core without edits.

private final class Seen: EventSink, @unchecked Sendable {
    private let lock = NSLock()
    private var all: [CoreEvent] = []
    func onEvent(event: CoreEvent) { lock.withLock { all.append(event) } }
    var events: [CoreEvent] { lock.withLock { all } }
}

private let question = "what did suppliers quote me this spring?"
private let labelAsk = "label these as Supplier quotes"
private let label = "Supplier quotes"

private func int(_ core: WhooshCore, _ path: String) -> Int? { core.state(path: path).flatMap { Int($0) } }

// swift-tests lane (round main76 reds theAgentStopsWithinItsStepLimit, aProposalChangesNothingUntilItIsApproved, aRejectedProposalChangesNothingAndAnApprovedOneDoes):
// the three tests asked through CoreHost.shared.core, a core every other test shares, and the fixture's "Ask my inbox" agent
// runs on the Anthropic provider, which answers `agentConsentNeeded` (no step) until mail consent and a key exist
// (Sources/CoreBridge/FixtureAgents.swift, `ask`: the `!onDevice(prov.id), prov.mailConsentAtMs == nil` branch). Each test now owns a
// fixture core whose Ask agent runs on the on-device provider ("local"), so no consent, key or network is involved: nothing leaves this Mac.
private func askCore() throws -> FixtureCore {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let c = try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
    _ = try c.run(command: .saveAgentConfig(config: AgentConfig(id: "inbox", name: "Ask my inbox", purpose: "Answers questions about your mail and proposes labels",
        providerId: "local", model: "on-device", instructions: "", allowed: [.readMail, .search, .label], builtIn: true)))
    return c
}

private func proposalId(_ seen: Seen) -> String? {
    for e in seen.events { if case .proposalReady(let id) = e { return id } }
    return nil
}

@Test func theAgentStopsWithinItsStepLimit() throws {
    let core = try askCore()
    _ = try core.run(command: .ask(agentId: nil, question: question))
    let steps = try #require(int(core, "agent_conversation/last/steps"), "the conversation records no step count")
    #expect(steps >= 1 && steps <= WhooshConstants.AGENT_MAX_STEPS)
}

// `@MainActor`: the agent events reach a sink synchronously only on the main thread (FixtureCore.deliver). The label ask comes AFTER a question:
// the fixture labels "the messages of the last answer" and answers "Ask a question first" otherwise (FixtureAgents.swift, `wantsLabel` branch).
@MainActor @Test func aProposalChangesNothingUntilItIsApproved() throws {
    let core = try askCore()
    let seen = Seen()
    let sub = core.subscribe(stream: .agent, sink: seen)
    defer { core.unsubscribe(subscription: sub) }
    _ = try core.run(command: .ask(agentId: nil, question: question))
    _ = try core.run(command: .ask(agentId: nil, question: labelAsk))
    let id = try #require(proposalId(seen), "no ProposalReady event")
    #expect(int(core, "label_assignment/\(label)/count") == 0, "the label was applied before approval")
    _ = try core.run(command: .approveProposal(proposalId: id, approve: true))
    let applied = try #require(int(core, "label_assignment/\(label)/count"))
    #expect(applied >= 1, "approval applied the label to nothing")
}

@MainActor @Test func aRejectedProposalChangesNothingAndAnApprovedOneDoes() throws {
    let core = try askCore()
    let seen = Seen()
    let sub = core.subscribe(stream: .agent, sink: seen)
    defer { core.unsubscribe(subscription: sub) }
    _ = try core.run(command: .ask(agentId: nil, question: question))
    _ = try core.run(command: .ask(agentId: nil, question: labelAsk))
    let first = try #require(proposalId(seen))
    _ = try core.run(command: .approveProposal(proposalId: first, approve: false))
    #expect(int(core, "label_assignment/\(label)/count") == 0, "a rejected proposal changed mail")
    // positive control: the same probe sees a change once a proposal is approved
    _ = try core.run(command: .ask(agentId: nil, question: labelAsk))
    let second = try #require(seen.events.reversed().compactMap { e -> String? in if case .proposalReady(let id) = e { return id }; return nil }.first)
    #expect(second != first)
    _ = try core.run(command: .approveProposal(proposalId: second, approve: true))
    #expect((int(core, "label_assignment/\(label)/count") ?? 0) >= 1)
}

@Test func approvingAProposalThatDoesNotExistIsRefusedInWords() throws {
    let core = CoreHost.shared.core
    do {
        _ = try core.run(command: .approveProposal(proposalId: "no-such-proposal", approve: true))
        Issue.record("approving a missing proposal succeeded")
    } catch CoreError.NotFound(let kind, let id) {
        #expect(id == "no-such-proposal" && !kind.isEmpty)
    } catch {
        Issue.record("expected NotFound for a missing proposal, got \(error)")
    }
}
