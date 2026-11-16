import Foundation
import Observation

/// The iPhone composer's one state object. The message itself (who it is from and to, the subject, the body, the files, where a send stands)
/// is the Mac composer's `ComposerModel.shared`, so a draft is saved and sent through the core exactly as on the Mac; this holds only what
/// the phone screen adds: which field has the cursor, the From list, the file picker, the folded quote and the surface to go back to.
@MainActor @Observable
public final class ComposerPhoneState {
    public static let shared = ComposerPhoneState()

    public enum Field: String { case to, cc, bcc, subject, body }

    public var focus: Field?
    public var fromOpen = false
    public var attaching = false
    public var quotedOpen = false
    private(set) var returnSurface = "iphone-inbox"

    private init() {}

    var model: ComposerModel { .shared }
    var speech: PhoneSpeech { .shared }

    // MARK: open and close

    /// A new message, or a reply to `messageId`. A draft the owner left open comes back as it was (the Mac's rule: finish it first).
    @discardableResult
    public func open(reply messageId: String?, _ m: AppModel) -> Bool {
        if m.surface != ComposerPhoneIds.surface { returnSurface = m.surface }
        let ok = messageId.map { model.openReply(m, messageId: $0) } ?? model.openNew(m)
        settle(m, reply: messageId != nil)
        return ok
    }

    /// The shell shows the composer for the overlay `iphone-composer`: a reply from the open thread, else a new message.
    func appeared(_ m: AppModel) {
        guard !model.isOpen || m.surface != ComposerPhoneIds.surface else { return }
        let fromThread = m.surface == "iphone-thread"
        open(reply: fromThread ? m.focusedMessageId : nil, m)
    }

    /// `ComposerModel.present` names the Mac's window slot; on the phone the surface and the overlay are this screen's.
    private func settle(_ m: AppModel, reply: Bool) {
        m.surface = ComposerPhoneIds.surface
        m.overlay = ComposerPhoneIds.surface
        focus = reply ? .body : .to
        fromOpen = false; attaching = false; quotedOpen = false
    }

    /// Cancel and the end of "Sent": the screen goes, the draft stays in the model as the Mac's Escape leaves it.
    func close(_ m: AppModel) {
        guard !model.isCounting else { return }
        model.isOpen = false
        fromOpen = false; attaching = false; focus = nil
        m.overlay = nil
        m.surface = returnSurface
    }

    // MARK: From

    func accountChoices(_ m: AppModel) -> [PhoneAccount] {
        let failing = PhoneAccounts.failingIds(m)
        return model.accounts.map {
            PhoneAccount(id: $0.id, name: $0.displayName, address: $0.address, colour: $0.colour, group: $0.group, iconId: $0.iconId,
                         needsAttention: failing.contains($0.id))
        }
    }

    func current(_ m: AppModel) -> PhoneAccount? { accountChoices(m).first { $0.id == model.fromId } }

    @discardableResult
    func pickFrom(_ index: Int, _ m: AppModel) -> Bool {
        let all = model.accounts
        guard model.isOpen, !model.isCounting, all.indices.contains(index) else { return false }
        model.fromId = all[index].id
        fromOpen = false
        return true
    }

    // MARK: send, undo, the beat

    @discardableResult
    func send(_ m: AppModel) -> Bool {
        guard model.isOpen, !model.isCounting else { return false }
        if !model.toInput.isEmpty { _ = model.commitRecipient() }
        // The core's quick reply goes out from the thread's own account. A reply from another account goes through the core's draft, saved
        // with that From, and a refused save says why and sends nothing (never a send from the wrong account).
        let original = model.replyMessageId.flatMap { ComposerContext.reply(messageId: $0, core: m.core)?.accountId }
        if model.threadId != nil, model.coreDraftId == nil, model.canSend, let original, original != model.fromId {
            do {
                let saved = try m.core.run(command: .saveDraft(fromAccountId: model.fromId, to: model.to, cc: model.cc, subject: model.subject,
                                                               body: model.outgoing, replyToThreadId: model.threadId)).createdId
                guard let d = saved else { throw CoreError.Invalid(field: "draft", reasonWords: ComposerWords.notSaved) }
                model.coreDraftId = d
            } catch {
                model.line = ComposerModel.words(error)
                return true
            }
        }
        return model.send(m)
    }

    @discardableResult
    func undoSend(_ m: AppModel) -> Bool { model.undo(m) }

    /// editing, sending (the undo window), sent, failed, scheduled.
    var phaseWord: String {
        switch model.phase {
        case .editing: "editing"
        case .counting: "sending"
        case .sent: "sent"
        case .failed: "failed"
        case .scheduled: "scheduled"
        }
    }

    var isSent: Bool { model.phase == .sent }

    func secondsLeft(at now: Date = Date()) -> Int {
        if case .counting(_, let c) = model.phase { return c.secondsLeft(at: now) }
        return 0
    }

    // MARK: text

    /// Text from the socket's `type` or a hardware keyboard: it goes to the field that has the cursor, else the body.
    func type(_ text: String) -> Bool {
        guard model.isOpen, !model.isCounting, !text.isEmpty else { return false }
        switch focus {
        case .to: model.toInput += text
        case .cc: model.ccInput += text
        case .bcc: model.bccInput += text
        case .subject: model.subject += text
        case .body, nil: append(text, spaced: false)
        }
        return true
    }

    /// Dictation finished: the words join the end of the body, a space between when the last character is not one.
    func insertDictated(_ text: String) {
        guard model.isOpen, !text.isEmpty else { return }
        append(text, spaced: true)
        speech.delivered(text)
    }

    /// The new words go on the last paragraph as a plain run, so what is already written keeps its formatting.
    private func append(_ text: String, spaced: Bool) {
        let session = model.session
        var doc = session.document
        let last = doc.paragraphs.last?.runs.last?.text.last
        let lead = spaced && last != nil && last?.isWhitespace == false ? " " : ""
        if doc.paragraphs.isEmpty { doc.paragraphs = [RichParagraph(lead + text)] }
        else { doc.paragraphs[doc.paragraphs.count - 1].runs.append(RichRun(lead + text)) }
        session.viewDidChange(doc)
        session.load(html: session.mimeHTML)   // the text view reloads from the document when its revision moves, and only `load` moves it from outside
    }
}
