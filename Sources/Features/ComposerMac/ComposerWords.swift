import Foundation

/// Every ax-id and every sentence of the Mac composer, in one place. Ids follow `<surface>.<region>[.<element>]`.
public enum ComposerIds {
    public static let surface = "mac-composer"
    public static let header = "mac-composer.header"
    public static let body = "mac-composer.body"
    public static let line = "mac-composer.line"
    public static let aiDraft = "mac-composer.ai-draft"
    public static let aiMarks = "mac-composer.ai-marks"
    public static let aiRun = "mac-composer.ai-draft.run"
    public static let aiAccept = "mac-composer.ai-accept"
    public static let rewrite = "mac-composer.rewrite"
    public static let rewriteChoices = "mac-composer.rewrite.choices"
    public static let sendLater = "mac-composer.send-later"
    public static let sendLaterOptions = "mac-composer.send-later.options"
    public static let tomorrowAlias = "mac-composer.send-later.tomorrow-0800"
    public static let send = "mac-composer.send"
    public static let undo = "mac-composer.undo"
    public static let undoBar = "mac-composer.undo.bar"
    public static let attach = "mac-composer.attach"
    public static let attachments = "mac-composer.attachments"
    public static let dictate = "mac-composer.dictate"
    public static let showCc = "mac-composer.cc.show"
    public static let from = "mac-composer.from"
    public static let openFull = "mac-main.reader.open-full-composer"
    public static let templates = "mac-composer.templates"
    public static let templatesSave = "mac-composer.templates.save"
    public static let snippets = "mac-composer.snippets"
    public static let snippetsSave = "mac-composer.snippets.save"
    public static let instantReply = "mac-composer.instant-reply"
    public static let instantOptions = "mac-composer.instant-reply.options"
    public static let sendDone = "mac-composer.send-done"
    public static let sendInstant = "mac-composer.send-instant"
    public static let sendMore = "mac-composer.send-more"
    public static let showBcc = "mac-composer.bcc.show"
    public static let footer = "mac-composer.footer"
    public static let devSampleFile = "mac-composer.dev.sample-file"
    /// RIG ONLY: selects all the text so a shot shows the format bar over a selection.
    public static let devSelectBody = "mac-composer.dev.select-body"
    /// how many numbered choices a list registers click targets for (the lists are shorter)
    public static let choiceSlots = 64
    public static func template(_ i: Int) -> String { "mac-composer.templates.\(i)" }
    public static func snippet(_ i: Int) -> String { "mac-composer.snippets.\(i)" }
    public static func instantChoice(_ i: Int) -> String { "mac-composer.instant-reply.\(i)" }
    public static let devWriter = "mac-composer.dev.fixture-writer"
    public static func rewriteChoice(_ style: RewriteStyle) -> String { "mac-composer.rewrite.\(style.slug)" }
    public static func fromChoice(_ i: Int) -> String { "mac-composer.from.\(i)" }
    // round 2: the field keys, the format keys, the inline lists
    public static let discard = "mac-composer.discard"
    public static let discardConfirm = "mac-composer.discard.confirm"
    public static let discardKeep = "mac-composer.discard.keep"
    public static let attachPanel = "mac-composer.attach.panel"
    public static let attachChoose = "mac-composer.attach.choose"
    public static func attachRecent(_ i: Int) -> String { "mac-composer.attach.recent.\(i)" }
    public static let format = "mac-composer.format"
    public static let draft = "mac-composer.draft"
    public static let formatBar = "mac-composer.format-bar"
    public static func formatBarButton(_ id: String) -> String { "mac-composer.format-bar.\(id)" }
    public static let controls = "mac-composer.controls"
    public static let inline = "mac-composer.inline"
    public static func inlineChoice(_ i: Int) -> String { "mac-composer.inline.\(i)" }
    public static let link = "mac-composer.link"
    public static let linkField = "mac-composer.link.field"
    public static let linkApply = "mac-composer.link.apply"
    public static let linkCancel = "mac-composer.link.cancel"
    public static let colorOptions = "mac-composer.color.options"
    public static func colorChoice(_ slug: String) -> String { "mac-composer.color.\(slug)" }
    public static let fields = "mac-composer.fields"
    public static func field(_ f: ComposerField) -> String { "mac-composer.field.\(f.rawValue)" }
    // round 3: pop-out windows
    public static let popout = "mac-composer.popout"
    public static let popIn = "mac-composer.pop-in"
    public static let thread = "mac-composer.thread"
    // round 4: remind me if no reply
    public static let remind = "mac-composer.remind"
    public static let remindOptions = "mac-composer.remind.options"
    public static let remindSet = "mac-composer.remind.set"
    public static let remindClear = "mac-composer.remind.clear"
    // round 6 (doors-compose): the doors the click-every-control sweep needs
    public static let quotedToggle = "mac-composer.quoted.toggle"
    public static let completions = "mac-composer.to.completions"
    public static func completion(_ i: Int) -> String { "mac-composer.to.completions.\(i)" }
    /// The list under To, Cc or Bcc (`field` is `to`, `cc` or `bcc`) and its lines.
    public static func completionList(_ field: String) -> String { "mac-composer.\(field).completions" }
    public static func completionLine(_ field: String, _ i: Int) -> String { "mac-composer.\(field).completions.\(i)" }
    /// The remove button of a recipient chip: `field` is `to`, `cc` or `bcc`, `i` the chip's place in its row.
    public static func chipRemove(_ field: String, _ i: Int) -> String { "mac-composer.\(field).chip.\(i).remove" }
    /// Return in the To, Cc or Bcc field: the typed address becomes a chip (the field's own submit).
    public static func fieldSubmit(_ field: String) -> String { "mac-composer.\(field).submit" }
    public static let attachmentNotes = "mac-composer.attachment-notes"
    // RIG ONLY: files for the refusals and the "being added" state
    public static let devHoldFiles = "mac-composer.dev.hold-files"
    // RIG ONLY (gap-composer-rich): a picture into the text, the From account's signature with marks or plain
    public static let devPicture = "mac-composer.dev.picture"
    public static let devRichSignature = "mac-composer.dev.rich-signature"
    public static let devPlainSignature = "mac-composer.dev.plain-signature"
    public static let devBigFile = "mac-composer.dev.big-file"
    public static let devMidFile = "mac-composer.dev.mid-file"
    public static let devGoneFile = "mac-composer.dev.gone-file"
    public static let devBadFile = "mac-composer.dev.bad-file"
    public static let devDeleteSample = "mac-composer.dev.delete-sample"
    public static let sendLaterPick = "mac-composer.send-later.pickDate"
    public static let sendLaterSchedule = "mac-composer.send-later.schedule"
    public static let remindPick = "mac-composer.remind.pickDate"
}

public enum RewriteStyle: String, CaseIterable, Sendable {
    case shorter, warmer, moreFormal = "more formal"
    public var slug: String { rawValue.replacingOccurrences(of: " ", with: "-") }
    public var title: String { Words.text(rawValue) }
}

/// What the composer says. Plain words; no code, no ids.
public enum ComposerWords {
    public static var newMessage: String { Words.text("New message") }
    public static var replyPrefix: String { Words.text("Re: ") }
    public static var to: String { Words.text("To") }
    public static var cc: String { Words.text("Cc") }
    public static var bcc: String { Words.text("Bcc") }
    public static var subject: String { Words.text("Subject") }
    public static var from: String { Words.text("From") }
    public static var bodyPlaceholder: String { Words.text("Write your message. Hold Fn to speak.") }
    public static var listening: String { Words.text("Listening") }
    public static var cleaning: String { Words.text("Cleaning up") }
    public static var aiDraft: String { Words.text("AI draft") }
    public static var aiFieldPlaceholder: String { Words.text("What should the reply say?") }
    public static var aiMark: String { Words.text("AI text. Edit it or accept it.") }
    public static var accept: String { Words.text("Accept") }
    public static var rewrite: String { Words.text("Rewrite") }
    public static var sendLater: String { Words.text("Send later") }
    public static var sendAt: String { Words.text("Send at") }
    public static var pickDate: String { Words.text("Pick a date…") }
    public static var schedule: String { Words.text("Schedule") }
    public static var reply: String { Words.text("Reply") }
    public static var send: String { Words.text("Send") }
    public static var attach: String { Words.text("Attach") }
    public static var sent: String { Words.text("Sent") }
    public static var sending: String { Words.text("Sending") }
    public static var quotedFold: String { Words.text("Show quoted text") }
    public static var quotedUnfold: String { Words.text("Hide quoted text") }
    public static var noProvider: String { Words.text("No AI provider answered. Add one in Settings: AI providers, speech and agents.") }
    public static var emptyInstruction: String { Words.text("Say what the reply should do, then press Enter.") }
    public static var nothingToSend: String { Words.text("Add a recipient and some text first.") }
    public static var notSaved: String { Words.text("This message could not be saved, so it was not sent.") }
    public static func notAnAddress(_ raw: String) -> String { Words.text("%@ is not an address.", raw) }
    public static var nothingToSave: String { Words.text("Write some text first.") }
    public static var templateSaved: String { Words.text("Saved as a template") }
    public static var snippetSaved: String { Words.text("Saved as a snippet") }
    public static var templates: String { Words.text("Templates") }
    public static var snippets: String { Words.text("Snippets") }
    public static var instantReply: String { Words.text("Instant Reply") }
    public static var saveTemplate: String { Words.text("Save this message as a template") }
    public static var saveSnippet: String { Words.text("Save the text as a snippet") }
    public static var footerOption: String { Words.text("Add “Sent with Whoosh”") }
    public static var sendAndDone: String { Words.text("Send and mark done") }
    public static var sendNow: String { Words.text("Send now, no undo") }
    public static var moreSend: String { Words.text("More ways to send") }
    public static var thinking: String { Words.text("Writing replies…") }
    public static var discardTip: String { Words.text("Discard this draft") }
    public static var attachTip: String { Words.text("Attach a file") }
    public static var dictateTip: String { Words.text("Dictate: hold Fn, speak, let go") }
    public static var rewriteTip: String { Words.text("Rewrite a paragraph") }
    public static var sendLabel: String { Words.text("Send, key command Return") }
    public static var close: String { Words.text("Close") }
    // files on the draft (delta 22): what a tile says, and the sentence of each refusal
    public static var fileAdding: String { Words.text("Adding") }
    public static var fileWillDownload: String { Words.text("Will be fetched when sent") }
    public static var fileGone: String { Words.text("Not on this Mac") }
    public static var fileTooLarge: String { Words.text("Too large") }
    static func fileState(_ s: ComposerModel.FileTile) -> String? {
        switch s {
        case .adding: return fileAdding
        case .ready: return nil
        case .willDownload: return fileWillDownload
        case .gone: return fileGone
        case .tooLarge: return fileTooLarge
        }
    }
    /// The engine's field name for a reply it cannot attach to its conversation.
    static let replyThreadField = "reply_to_thread_id"
    /// The sentence for a refusal code of the files (contract delta 22); nil for any other code.
    static func refusal(_ code: String) -> String? {
        switch code {
        case "attachment_file_gone": return Words.text("This file is no longer on this Mac.")
        case "attachment_too_large": return Words.text("This file is too large to send.")
        case "attachment_total_too_large": return Words.text("These files are too large to send together.")
        case "bad_attachment": return Words.text("This file cannot be attached.")
        case "attachment_draft_closed": return Words.text("Undo the send to change the files.")
        // A144: the engine cannot attach the reply to its conversation (its own words stay behind Details)
        case replyThreadField: return Words.text("This reply cannot be linked to its conversation yet.")
        default: return nil
        }
    }
    public static func remove(_ what: String) -> String { Words.text("Remove %@", what) }
    public static func scheduled(_ when: String) -> String { Words.text("Scheduled for %@", when) }
    public static func attached(_ name: String) -> String { Words.text("Attached %@", name) }
    public static var noContactsToMove: String { Words.text("No one in To or Cc to move.") }
    public static var remindTitle: String { Words.text("Remind me if no reply") }
    public static var remindBy: String { Words.text("Remind me if no reply by") }
    public static var remindSetButton: String { Words.text("Set the reminder") }
    public static var remindClear: String { Words.text("Remove the reminder") }
    public static func reminderSet(_ when: String) -> String { Words.text("Reminder set for %@", when) }
    public static var discardQuestion: String { Words.text("Discard this draft?") }
    public static var discard: String { Words.text("Discard") }
    public static var keep: String { Words.text("Keep") }
    public static var chooseFile: String { Words.text("Choose a file…") }
    public static var recentFiles: String { Words.text("Recent") }
    public static var linkPlaceholder: String { Words.text("Paste or type a web address") }
    public static var link: String { Words.text("Link") }
    public static var apply: String { Words.text("Apply") }
    public static var cancel: String { Words.text("Cancel") }
    public static var noAddress: String { Words.text("Type a web address first.") }
    public static func notAWebAddress(_ s: String) -> String { Words.text("%@ is not a web address.", s) }
    public static var colour: String { Words.text("Colour") }
    public static var colourDefault: String { Words.text("Default") }
    public static var colourGrey: String { Words.text("Grey") }
    public static var colourBlue: String { Words.text("Blue") }
    public static var colourOrange: String { Words.text("Orange") }
    // round 3: pop-out windows
    public static var replyAll: String { Words.text("Reply all") }
    public static var forward: String { Words.text("Forward") }
    public static var forwardPrefix: String { Words.text("Fwd: ") }
    public static var forwarded: String { Words.text("Forwarded message") }
    public static var popInTitle: String { Words.text("Pop in") }
    public static var threadTitle: String { Words.text("Thread") }
    public static var finishFirst: String { Words.text("Finish or discard this draft before starting another.") }
    public static var onlyReplyGoesBack: String { Words.text("Only a reply can go back into its thread.") }
    public static var filesStayOut: String { Words.text("Files stay in the pop-out window. Remove them to put the reply back in the thread.") }

    /// The window's title line for what it holds.
    static func title(_ kind: ComposerKind) -> String {
        switch kind {
        case .new: return newMessage
        case .reply: return reply
        case .replyAll: return replyAll
        case .forward: return forward
        }
    }

    /// The text a forward starts with: a rule, who wrote it, to whom, the subject, then the message itself.
    static func forwardBody(name: String, address: String, recipients: [String], title: String, text: String, files: [String] = []) -> String {
        let who = name.isEmpty ? address : name + " <" + address + ">"
        let list = files.isEmpty ? [] : [Words.text("Attachments") + ": " + files.joined(separator: ", ")]      // the forward lists the files it came with
        let head = ["", "", "---------- " + forwarded + " ----------",
                    from + ": " + who, to + ": " + recipients.joined(separator: ", "), subject + ": " + title]
        return (head + list + ["", text]).joined(separator: "\n")
    }
}
