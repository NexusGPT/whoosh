import Foundation
import SwiftUI

/// The composermac lane's registration: rows, clicks, keys and palette commands for the Mac composer (J3.1 to J3.8).
/// The views and the model live in this folder; the shell's surface window draws `ComposerView` through `view(for:)`.
@MainActor
public enum ComposerMacFeature: FeatureModule {
    public static let id = "composermac"
    public static let surfaces: [String] = ["mac-composer"]
    static var model: ComposerModel { .shared }

    /// The notch's file drop (`NotchModel.fileSink`, docs/lanes/notch.md N4): a file goes to the open composer, or to a new one.
    public static func acceptFile(_ url: URL) {
        if !model.isOpen { _ = model.openNew(.shared) }
        _ = model.attach(url)
    }

    /// Focus is the composer's window: its surface is current, or its window is the key window (a shell window beside it
    /// keeps its own keys).
    public static func hasFocus(_ m: AppModel) -> Bool {
        guard model.isOpen else { return false }
        #if os(macOS)
        if WindowBridge.window(for: ComposerContext.slot)?.isKeyWindow == true { return true }
        #endif
        return m.surface == ComposerIds.surface
    }

    /// The lines of the list under a recipient field, as drawn: at most five, the highlighted one marked.
    private static func completionRows(_ field: ComposerModel.RecipientField, _ name: String) -> [ControlRow] {
        guard model.isOpen else { return [] }
        return Array(model.offered(field).prefix(5)).enumerated().map {
            ["ax": ComposerIds.completionLine(name, $0.offset), "name": $0.element.name, "address": $0.element.address, "highlighted": $0.offset == model.completionIndex]
        }
    }

    public static func view(for slot: SurfaceSlot) -> AnyView? {
        #if os(macOS)
        ComposerKeyboard.installOnce()   // the title's words and the ring's field, before the window's view is first drawn
        #endif
        return slot == ComposerContext.slot ? AnyView(ComposerView()) : nil
    }

    /// `type <text>` while the composer holds the keys: the text lands at the caret in the same turn, so a key sent right after
    /// it (Escape, Return) finds the text and the `;` or `:` list it opened. Before, the text went through the shared buffer and
    /// a SwiftUI onChange on a later turn; Escape could arrive first and close the composer with no list open. The AI field
    /// keeps the buffer (it is a SwiftUI text field bound to it).
    public static func typeText(_ text: String, _ m: AppModel) -> Bool {
        guard !text.isEmpty, model.isOpen, !model.aiFieldOpen, hasFocus(m) else { return false }
        model.insertAtCaret(text)
        return true
    }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            ComposerIds.header: { _ in
                guard model.isOpen else { return [] }
                model.syncFiles()
                var row: ControlRow = ["draft_id": model.coreDraftId ?? "", "reply_message_id": model.replyMessageId ?? "", "files_held": model.filesHeld, "panel": model.panelWord, "to_input": model.toInput, "cc_input": model.ccInput, "bcc_input": model.bccInput,
                                       "date_picking": model.datePicking, "date_refusal": model.dateRefusal ?? ""]   // doors-compose: what the sweep reads back
                row.merge(["ax": ComposerIds.header, "from": model.from?.address ?? "", "from_name": model.from?.displayName ?? "",
                         "signature": model.signature, "outgoing": model.outgoing,
                         "signature_rich": ComposerRichMail.isRich(model.signature), "pictures": ComposerRichMail.pictureIds(model.session.document),
                         "picture_sources": ComposerRichMail.pictureIds(model.session.document).map { RichPictures.shared.file(for: $0)?.path ?? "" },
                         "bcc": model.bcc, "bcc_visible": model.showBcc, "cc_visible": model.showCc, "footer_on": model.library.footerOn, "has_draft": model.hasDraft, "is_open": model.isOpen,
                         "quoted_folded": model.quotedFolded, "quoted_count": model.quoted.count,
                         "to": model.to, "cc": model.cc, "subject": model.subject, "accounts": model.accounts.count,
                         "focused": model.focusedWord, "ring": model.focusedWord, "ring_motion": model.ringField == nil ? "none" : (model.ringSlide ? "slide" : "fade"),
                         "from_open": model.fromOpen, "attach_open": model.attachOpen, "discard_ask": model.discardAsk, "link_open": model.linkOpen,
                         "color_open": model.colorOpen, "remind_open": model.remindOpen, "reminder_ms": model.reminderMs, "reminder_words": model.reminderWords, "line": model.line, "bcc_count": model.bcc.count, "to_count": model.to.count]) { a, _ in a }
                return [row]
            },
            ComposerIds.body: { _ in model.isOpen ? [["ax": ComposerIds.body, "text": model.bodyText,
                                                       "editor_text": model.session.hooks.viewText() ?? NSNull()]] : [] },   // the editor's own text: it must be the copy's
            ComposerIds.aiMarks: { _ in
                guard model.aiMarkActive, let t = model.aiText else { return [] }
                return [["ax": ComposerIds.aiMarks, "words": t, "start": 0, "end": t.count]]
            },
            ComposerIds.rewriteChoices: { _ in
                model.rewriteOpen ? RewriteStyle.allCases.map { ["ax": ComposerIds.rewriteChoice($0), "title": $0.title] } : []
            },
            ComposerIds.undoBar: { _ in
                switch model.phase {
                case .counting(let id, let c): return [["ax": ComposerIds.undoBar, "seconds_left": c.secondsLeft(at: Date()), "text": ComposerWords.sending, "undo_offered": true, "outbox_id": id]]
                case .sent: return [["ax": ComposerIds.undoBar, "seconds_left": 0, "text": ComposerWords.sent, "undo_offered": false]]
                default: return []
                }
            },
            ComposerIds.sendLaterOptions: { _ in
                guard model.sendLaterOpen else { return [] }
                return TimeInWords.options(now: Date()).map { ["ax": ComposerIds.sendLater + "." + $0.kind.rawValue, "title": $0.title, "resolves_to": $0.when?.description ?? "", "refused": $0.refusal != nil] }
            },
            ComposerIds.remindOptions: { _ in
                guard model.remindOpen else { return [] }
                return TimeInWords.options(now: Date()).map { ["ax": ComposerIds.remind + "." + $0.kind.rawValue, "title": $0.title, "resolves_to": $0.when?.description ?? "", "refused": $0.refusal != nil] }
            },
            ComposerIds.templates: { _ in
                model.templatesOpen ? model.library.templates.enumerated().map { ["ax": ComposerIds.template($0.offset), "title": $0.element.name, "subject": $0.element.subject, "files": $0.element.files.count] } : []
            },
            ComposerIds.snippets: { _ in
                model.snippetsOpen ? model.library.snippets.enumerated().map { ["ax": ComposerIds.snippet($0.offset), "title": $0.element.name, "text": $0.element.text] } : []
            },
            ComposerIds.instantOptions: { _ in
                model.instantOpen ? model.instantOptions.enumerated().map { ["ax": ComposerIds.instantChoice($0.offset), "text": $0.element] } : []
            },
            ComposerIds.line: { _ in [["ax": ComposerIds.line, "text": model.line]] },
            ComposerIds.format: { _ in model.isOpen ? [model.formatRow] : [] },
            ComposerIds.draft: { _ in [["ax": ComposerIds.draft, "kept": model.draftKey != nil, "body_chars": model.bodyText.count, "subject_chars": model.subject.count]] },
            ComposerIds.inline: { _ in
                model.inlineItems.enumerated().map { i, it in
                    ["ax": ComposerIds.inlineChoice(i), "kind": model.session.triggerState?.trigger.kind.rawValue ?? "", "title": it.title, "glyph": it.glyph,
                     "text": it.text, "highlighted": i == model.inlineIndex]
                }
            },
            ComposerIds.controls: { _ in model.isOpen ? model.controlRows : [] },
            ComposerIds.attachPanel: { _ in
                guard model.attachOpen else { return [] }
                return [["ax": ComposerIds.attachChoose, "title": ComposerWords.chooseFile, "highlighted": model.listIndex == 0]]
                    + model.library.recentFiles.enumerated().map { ["ax": ComposerIds.attachRecent($0.offset), "title": URL(fileURLWithPath: $0.element).lastPathComponent, "highlighted": model.listIndex == $0.offset + 1] }
            },
            ComposerIds.colorOptions: { _ in
                model.colorOpen ? ComposerColor.all.enumerated().map { ["ax": ComposerIds.colorChoice($0.element.slug), "name": $0.element.slug, "title": $0.element.title, "highlighted": model.listIndex == $0.offset] } : []
            },
            ComposerIds.link: { _ in model.linkOpen ? [["ax": ComposerIds.link, "text": model.linkInput]] : [] },
            ComposerIds.completions: { _ in completionRows(.to, "to") },
            ComposerIds.completionList("cc"): { _ in completionRows(.cc, "cc") },
            ComposerIds.completionList("bcc"): { _ in completionRows(.bcc, "bcc") },
            ComposerIds.popout: { _ in [model.popoutRow] },
            ComposerIds.attachments: { _ in
                model.syncFiles()
                return model.attachments.map { ["ax": ComposerIds.attachments, "id": $0.id, "name": $0.name, "size": $0.sizeWords, "size_bytes": Int($0.sizeBytes), "state": $0.state.rawValue,
                                                 "source_path": $0.url?.path ?? ""] }   // where the file the tile sends lies (a pasted picture: the app's data folder)
            },
            ComposerIds.attachmentNotes: { _ in
                model.syncFiles()
                return model.attachments.compactMap { a in ComposerWords.fileState(a.state).map { ["ax": ComposerIds.attachmentNotes, "name": a.name, "words": $0] } }
            },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [
            ComposerIds.dictate: { _ in toggleDictation(); return true },   // the socket reaches a control through this table, not through its SwiftUI action
            ComposerIds.attach: { _ in model.toggleAttachPanel() },
            ComposerIds.attachChoose: { _ in model.pickAttachRow(0) },
            ComposerIds.discard: { model.requestDiscard($0) },
            ComposerIds.discardConfirm: { model.confirmDiscard($0) },
            ComposerIds.discardKeep: { _ in model.dismissPanels() },
            ComposerIds.linkApply: { _ in model.applyLink() },
            ComposerIds.linkCancel: { _ in model.linkOpen && model.dismissPanels() },
            ComposerIds.from: { _ in model.isOpen && (model.fromOpen ? model.closeFromList() : model.focus(.from)) },
            ComposerIds.openFull: { model.openReply($0) },
            ComposerIds.popIn: { model.popIn($0) },
            ComposerIds.thread: { model.switchDraft($0) },
            ComposerIds.aiDraft: { model.openAIField($0) },
            ComposerIds.aiRun: { model.runDraft($0) },
            ComposerIds.aiAccept: { _ in model.acceptAI() },
            ComposerIds.rewrite: { _ in model.openRewrite() },
            ComposerIds.sendLater: { _ in
                guard model.isOpen else { return false }
                let open = !model.sendLaterOpen; model.closePanels(); model.sendLaterOpen = open; return true
            },
            ComposerIds.tomorrowAlias: { m in tomorrow(m) },
            ComposerIds.remind: { _ in model.toggleRemind() },
            ComposerIds.remindClear: { _ in model.clearReminder() },
            ComposerIds.send: { model.send($0) },
            ComposerIds.undo: { model.undo($0) },
            ComposerIds.showCc: { _ in model.focus(.cc) },
            ComposerIds.showBcc: { _ in model.focus(.bcc) },
            ComposerIds.templates: { _ in model.toggleTemplates() },
            ComposerIds.templatesSave: { _ in model.saveTemplate() },
            ComposerIds.snippets: { _ in model.toggleSnippets() },
            ComposerIds.snippetsSave: { _ in model.saveSnippet() },
            ComposerIds.instantReply: { _ in model.toggleInstantReply() },
            ComposerIds.sendDone: { model.sendAndMarkDone($0) },
            ComposerIds.sendInstant: { model.sendInstantly($0) },
            ComposerIds.sendMore: { _ in model.closePanelsExcept(more: true); return model.isOpen },
            ComposerIds.footer: { _ in model.toggleFooter() },
            ComposerIds.devWriter: { _ in model.useFixtureWriter(); return true },
            ComposerIds.devSampleFile: { _ in sampleFile() },
            ComposerIds.devPicture: { _ in model.rigPicture() },
            // RIG ONLY: the Edit menu's Redo, as its item sends it (`redo:` up the composer's responder chain); ⇧⌘Z in the text is instant send
            "mac-composer.dev.redo": { _ in
                #if os(macOS)
                return WindowBridge.window(for: ComposerContext.slot)?.firstResponder?.tryToPerform(Selector(("redo:")), with: nil) ?? false
                #else
                return false
                #endif
            },
            ComposerIds.devRichSignature: { _ in model.rigSignature(rich: true) },
            ComposerIds.devPlainSignature: { _ in model.rigSignature(rich: false) },
            ComposerIds.devHoldFiles: { _ in model.filesHeld.toggle(); if !model.filesHeld { model.settleFiles() }; return true },
            ComposerIds.devBigFile: { _ in rigFile("whoosh-big-file.bin", megabytes: 11) },
            ComposerIds.devMidFile: { _ in midFile() },
            ComposerIds.devGoneFile: { _ in acceptFile(FileManager.default.temporaryDirectory.appendingPathComponent("whoosh-gone-file.txt")); return true },
            ComposerIds.devBadFile: { _ in model.isOpen && model.attach(URL(string: "relative-file.txt") ?? FileManager.default.temporaryDirectory) },
            ComposerIds.devDeleteSample: { _ in (try? FileManager.default.removeItem(at: FileManager.default.temporaryDirectory.appendingPathComponent("whoosh-sample-quote.txt"))) != nil },
            ComposerIds.devSelectBody: { _ in model.isOpen && { model.session.hooks.selectAll(); return true }() },
        ]
        for s in RewriteStyle.allCases { t[ComposerIds.rewriteChoice(s)] = { _ in model.rewriteOpen && model.rewrite(s) } }
        t[ComposerIds.sendLater + "." + SnoozeOption.Kind.tomorrowMorning.rawValue] = { m in tomorrow(m) }
        for kind in [SnoozeOption.Kind.laterToday, .tomorrowMorning, .nextWeek] {
            t[ComposerIds.remind + "." + kind.rawValue] = { m in
                guard model.remindOpen, let o = TimeInWords.options(now: Date()).first(where: { $0.kind == kind }), o.refusal == nil, let d = o.when else { return false }
                return model.setReminder(d, m)
            }
        }
        // round 6 (doors-compose): the doors the click-every-control sweep added
        t[ComposerIds.quotedToggle] = { _ in model.toggleQuoted() }
        t[ComposerIds.sendLaterPick] = { _ in model.sendLaterOpen && model.toggleDatePicker() }
        t[ComposerIds.sendLaterSchedule] = { m in model.sendLaterOpen && model.datePicking && model.scheduleChosen(m) }
        t[ComposerIds.remindPick] = { _ in model.remindOpen && model.toggleDatePicker() }
        t[ComposerIds.remindSet] = { m in model.remindOpen && model.datePicking && model.setChosenReminder(m) }
        for kind in [SnoozeOption.Kind.laterToday, .nextWeek] { t[ComposerIds.sendLater + "." + kind.rawValue] = { m in model.scheduleOption(kind, m) } }
        for f in ["to", "cc", "bcc"] { for i in 0..<ComposerIds.choiceSlots { t[ComposerIds.chipRemove(f, i)] = { _ in model.removeChip(f, i) } } }
        t[ComposerIds.fieldSubmit("to")] = { _ in model.commitRecipient(.to) }
        t[ComposerIds.fieldSubmit("cc")] = { _ in model.commitRecipient(.cc) }
        t[ComposerIds.fieldSubmit("bcc")] = { _ in model.commitRecipient(.bcc) }
        for i in 0..<5 {
            t[ComposerIds.completion(i)] = { _ in model.pickCompletion(i) }
            t[ComposerIds.completionLine("cc", i)] = { _ in model.pickCompletion(i, .cc) }
            t[ComposerIds.completionLine("bcc", i)] = { _ in model.pickCompletion(i, .bcc) }
        }
        for i in 0..<64 { t[ComposerIds.fromChoice(i)] = { _ in
            guard model.accounts.indices.contains(i) else { return false }
            model.fromId = model.accounts[i].id; model.fromOpen = false; return true
        } }
        for i in 0..<ComposerIds.choiceSlots {
            t[ComposerIds.inlineChoice(i)] = { _ in model.pickInline(i) }
            t[ComposerIds.attachRecent(i)] = { _ in model.pickAttachRow(i + 1) }
            t[ComposerIds.template(i)] = { _ in model.applyTemplate(i) }
            t[ComposerIds.snippet(i)] = { _ in model.insertSnippet(i) }
            t[ComposerIds.instantChoice(i)] = { _ in model.pickInstantReply(i) }
        }
        for c in ComposerColor.all { t[ComposerIds.colorChoice(c.slug)] = { _ in model.pickColor(c.name) } }
        #if canImport(AppKit)
        for item in RichTextToolbar.items.flatMap({ $0 }) {
            t[ComposerIds.formatBarButton(item.id)] = { _ in model.runFormat(item.act) }
        }
        #endif
        return t
    }

    /// Keys by focus: the shell asks the composer first while its window holds the focus, so these never reach the
    /// quick reply. A handler that does nothing in the current state returns false and the key passes on.
    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        #if os(macOS)
        ComposerKeyboard.installOnce()
        #endif
        let own: [String: (AppModel) -> Bool] = [
            "⌘Enter": { model.isOpen ? model.send($0) : false },
            "⌘⇧Enter": { model.isOpen ? model.sendAndMarkDone($0) : false },
            "⌘⇧Z": { model.isOpen ? model.sendInstantly($0) : false },
            "⌘;": { _ in model.toggleSnippets() },
            "⌘⇧B": { _ in model.focus(.bcc) },
            "⌘⇧C": { _ in model.focus(.cc) },
            "⌘⇧L": { m in
                if !model.isOpen && !model.openReply(m) { return false }
                model.closePanels(); model.sendLaterOpen = true; return true
            },
            "Enter": { model.isOpen && (model.inlineKey(.accept) || model.listReturn($0)) ? true : (model.aiFieldOpen ? model.runDraft($0) : false) },
            "Z": { model.isCounting ? model.undo($0) : false },
            "Escape": { model.close($0) },
        ]
        return own.merging(ComposerCommands.listKeys) { a, _ in a }
    }

    public static func paletteCommands() -> [(title: String, key: String)] {
        [(Words.text("New message"), "C"), (Words.text("AI draft"), ""), (Words.text("Rewrite a paragraph"), ""), (Words.text("Send later"), "⌘⇧L"),
         (Words.text("Send"), "⌘Enter"), (Words.text("Snippets"), "⌘;"), (Words.text("Send and mark done"), "⌘⇧Enter"), (Words.text("Instant Reply"), "")]
    }

    private static func tomorrow(_ m: AppModel) -> Bool {
        guard model.isOpen, let d = TimeInWords.options(now: Date()).first(where: { $0.kind == .tomorrowMorning })?.when else { return false }
        return model.schedule(d, m)
    }

    /// The Dictate button: what the Fn key pair does (DictationFeature.keyBindings). Idle starts a session, listening ends it and hands the
    /// words to the field that holds the keys; cleaning (a session finishing) is left alone.
    @MainActor static func toggleDictation() {
        switch Dictation.shared.state {
        case .idle: Task { @MainActor in await Dictation.shared.begin() }
        case .listening: Task { @MainActor in await Dictation.shared.end() }
        default: break
        }
    }

    private static var rigFileCount = 0
    private static func midFile() -> Bool { rigFileCount += 1; return rigFile("whoosh-mid-file-\(rigFileCount).bin", megabytes: 8) }

    /// RIG ONLY (`mac-composer.dev.big-file`, `.mid-file`): a sparse file of that many MiB in the temporary folder, attached.
    private static func rigFile(_ name: String, megabytes: Int) -> Bool {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(name)
        guard FileManager.default.createFile(atPath: url.path, contents: nil), let h = try? FileHandle(forWritingTo: url) else { return false }
        defer { try? h.close() }
        guard (try? h.truncate(atOffset: UInt64(megabytes) * 1_048_576)) != nil else { return false }
        acceptFile(url)
        return true
    }

    /// RIG ONLY (`mac-composer.dev.sample-file`): writes a small file and attaches it, standing for a file drop.
    private static func sampleFile() -> Bool {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("whoosh-sample-quote.txt")
        guard (try? "Sample quote".write(to: url, atomically: true, encoding: .utf8)) != nil else { return false }
        acceptFile(url)
        return true
    }
}

/// "compose" and "ai-draft" through the shortcuts table; "reply" stays the quick reply's (R opens the field, the full
/// composer opens from `mac-main.reader.open-full-composer`).
extension ComposerMacFeature: CommandProviding {
    public static func commands() -> [String: (AppModel) -> Bool] {
        let own: [String: (AppModel) -> Bool] = [
            "compose": { model.openNew($0) },
            "ai-draft": { m in (model.isOpen || model.openReply(m) || model.openNew(m)) && model.openAIField(m) },
        ]
        return own.merging(ComposerCommands.table) { a, _ in a }
    }
}
