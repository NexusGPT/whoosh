import Foundation
#if canImport(FoundationModels)
import FoundationModels
#endif

/// Lane fix-aisort round 8 (the owner's "AI filter seeing 0 messages"): the judge of the Labels AI step is Apple's on-device language model
/// when the system says it is available. Measured by the lead (2026-11-12, 12 synthetic mails x 4 labels, one request per mail, the
/// "one label or none" form below): 10 right, 1 wrong, 1 unclear, median 372 ms per mail; Laya reached the trust level on 0 of 12.
///
/// The engine asks the Labels question in its per-label form (one yes/no per offered label, id `labels/model#<label id>`, the statement
/// `decide::label_statement` = `This email belongs under the label "<path>": <description>`). This judge reads the label id from the question
/// id and the path and description back from the statement, asks Apple's model ONE request per mail with the instructions the lead measured
/// (verbatim), and answers in the per-label form the engine reads today: yes-probability 1.0 for the label picked, 0.0 for the others, 0.0
/// for all on "none". So MODEL_TRUST_LABEL and the walk stay as they are. The strict read: exactly one offered label id, or `none`; anything
/// else is unreadable and THROWS (the engine's failure state of round 1: the mail waits and is asked again, never a "no").
///
/// Every other question (the junk filter's yes/no, a choice in choice mode, the sanity check's statement forms) goes to Laya as before.
/// When Apple's model is not available, the Labels question goes to Laya too, and the result's checkpoint says why
/// (`fallback:<AppleReason raw value>`), so the engine's label card can say that sorting needs Apple Intelligence.
/// A test copy (rig) never calls the real model: it uses the rig's armed stand-in (`RigFixture`, unavailable by default) unless the check
/// sets `WHOOSH_RIG_APPLE_JUDGE=real` (rig only).
public final class LabelJudgeRouter: DecisionPort, @unchecked Sendable {
    /// The checkpoint name a result answered by Apple's model carries (the engine reads it: `decision_model.rs` `APPLE_CHECKPOINT`).
    public static let appleCheckpoint = "apple-on-device"
    /// The prefix of the checkpoint of a Labels result Laya answered because Apple's model was not available, then the reason's raw value.
    public static let fallbackPrefix = "fallback:"

    private let laya: DecisionPort
    private let judge: AppleLabelJudge

    init(laya: DecisionPort, judge: AppleLabelJudge = AppleLabelJudge()) {
        self.laya = laya
        self.judge = judge
    }

    public func provider() -> DecisionProviderKind { .local }

    public func decide(state: DecisionState, questions: [DecisionQuestion]) throws -> DecisionResult {
        let labels = questions.filter { AppleLabelPrompt.offered($0) != nil }
        guard !labels.isEmpty else { return try laya.decide(state: state, questions: questions) }
        if let reason = judge.unavailableReason() {
            let r = try laya.decide(state: state, questions: questions)
            return DecisionResult(answers: r.answers, answeredBy: r.answeredBy, checkpoint: Self.fallbackPrefix + reason.rawValue)
        }
        var answers = try judge.answer(state: state, questions: labels)
        let others = questions.filter { AppleLabelPrompt.offered($0) == nil }
        if !others.isEmpty { answers += try laya.decide(state: state, questions: others).answers }
        // the filing names the model that decided: `DecisionProviderKind.apple` -> `DecidedBy.apple` (contracts, round 8; migration 0031)
        return DecisionResult(answers: answers, answeredBy: .apple, checkpoint: Self.appleCheckpoint)
    }
}

/// One request per mail to Apple's on-device model; the strict read of its one-word answer.
struct AppleLabelJudge: Sendable {
    // the model's instructions, the request text and its strict read live in AppleLabelPrompt.swift: they are words for the model, in
    // English whatever the app's language, never the owner's, so they do not go through the catalogue
    /// The reason Apple's model cannot judge now, nil when it can. A rig copy reads its armed stand-in unless `WHOOSH_RIG_APPLE_JUDGE=real`.
    func unavailableReason() -> AppleReason? {
        let env = ProcessInfo.processInfo.environment
        if CorePlace.current(env: env).rig && env["WHOOSH_RIG_APPLE_JUDGE"] == "stand-in" { return nil }   // round 11: the scripted judge
        if CorePlace.current(env: env).rig && env["WHOOSH_RIG_APPLE_JUDGE"] != "real" {
            return (RigFixture.load()?.apple ?? ArmedAppleBackend(reason: .deviceNotEligible)).reason
        }
        return AppleIntelligenceBackend.reason()
    }

    /// One request; throws in words on a failure or an unreadable answer (never a "no").
    func answer(state: DecisionState, questions: [DecisionQuestion]) throws -> [DecisionAnswer] {
        let labels = questions.compactMap(AppleLabelPrompt.offered)
        let standIn = CorePlace.current(env: ProcessInfo.processInfo.environment).rig && ProcessInfo.processInfo.environment["WHOOSH_RIG_APPLE_JUDGE"] == "stand-in"
        // round 11: a refusal of the mail's text (content, size, language) is asked once more with the subject and the sender alone; a second
        // refusal, or an answer with no label of the list, sets the mail aside (the engine's code: it is not asked again); only a failure of
        // the model itself (not available, a crash, a time-out) keeps the mail waiting and the label's failure sentence
        var text: String
        do {
            text = try Self.ask(AppleLabelPrompt.prompt(labels, state: state), labels: labels, standIn: standIn)
        } catch let first as AppleMailRefusal {
            do {
                text = try Self.ask(AppleLabelPrompt.prompt(labels, state: state, withBody: false), labels: labels, standIn: standIn)
            } catch let again as AppleMailRefusal {
                throw Self.setAside(again.rawValue == first.rawValue ? first.rawValue : again.rawValue)
            }
        }
        guard let pick = AppleLabelPrompt.read(text, ids: labels.map(\.id)) else {
            // the model's words are not repeated: they may quote the mail
            throw Self.setAside(AppleLabelPrompt.noLabelReason)
        }
        return AppleLabelPrompt.answers(for: labels, pick: pick)
    }

    /// One request, blocking the caller (the core's filing thread). Throws `AppleMailRefusal` for a refusal of the mail, the failure in words
    /// (`refusal`) for a failure of the model.
    static func ask(_ prompt: String, labels: [(id: String, path: String, description: String)], standIn: Bool) throws -> String {
        if standIn { return try AppleLabelPrompt.standIn(labels, prompt: prompt) }
        final class Box: @unchecked Sendable { var result: Result<String, Error>? }
        let box = Box(), done = DispatchSemaphore(value: 0)
        Task.detached(priority: .utility) {
            do { box.result = .success(try await AppleIntelligenceBackend.respond(instructions: AppleLabelPrompt.instructions, prompt: prompt, maxTokens: AppleLabelPrompt.answerTokens)) }
            catch { box.result = .failure(error) }
            done.signal()
        }
        done.wait()
        do { return try box.result!.get() }
        catch let e as AppleMailRefusal { throw e }
        catch let e as LocalModelError { throw refusal(e.description) }
        catch { throw refusal(Words.text("Apple Intelligence could not answer: %@", error.localizedDescription)) }
    }

    /// Round 11: a failure that belongs to this mail (`decision::decide::MAIL_REFUSED_CODE`), its reason as an id: the engine sets the mail aside.
    static func setAside(_ reason: String) -> CoreError {
        .Action(failure: ActionFailed(accountName: "Apple Intelligence", reasonWords: reason, retryable: false, code: AppleLabelPrompt.mailRefusedCode))
    }

    static func refusal(_ words: String) -> CoreError {
        .Action(failure: ActionFailed(accountName: "Apple Intelligence", reasonWords: words, retryable: true, code: nil))
    }
}
