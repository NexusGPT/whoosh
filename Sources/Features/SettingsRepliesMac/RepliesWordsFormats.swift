import Foundation

/// The words that carry a figure, a date or a name: numbers grouped in the app's language (« 18 096 »), the board's sentences filled.
extension RepliesWords {
    static func n(_ v: some BinaryInteger) -> String { Int(v).formatted(.number.locale(Locale(identifier: Words.current))) }
    /// round 24: « 62 % » in French, « 62% » in English: the app language's own percent.
    static func percent(_ p: some BinaryInteger) -> String {
        (Double(Int(p)) / 100).formatted(.percent.precision(.fractionLength(0)).locale(Locale(identifier: Words.current)))
    }
    /// round 24: a sentence the owner wrote, in the app language's quotation marks (“…” in English, « … » in French).
    static func quoted(_ s: String) -> String {
        let l = Locale(identifier: Words.current)
        return (l.quotationBeginDelimiter ?? "“") + s + (l.quotationEndDelimiter ?? "”")
    }

    static func kept(_ a: UInt32, of b: UInt32) -> String { Words.text("%@ of %@", n(a), n(b)) }
    static func keeps(_ a: UInt32, of b: UInt32) -> String { Words.text("keeps %@ of %@", n(a), n(b)) }
    static func yesFor(_ a: UInt32, of b: UInt32) -> String { Words.text("yes for %@ of %@", n(a), n(b)) }
    static func lastDays(_ d: UInt32, received: UInt32, drafts: UInt32) -> String {
        Words.text("Over your last %@ days: %@ messages received, %@ drafts.", n(d), n(received), n(drafts))
    }
    static func drafts(_ k: UInt32) -> String { Words.text("%@ drafts", n(k)) }
    static func lastDaysShort(_ d: UInt32) -> String { Words.text("Over your last %@ days", n(d)) }
    static func funnel(_ c: ReplyStepCounts) -> String {
        Words.text("%@ received · %@ after the entry filter · %@ by the AI · %@ drafts", n(c.received), n(c.afterEntry), n(c.afterAi), n(c.drafts))
    }
    static func sourcesOn(_ count: Int) -> String { Words.text("%@ on, set per account", n(count)) }
    static func senderIs(_ w: String) -> String { Words.text("Sender: %@", w) }

    /// « Étiquettes À répondre, Clients »; nil for all labels (nothing to say); « no label » when the list is empty.
    static func scopeLine(_ scope: ReplyLabelScope, _ labels: [MailLabel]) -> String? {
        guard case .labels(let ids) = scope else { return nil }
        let names = ids.compactMap { id in labels.first { $0.id == id }.map { $0.kind == .builtIn ? Words.text($0.name) : $0.name } }
        return names.isEmpty ? Words.text("No label") : Words.text("Labels %@", names.joined(separator: ", "))
    }

    static func foldersOf(_ on: UInt32, _ all: UInt32) -> String { Words.text("%@ folders of %@", n(on), n(all)) }
    static func foldersLine(_ a: ReplyAccountSources) -> String { "\(foldersOf(a.enabledFolders, a.totalFolders)) · \(Words.text("%@ messages", n(a.enabledItems)))" }
    /// A mail folder's server name in words (the well-known roles translated, « Clients/Oranta » as « Clients / Oranta »); a folder of this Mac by its last part.
    static func folderName(_ ref: ReplySourceRef) -> String {
        switch ref {
        case .mailFolder(_, let folder):
            switch folder.uppercased() {
            case "INBOX": return Words.text("replies.folder.inbox")   // round 24: « Inbox », the folder's usual name (the key « Inbox folder » says more elsewhere)
            case "SENT", "SENT ITEMS", "SENT MESSAGES": return Words.text("Sent")
            default: return folder.replacingOccurrences(of: "/", with: " / ")
            }
        case .localFolder(let path): return (path as NSString).lastPathComponent
        case .calendar(let id): return id
        }
    }
    static func count(_ s: ReplySource) -> String {
        if case .localFolder = s.source { return Words.text("%@ files", n(s.itemCount)) }
        return n(s.itemCount)
    }
    static func progress(_ p: ReplySyncProgress) -> String {
        switch p {
        case .notStarted: return ""
        case .syncing(let p): return percent(p)
        case .upToDate: return Words.text("up to date")
        case .failed: return Words.text("failed")
        case .gone: return goneLine   // delta 24: one key for the tile's line
        }
    }

    /// Delta 24: a mail folder the server no longer holds. One plain line on its tile, and its help.
    static var goneLine: String { Words.text("No longer on the server") }
    static var goneHelp: String { Words.text("Your click is kept for when the folder comes back.") }

    /// The one line under an editor: the revert question first, then an unknown « @mot », then « Non enregistré ».
    @MainActor static func editorLine(_ e: RepliesEditorState) -> String? {
        if e.confirmRevert { return revertAsk }
        if !e.unknown.isEmpty { return Words.text("Not a variable: %@", e.unknown.joined(separator: ", ")) }
        return e.dirty ? notSavedYet : nil
    }

    static func size(_ s: ExcerptSize) -> String {
        switch s { case .sentence: return Words.text("A sentence"); case .paragraph: return Words.text("A paragraph"); case .section: return Words.text("A section") }
    }
    static func best(_ k: UInt32) -> String { Words.text("The %@ best", n(k)) }
    static func clashRule(_ r: ClashRule) -> String {
        switch r {
        case .keepBothDated: return Words.text("Keep both, dated")
        case .keepNewest: return Words.text("Keep the newest")
        case .keepOldest: return Words.text("Keep the oldest")
        }
    }
    static func field(_ f: RepliesField) -> String {
        switch f { case .date: return Words.text("Date"); case .sender: return Words.text("Sender"); case .folder: return Words.text("Folder"); case .subject: return Words.text("Subject") }
    }
    static func isOn(_ f: RepliesField, _ x: ExcerptFields) -> Bool {
        switch f { case .date: return x.date; case .sender: return x.sender; case .folder: return x.folder; case .subject: return x.subject }
    }
    static func fieldsLine(_ x: ExcerptFields) -> String {
        let on = RepliesField.allCases.filter { isOn($0, x) }.map { field($0).lowercased() }
        guard let first = on.first else { return none }
        return ([first.prefix(1).uppercased() + first.dropFirst()] + on.dropFirst()).joined(separator: ", ")
    }

    static func latest(_ k: UInt32) -> String { Words.text("Latest %@", n(k)) }
    static func upTo(_ k: UInt32) -> String { k == 0 ? Words.text("None") : Words.text("Up to %@", n(k)) }
    static func lastAnalysis(_ v: ReplyVoice?, _ a: ReplyVoiceAnalysis) -> String {
        if case .running(let read, let of) = a { return Words.text("Reading %@ of %@ messages…", n(read), n(of)) }
        if case .failed(let why) = a { return why }
        guard let v, let at = v.analysedAtMs else { return Words.text("Never analysed.") }
        let f = DateFormatter()
        f.locale = Locale(identifier: Words.current)
        f.setLocalizedDateFormatFromTemplate("d MMM")
        return Words.text("Last analysis: %@, over %@ messages.", f.string(from: Date(timeIntervalSince1970: Double(at) / 1000)), n(v.sampleSize))
    }

    static func variablesLine(_ k: Int) -> String { Words.text("%@ available, with their meaning", n(k)) }
    static func notesLine(_ k: Int) -> String { k == 1 ? Words.text("1 note always joined") : Words.text("%@ notes always joined", n(k)) }
    static func versionLine(_ p: ReplyPrompt) -> String {
        let day = Calendar.current.isDateInToday(Date(timeIntervalSince1970: Double(p.createdAtMs) / 1000))
        return day ? Words.text("Version %@, saved today", n(p.version)) : Words.text("Version %@", n(p.version))
    }
    static func foundKept(_ found: UInt32, kept: Int, dropped: UInt32) -> String {
        Words.text("%@ excerpts found, %@ kept, %@ set aside by the AI filter.", n(found), n(kept), n(dropped))
    }
    static func stepResult(_ r: ReplyStepResult) -> String {
        switch r { case .passed: return Words.text("passed"); case .skipped: return Words.text("turned off"); case .stopped: return Words.text("stopped"); case .notReached: return Words.text("not reached") }
    }
    static func stepSymbol(_ r: ReplyStepResult) -> String {
        switch r { case .passed: return "checkmark.circle.fill"; case .skipped: return "circle.dashed"; case .stopped: return "xmark.circle.fill"; case .notReached: return "circle" }
    }
}
