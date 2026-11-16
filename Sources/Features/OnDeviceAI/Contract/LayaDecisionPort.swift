import Foundation

/// The DecisionPort the core calls for provider "local": Laya on this Mac. Typed questions in, verdicts with probabilities out.
/// Contract limits this port works around (listed in docs/lanes/ondeviceai.md "Contract deltas"): a choice option is one string,
/// so "label: description" is the convention (the answer returns the label); a score question carries no levels, so it gets
/// three plain levels; a noul carries no wording, so the authors' defaults apply.
public final class LayaDecisionPort: DecisionPort, @unchecked Sendable {
    public static let scoreLevels = ["low", "medium", "high"]
    private let engine: LayaEngine

    public init(engine: LayaEngine) { self.engine = engine }

    public func provider() -> DecisionProviderKind { .local }

    public func decide(state: DecisionState, questions: [DecisionQuestion]) throws -> DecisionResult {
        let layaQs: [LayaQuestion] = questions.map { q in
            switch q {
            case .choice(let id, let prompt, let options):
                return .choice(id: id, instructions: prompt, options: options.map(Self.split))
            case .score(let id, let prompt):
                return .score(id: id, instructions: prompt, levels: Self.scoreLevels)
            case .noul(let id, let prompt):
                return .noul(id: id, instructions: prompt)
            }
        }
        let text = LayaState.text(fields: state.fields, text: state.text)
        let answers = try engine.answer(state: text, questions: layaQs).map { a -> DecisionAnswer in
            switch a.value {
            case .choice(let label, let probs): return .choice(id: a.id, choice: label, probabilities: probs)
            case .score(let v, _): return .score(id: a.id, value: v)
            case .noul(let yes): return .noul(id: a.id, yes: yes >= 0.5, probability: yes)
            }
        }
        return DecisionResult(answers: answers, answeredBy: .local, checkpoint: engine.checkpointName)
    }

    static func split(_ option: String) -> (label: String, description: String?) {
        guard let r = option.range(of: ": ") else { return (option, nil) }
        return (String(option[..<r.lowerBound]), String(option[r.upperBound...]))
    }
}
