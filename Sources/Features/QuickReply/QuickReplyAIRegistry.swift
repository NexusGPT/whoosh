import Foundation

/// The AI reply's doors (docs/CONTROL-SOCKET.md naming), its words through the catalogue, and its socket rows.
extension QuickReplyIds {
    /// The pill in the slab's header: "✦ Brouillon ⌘J". A click is ⌘J.
    public static let ai = "mac-main.quickreply.ai"
    public static let aiInstruction = "mac-main.quickreply.ai.instruction"
    public static let aiInstructionSubmit = "mac-main.quickreply.ai.instruction.submit"
    public static let aiChange = "mac-main.quickreply.ai.change"
    public static let aiChangeSubmit = "mac-main.quickreply.ai.change.submit"
    public static let aiDiscard = "mac-main.quickreply.ai.discard"
    /// `mac-main.quickreply.ai.option.<instruction-before|change-after>.<on|off>`: the fixture-only seam above.
    public static func aiOption(_ key: String, _ on: Bool) -> String { "mac-main.quickreply.ai.option.\(key).\(on ? "on" : "off")" }
}

extension QuickReplyCopy {
    /// The AI pill's key: one modifier and one key, shown on the pill (owner: every control with a key shows it).
    public static let aiCap = "⌘J"
    public static let returnCap = "Return"
    public static let escapeCap = "Esc"
    /// "Brouillon": the board's word for the AI mark, the same catalogue key as the header's "Brouillon à <x>".
    public static var aiMark: String { Words.text("Draft") }
    public static var aiMarkLabel: String { Words.text("AI draft") }
    public static var aiWriting: String { Words.text("Writing…") }
    public static var aiWritingPlaceholder: String { Words.text("Writing the draft…") }
    public static var aiWriteHelp: String { Words.text("Write a draft (⌘J)") }
    public static var aiRewriteHelp: String { Words.text("Write again (⌘J)") }
    /// "Consigne": one name everywhere (the Sur demande switch "Donner une consigne avant", the prompt's variable).
    public static var aiInstruction: String { Words.text("Instruction") }
    public static var aiInstructionPlaceholder: String { Words.text("accept, suggest Thursday") }
    public static var aiChangePlaceholder: String { Words.text("Say what to change: “shorter”, “without the price”…") }
    public static var aiDiscard: String { Words.text("Discard draft") }
    public static var aiFailed: String { Words.text("The draft could not be written") }
}

@MainActor
enum QuickReplyAIRegistry {
    static var model: QuickReplyModel { .shared }

    static func rows() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            QuickReplyIds.ai: { _ in
                guard model.isOpen else { return [] }
                let a = model.ai, d = a.draft
                return [["ax": QuickReplyIds.ai, "phase": phaseWord(a.phase), "mark": d == nil ? "0" : "1", "label": pillWords(a),
                         "cap": QuickReplyCopy.aiCap, "draft_id": d?.id ?? "", "made_by": d.map { replyMadeByKey(v: $0.madeBy) } ?? "",
                         "revision": Int(d?.revision ?? 0), "draft_text": d?.text ?? "", "focus": a.focus.rawValue, "writes": a.writes,
                         "instruction_field": a.phase == .asking ? "1" : "0", "change_field": a.changeFieldShown ? "1" : "0",
                         "words": a.failureWords ?? "", "discard": QuickReplyIds.aiDiscard, "discard_cap": QuickReplyCopy.escapeCap]]
            },
            QuickReplyIds.aiInstruction: { _ in
                guard model.isOpen, model.ai.phase == .asking else { return [] }
                return [["ax": QuickReplyIds.aiInstruction, "label": QuickReplyCopy.aiInstruction, "text": model.ai.instruction,
                         "placeholder": QuickReplyCopy.aiInstructionPlaceholder, "cap": QuickReplyCopy.returnCap]]
            },
            QuickReplyIds.aiChange: { _ in
                guard model.isOpen, model.ai.changeFieldShown else { return [] }
                return [["ax": QuickReplyIds.aiChange, "text": model.ai.change, "placeholder": QuickReplyCopy.aiChangePlaceholder]]
            },
        ]
    }

    static func clicks() -> [String: (AppModel) -> Bool] {
        [
            QuickReplyIds.ai: { model.aiKey($0) },
            QuickReplyIds.aiInstruction: { _ in model.aiFocus(.instruction) },
            QuickReplyIds.aiInstructionSubmit: { m in model.ai.phase == .asking && model.aiKey(m) },
            QuickReplyIds.aiChange: { _ in model.aiFocus(.change) },
            QuickReplyIds.aiChangeSubmit: { model.aiRequestChange($0) },
            QuickReplyIds.aiDiscard: { model.aiDiscard($0) },
        ].merging(optionDoors()) { a, _ in a }
    }

    /// The drive's seam for the two Sur demande switches, under the fixture core only: `SetOnDemandOption` with the value named in
    /// the id (set, never flip), so the quick reply's cases need neither the Settings window nor its doors. No control draws it.
    static func optionDoors() -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [:]
        for (key, option) in [("instruction-before", OnDemandOption.instructionBefore), ("change-after", .changeAfter)] {
            for on in [true, false] {
                t[QuickReplyIds.aiOption(key, on)] = { m in m.host.kind == .fixture && m.perform(.setOnDemandOption(option: option, enabled: on)) }
            }
        }
        return t
    }

    static func phaseWord(_ p: QuickReplyAIState.Phase) -> String {
        switch p {
        case .idle: return "idle"
        case .asking: return "asking"
        case .writing(let rewrite): return rewrite ? "rewriting" : "writing"
        case .ready: return "ready"
        case .failed: return "failed"
        }
    }

    /// The pill's words: "Brouillon" (the mark, or the offer), "Écriture…" while the model writes.
    static func pillWords(_ a: QuickReplyAIState) -> String { a.isWriting ? QuickReplyCopy.aiWriting : QuickReplyCopy.aiMark }
}

extension QuickReplyModel {
    /// A click on the instruction or the change field: the socket's `type` lands there until the reply field is clicked.
    func aiFocus(_ f: QuickReplyAIState.Focus) -> Bool {
        switch f {
        case .instruction: guard ai.phase == .asking else { return false }
        case .change: guard ai.changeFieldShown else { return false }
        case .reply: break
        }
        ai.focus = f
        header.focus = .none
        return true
    }

    /// The socket's `type` while an AI field holds the keys; false leaves it to the reply field.
    func aiType(_ text: String) -> Bool {
        switch ai.focus {
        case .instruction where ai.phase == .asking: ai.instruction += text; return true
        case .change where ai.changeFieldShown: ai.change += text; return true
        default: return false
        }
    }

    /// Return while an AI field holds the keys: the instruction writes, the change rewrites. False: the reply's own Return.
    func aiEnter(_ m: AppModel) -> Bool {
        switch ai.focus {
        case .instruction where ai.phase == .asking: return aiKey(m)
        case .change where ai.changeFieldShown: return aiRequestChange(m)
        default: return false
        }
    }
}
