import Foundation

/// The provider behind "AI draft" and "Rewrite". The agents core is not served to Swift yet, so the default writer says
/// so in words and writes nothing. The owner's text is never touched when a provider fails (J3.3).
public protocol ComposerWriter: Sendable {
    func draft(instruction: String, to: [String], subject: String, fromAddress: String) async throws -> String
    func rewrite(_ paragraph: String, as style: RewriteStyle) async throws -> String
    /// Instant Reply: a few whole replies to the message, each a different way to answer it.
    func instantReplies(to message: String, subject: String) async throws -> [String]
}

struct ComposerWriterError: Error { let words: String }

struct NoProviderWriter: ComposerWriter {
    func draft(instruction: String, to: [String], subject: String, fromAddress: String) async throws -> String { throw ComposerWriterError(words: ComposerWords.noProvider) }
    func rewrite(_ paragraph: String, as style: RewriteStyle) async throws -> String { throw ComposerWriterError(words: ComposerWords.noProvider) }
    func instantReplies(to message: String, subject: String) async throws -> [String] { throw ComposerWriterError(words: ComposerWords.noProvider) }
}

/// RIG ONLY, installed by the click `mac-composer.dev.fixture-writer`: answers with the J3 fixture's texts so a shot and
/// a drive can show the AI mark and the rewrite. It is not a provider and ships nothing to the owner.
struct FixtureWriter: ComposerWriter {
    func instantReplies(to message: String, subject: String) async throws -> [String] {
        [Words.text("Yes, that works for me. Thank you."), Words.text("Thank you. I need a little more time and will reply by tomorrow."), Words.text("I cannot do this, sorry. Could we find another way?")]
    }
    func draft(instruction: String, to: [String], subject: String, fromAddress: String) async throws -> String {
        "Hi Karen,\n\nThank you for the quote. The price works for us, and we would like delivery confirmed for March. Please also confirm the signed quote reaches our accounts team before the end of the week so the press slot is held.\n\nAssem"
    }
    func rewrite(_ paragraph: String, as style: RewriteStyle) async throws -> String {
        switch style {
        case .shorter: return "The price works for us. Please confirm delivery in March."
        case .warmer: return "Thank you so much for the quote. The price works well for us, and we would love delivery confirmed for March."
        case .moreFormal: return "We confirm that the proposed price is acceptable and request written confirmation of delivery in March."
        }
    }
}

extension ComposerModel {
    public func useFixtureWriter() { writer = FixtureWriter() }

    public func openAIField(_ m: AppModel) -> Bool {
        guard isOpen, !isCounting else { return false }
        let open = !aiFieldOpen; closePanels(); aiFieldOpen = open; m.typed = ""; aiInput = ""
        return true
    }

    /// Enter in the instruction field. An empty instruction writes nothing and says what to do.
    public func runDraft(_ m: AppModel) -> Bool {
        let ins = (aiInput.isEmpty ? m.typed : aiInput).trimmingCharacters(in: .whitespacesAndNewlines)
        guard aiFieldOpen else { return false }
        guard !ins.isEmpty else { line = ComposerWords.emptyInstruction; return true }
        aiBusy = true; line = ""
        let (w, to, subj, from) = (writer, to, subject, self.from?.address ?? "")
        Task { @MainActor in
            defer { self.aiBusy = false }
            do {
                let text = try await w.draft(instruction: ins, to: to, subject: subj, fromAddress: from)
                self.session.setPlain(text); self.aiText = text; self.aiFieldOpen = false; self.aiInput = ""; m.typed = ""
                (m.core as? FixtureCore)?.noteAIJob(voiceProfileAddress: from)   // the fixture core's `ai_job/last/*` (J3.3); the real core records its own jobs
                _ = self.focus(.body)    // the field that held the keys is gone: the cursor goes back to the text, so the owner's next key edits it (J3.3 edit-lands-in-the-draft)
            } catch { self.line = Self.writerWords(error) }
        }
        return true
    }

    public func acceptAI() -> Bool { guard aiMarkActive else { return false }; aiText = nil; return true }

    public func openRewrite() -> Bool {
        guard isOpen, !isCounting, rewriteTarget != nil else { return false }
        let open = !rewriteOpen; closePanels(); rewriteOpen = open
        return true
    }

    /// The paragraph the rewrite acts on: the longest one (the editor does not report a selection yet).
    var rewriteTarget: Int? {
        let ps = session.document.paragraphs
        return ps.indices.filter { !ps[$0].text.isEmpty }.max { ps[$0].text.count < ps[$1].text.count }
    }

    /// Pick "shorter", "warmer" or "more formal": the paragraph is replaced and ⌘Z brings the old one back (J3.4).
    public func rewrite(_ style: RewriteStyle) -> Bool {
        guard let i = rewriteTarget else { return false }
        let old = session.document.paragraphs[i].text
        rewriteOpen = false; aiBusy = true
        let w = writer
        Task { @MainActor in
            defer { self.aiBusy = false }
            do { let text = try await w.rewrite(old, as: style); _ = self.session.applyRewrite(paragraph: i, text: text); self.line = "" }
            catch { self.line = Self.writerWords(error) }
        }
        return true
    }

    static func writerWords(_ e: Error) -> String { (e as? ComposerWriterError)?.words ?? "\(e)" }
}
