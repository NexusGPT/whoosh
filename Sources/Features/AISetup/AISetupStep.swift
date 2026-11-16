import Foundation
import Observation

/// The quick-start step's logic, without its view (the look is the lead's). Each of the three needs is set OR explicitly skipped; the step is done
/// when none is left open. Skip is always possible: it ends the step at once, leaves every unset need unset, and the warning marks stay on
/// (they read `AISetupStatus`, never this model). Nothing here writes until an action is called: choosing a need only opens its door.
@MainActor @Observable
final class AISetupStep {
    static let shared = AISetupStep()
    private(set) var skippedNeeds: Set<AINeed> = []
    /// Skip pressed on the step as a whole
    private(set) var skippedAll = false
    /// "Use Jev" in place: the key field, then the consent, then Jev is the decision model.
    enum JevStage: String { case none, key, consent, chosen }
    private(set) var jev: JevStage = .none
    private var status: AISetupStatus { .shared }

    private init() {}

    func reset() { skippedNeeds = []; skippedAll = false; jev = .none }

    /// A need is settled when it is ready, or the owner skipped it. A download or a check in flight is not settled yet.
    func settled(_ n: AINeed) -> Bool { status.status(n).usable || skippedNeeds.contains(n) || skippedAll }
    /// "done": every need set or skipped
    var done: Bool { AINeed.allCases.allSatisfy(settled) }
    /// Continue is always on: the step never blocks (a need left open only keeps its warning mark).
    var canContinue: Bool { true }
    /// What a surface shows after the step: the warning marks to keep on.
    var marksOn: [AINeed] { AINeed.allCases.filter { !status.status($0).usable } }

    func skip(_ n: AINeed) { skippedNeeds.insert(n) }
    func unskip(_ n: AINeed) { skippedNeeds.remove(n) }
    /// Skip on the step: leaves the marks on, ends the step
    func skipAll() { skippedAll = true }

    // the three actions: the existing doors
    func saveKey(provider: String, _ key: String) { skippedNeeds.remove(.writing); status.saveKey(provider: provider, key) }
    func download() { skippedNeeds.remove(.decision); status.downloadLaya() }
    /// The provider a pasted key belongs to, from its first characters (Anthropic `sk-ant`, OpenRouter `sk-or`, OpenAI `sk-`, Google `AIza`); else Anthropic.
    nonisolated static func provider(forKey key: String) -> String {
        let k = key.trimmingCharacters(in: .whitespaces)
        if k.hasPrefix("sk-ant") { return "anthropic" }
        if k.hasPrefix("sk-or") { return "openrouter" }
        if k.hasPrefix("AIza") { return "gemini" }
        if k.hasPrefix("sk-") { return "openai" }
        return "anthropic"
    }
    /// The step's one key field: the key goes to the provider it belongs to.
    func pasteWritingKey(_ key: String) { saveKey(provider: Self.provider(forKey: key), key) }

    /// "Use Jev": opens the key field in place.
    func openJev() { skippedNeeds.remove(.decision); jev = .key }
    /// The Jev key typed in the field: stored, then the consent is asked with the existing sentence.
    func submitJevKey(_ key: String) { guard jev == .key, !key.trimmingCharacters(in: .whitespaces).isEmpty else { return }; status.connectJev(key: key); jev = .consent }
    /// Allow: GiveDecisionConsent, then the choice of Jev as the decision model (SaveDecisionModel; the pane's own `pendingJev` finishes it if the key is still landing).
    func allowJev() {
        guard jev == .consent else { return }
        status.giveJevConsent(true)
        AISettingsStore.shared.chooseEngine(.jev)
        jev = .chosen; status.refresh()
    }
    func useJev(key: String, consent: Bool) {
        skippedNeeds.remove(.decision)
        status.connectJev(key: key)
        if consent { status.giveJevConsent(true) }
    }
}
