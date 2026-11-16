import Foundation

/// Every control of Settings › Réponses as a click door: its socket id, the page it lives on, its kind, and the SAME setter the control calls.
/// One list feeds both `clickTargets` and the `controls` row, so a control cannot exist without a door. A text field's door types a fixed
/// sample (the socket's `type` verb reaches the window's search field only); the field itself keeps what is typed by hand.
@MainActor
enum RepliesDoors {
    struct Door { let ax: String; let page: String; let kind: String; let act: () -> Bool }

    /// The fixed samples the text doors type (French, as the board).
    static let sampleSender = "laura@example.com"
    static let sampleDescription = "Les messages d’un client qui pose une question."
    static let sampleFolder = "~/Documents/Notes"
    static let sampleFilter = "Cet extrait parle-t-il de ce client ?"
    static let sampleAppend = "Réponds en français."
    static let samplePaste = "Écris comme @tone."
    static let sampleTone = ReplyTone(id: "", name: "Chaleureux", text: "Phrases chaleureuses, je tutoie.")
    static let sampleNote = FixedNote(id: "", title: "Ton", text: "Toujours vouvoyer un nouveau client.")

    /// Typed words for the popover fields (F-rep-3): a `.focus` door picks the field and empties the buffer, the socket's `type` appends to it
    /// (SettingsRepliesMacFeature.typeText), a `.commit` door runs the field's own setter with the typed words. Beside the fixed-sample doors, which stay.
    static var typedField: String?
    static var typedWords = ""
    static func typedRow() -> [ControlRow] { [["ax": RepliesAx.id("typed"), "field": typedField ?? "", "words": typedWords]] }
    static func typeInto(_ text: String) -> Bool {
        guard typedField != nil, !text.isEmpty else { return false }
        typedWords += text; return true
    }

    static func controls(_ s: RepliesStore) -> [Door] {
        var out: [Door] = []
        func add(_ page: RepliesPage, _ ax: String, _ kind: String, _ act: @escaping () -> Void) {
            out.append(Door(ax: RepliesAx.id(ax), page: page.rawValue, kind: kind, act: { act(); return true }))
        }
        for p in RepliesPage.allCases { add(p, "page.\(p.rawValue)", "page") { RepliesRouter.go(p) } }
        add(.auto, "popover.close", "button") { s.popover = nil }
        uses(s, add)
        FilterDoors.add(s, add)
        TextDoors.add(s, add)
        return out
    }

    /// Click doors only (not in the `controls` list, which the 167-case sweep requires a case for): the typed-words doors.
    static func typedClickTargets(_ s: RepliesStore) -> [String: (AppModel) -> Bool] {
        var out: [String: (AppModel) -> Bool] = [:]
        func add(_ page: RepliesPage, _ ax: String, _ kind: String, _ act: @escaping @MainActor () -> Void) { out[RepliesAx.id(ax)] = { _ in act(); return true } }
        let fields: [(RepliesPage, String, @MainActor () -> Void)] = [
            (.filter, "filter.entry.sender", { s.setSender(typedWords) }),
            (.sources, "sources.add-folder", { s.addFolder(typedWords) }),
            (.retrieval, "retrieval.ai-filter", { s.setRetrieval(.aiFilterQuestion(question: typedWords)) }),
            (.voice, "voice.tone.add", { s.saveTone(ReplyTone(id: "", name: typedWords, text: typedWords)) }),
            (.prompt, "prompt.note.add", { s.saveNote(FixedNote(id: "", title: typedWords, text: typedWords)) }),
        ]
        for (page, ax, commit) in fields {
            add(page, "\(ax).typed.focus", "field") { typedField = ax; typedWords = "" }
            add(page, "\(ax).typed.commit", "button") { if typedField == ax, !typedWords.isEmpty { commit(); typedField = nil; typedWords = "" } }
        }
        return out
    }

    private static func uses(_ s: RepliesStore, _ add: (RepliesPage, String, String, @escaping () -> Void) -> Void) {
        add(.auto, "auto.enabled", "switch") { s.setEnabled(.auto, !(s.use(.auto)?.enabled ?? false)) }
        add(.auto, "auto.try", "button") { s.tryOnMessage(.auto) }
        for k in ["entry", "ai", "exit"] { add(.auto, "auto.step.\(k)", "row") { RepliesRouter.go(.filter) } }
        for (kind, key, page) in [(ReplyUseKind.auto, "auto", RepliesPage.auto), (.onDemand, "on-demand", .onDemand)] {
            add(page, "\(key).sources", "row") { RepliesRouter.go(.sources) }
            add(page, "\(key).prompt", "row") { RepliesRouter.go(.prompt) }
            add(page, "\(key).match-thread", "switch") { s.setMatchThread(kind, !(s.use(kind)?.matchThread ?? false)) }
            for id in ["voice"] + s.tones.map(\.id) { add(page, "\(key).tone.\(id)", "choice") { s.setTone(kind, id) } }
        }
        add(.onDemand, "on-demand.instruction-before", "switch") { s.setOption(.instructionBefore, !s.onDemand.instructionBefore) }
        add(.onDemand, "on-demand.change-after", "switch") { s.setOption(.changeAfter, !s.onDemand.changeAfter) }
    }
}

/// The doors of « Quels messages »: the three card switches, the labels, the two rules, the sender, what the AI reads and its description.
@MainActor
enum FilterDoors {
    static func add(_ s: RepliesStore, _ add: (RepliesPage, String, String, @escaping () -> Void) -> Void) {
        for (card, key) in [(FlowCard.entry, "entry"), (.ai, "ai"), (.exit, "exit")] {
            add(.filter, "filter.\(key).switch", "switch") {
                let on: Bool
                switch card { case .entry: on = s.filter?.entryEnabled ?? true; case .ai: on = s.filter?.aiEnabled ?? true; case .exit: on = s.filter?.exitEnabled ?? true }
                s.setCard(card, !on)
            }
        }
        add(.filter, "filter.scope.all", "check") { s.setScope(.all) }
        for l in s.labels { add(.filter, "filter.scope.label.\(l.id)", "check") { s.toggleLabel(l.id) } }
        for (card, key) in [(FlowCard.entry, "entry"), (.exit, "exit")] {
            add(.filter, "filter.\(key).match.all", "choice") { s.setMode(card, .all) }
            add(.filter, "filter.\(key).match.any", "choice") { s.setMode(card, .any) }
            for fact in RepliesFacts.order {
                let fk = messageFactKey(fact: fact)
                for (state, sk) in [(FactState.any, "any"), (.yes, "yes"), (.no, "no")] {
                    add(.filter, "filter.\(key).fact.\(fk).\(sk)", "segment") { s.setFact(card, fact, state) }
                }
            }
        }
        add(.filter, "filter.entry.sender", "field") { s.setSender(RepliesDoors.sampleSender) }
        add(.filter, "filter.entry.sender.clear", "button") { s.setSender("") }
        for r in RepliesReadInput.allCases { add(.filter, "filter.ai.reads.\(r.rawValue)", "check") { s.toggleRead(r) } }
    }
}
