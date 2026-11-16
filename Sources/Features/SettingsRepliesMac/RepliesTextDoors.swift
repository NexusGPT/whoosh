import Foundation

/// The doors of Sources, Recherche, Voix and Le prompt (the long texts' editors: RepliesEditorDoors).
@MainActor
enum TextDoors {
    typealias Add = (RepliesPage, String, String, @escaping () -> Void) -> Void

    static func add(_ s: RepliesStore, _ add: Add) {
        sources(s, add)
        retrieval(s, add)
        voice(s, add)
        prompt(s, add)
        RepliesEditorDoors.add(s, add)
    }

    private static func sources(_ s: RepliesStore, _ add: Add) {
        add(.sources, "sources.add", "button") { RepliesFolderPicker.choose() }
        add(.sources, "sources.add-folder", "button") { RepliesFolderPicker.choose() }
        add(.sources, "sources.add-folder.sample", "field") { s.addFolder(RepliesDoors.sampleFolder) }
        for a in s.sources.accounts {
            add(.sources, "sources.account.\(a.accountId)", "row") { s.toggleAccount(a.accountId) }
            for (i, f) in a.folders.enumerated() { add(.sources, "sources.account.\(a.accountId).folder.\(i)", "tile") { s.toggleSource(f) } }
        }
        for (i, f) in s.sources.local.enumerated() {
            add(.sources, "sources.local.\(i)", "tile") { s.toggleSource(f) }
            if case .localFolder(let path) = f.source { add(.sources, "sources.local.\(i).remove", "menu") { s.removeFolder(path) } }
        }
        for c in s.sources.calendars { add(.sources, "sources.calendar.\(c.calendarId)", "switch") { s.toggleCalendar(c) } }
        add(.sources, "sources.scope.this_account", "segment") { s.setSearchScope(.thisAccount) }
        add(.sources, "sources.scope.all_accounts", "segment") { s.setSearchScope(.allAccounts) }
    }

    private static func retrieval(_ s: RepliesStore, _ add: Add) {
        add(.retrieval, "retrieval.try", "button") { s.tryOnMessage(.onDemand) }
        for n in RepliesChoices.perQuestion { add(.retrieval, "retrieval.per-question.\(n)", "choice") { s.setRetrieval(.excerptsPerQuestion(n: n)) } }
        for z in [ExcerptSize.sentence, .paragraph, .section] {
            add(.retrieval, "retrieval.size.\(excerptSizeKey(v: z))", "choice") { s.setRetrieval(.excerptSize(size: z)) }
        }
        add(.retrieval, "retrieval.fields", "row") { s.popover = .fields }
        for f in RepliesField.allCases { add(.retrieval, "retrieval.fields.\(f.rawValue)", "check") { s.toggleField(f) } }
        for n in RepliesChoices.keep { add(.retrieval, "retrieval.keep.\(n)", "choice") { s.setRetrieval(.keepN(n: n)) } }
        for r in [ClashRule.keepBothDated, .keepNewest, .keepOldest] {
            add(.retrieval, "retrieval.clash.\(clashRuleKey(v: r))", "choice") { s.setRetrieval(.clash(rule: r)) }
        }
        add(.retrieval, "retrieval.ai-filter", "row") { s.popover = .aiFilter }
        add(.retrieval, "retrieval.ai-filter.field", "field") { s.setRetrieval(.aiFilterQuestion(question: RepliesDoors.sampleFilter)) }
        add(.retrieval, "retrieval.ai-filter.save", "button") { s.setRetrieval(.aiFilterQuestion(question: RepliesDoors.sampleFilter)) }
        add(.retrieval, "retrieval.ai-filter.off", "button") { s.setRetrieval(.aiFilterQuestion(question: nil)) }
    }

    private static func voice(_ s: RepliesStore, _ add: Add) {
        for n in RepliesChoices.sample { add(.voice, "voice.sample.\(n)", "choice") { s.voiceSample = n } }
        add(.voice, "voice.analyse", "button") { s.analyse(s.voiceSample ?? s.voice?.sampleSize ?? 100) }
        for n in RepliesChoices.examples { add(.voice, "voice.examples.\(n)", "choice") { s.setExamples(n) } }
        add(.voice, "voice.tones", "row") { s.popover = .tones }
        for t in s.tones { add(.voice, "voice.tone.\(t.id).delete", "button") { s.deleteTone(t.id) } }
        add(.voice, "voice.tone.add", "button") { s.saveTone(RepliesDoors.sampleTone) }
    }

    private static func prompt(_ s: RepliesStore, _ add: Add) {
        add(.prompt, "prompt.preview", "button") { s.openPreview() }
        add(.prompt, "prompt.variables", "row") { s.popover = .variables }
        for v in s.variables { add(.prompt, "prompt.variable.\(v.id)", "row") { s.placeVariable(v) } }
        add(.prompt, "prompt.notes", "row") { s.popover = .notes }
        for n in s.notes { add(.prompt, "prompt.note.\(n.id).delete", "button") { s.deleteNote(n.id) } }
        add(.prompt, "prompt.note.add", "button") { s.saveNote(RepliesDoors.sampleNote) }
        add(.prompt, "prompt.versions", "row") { s.popover = .versions }
        for v in s.versions where !v.current { add(.prompt, "prompt.version.\(v.version).restore", "button") { s.restore(v.version) } }
    }
}
