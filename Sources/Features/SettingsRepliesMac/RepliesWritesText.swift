import Foundation

/// The writes of Sources, Voix and Le prompt, and the in-place editor every long text shares.
extension RepliesStore {
    // MARK: Sources

    func toggleSource(_ s: ReplySource) {
        guard s.isGone else { run(.setReplySourceEnabled(source: s.source, enabled: !s.enabled)); return }
        // a gone folder stays off: the click flips the owner's remembered choice for its return, as the core holds it (delta 24, `owner_enabled`)
        run(.setReplySourceEnabled(source: s.source, enabled: !(sources.wish(s) ?? false)))
    }
    func toggleCalendar(_ c: ReplyCalendar) { run(.setReplySourceEnabled(source: .calendar(calendarId: c.calendarId), enabled: !c.enabled)) }
    func addFolder(_ path: String) { run(.addReplyLocalFolder(path: path)) }
    func removeFolder(_ path: String) { run(.removeReplyLocalFolder(path: path)) }
    func setSearchScope(_ scope: RetrievalScope) { setRetrieval(.scope(scope: scope)) }
    func toggleAccount(_ id: String) { openedByDefault = true; if openAccounts.contains(id) { openAccounts.remove(id) } else { openAccounts.insert(id) } }

    // MARK: Voix

    func analyse(_ sample: UInt32) { run(.runVoiceAnalysis(sampleSize: sample)) }
    func setExamples(_ n: UInt32) { run(.setVoiceSamePersonExamples(count: n)) }
    func saveTone(_ tone: ReplyTone) { run(.saveReplyTone(tone: tone)) }
    func deleteTone(_ id: String) { run(.deleteReplyTone(toneId: id)) }

    // MARK: Le prompt

    func restore(_ version: UInt32) { run(.restoreReplyPromptVersion(promptId: prompt?.promptId ?? "replies", version: version)) }
    func saveNote(_ note: FixedNote) { run(.saveFixedNote(note: note)) }
    func deleteNote(_ id: String) { run(.deleteFixedNote(noteId: id)) }

    /// The check's seam: the panes' own state back to its start, then one read of the core.
    func resetForTest() {
        popover = nil; editors = [:]; newNoteTitle = ""; voiceSample = nil; trial = nil; error = nil; openAccounts = []; openedByDefault = false; page = .auto
        load()
    }
}
