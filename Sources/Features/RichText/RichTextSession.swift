import Foundation
import Observation

/// What the platform text view offers the session: the composer's toolbar and the keys call these. Each format call acts
/// on the selection, or on the text typed next when nothing is selected, and changes the editor's attributed text at once.
public struct RichTextViewHooks {
    public var toggleBold: () -> Void = {}
    public var toggleList: () -> Void = {}
    /// Link the selection to this address; with no selection the address goes in as linked text.
    public var setLink: (String) -> Void = { _ in }
    public var insertPicture: (String) -> Void = { _ in }
    public var toggleItalic: () -> Void = {}
    public var toggleUnderline: () -> Void = {}
    public var toggleStrike: () -> Void = {}
    /// A `RichColor` name, or nil for the default text colour.
    public var setColor: (String?) -> Void = { _ in }
    public var toggleNumbered: () -> Void = {}
    public var toggleQuote: () -> Void = {}
    /// Tab and Shift+Tab inside a list: one level in or out. False when the caret is not in a list, so the key passes on.
    public var indentList: (Int) -> Bool = { _ in false }
    /// The Increase and Decrease Indent keys: one step on every paragraph the selection touches.
    public var indentBlock: (Int) -> Void = { _ in }
    /// What the format is at the caret (or the start of the selection).
    public var formatState: () -> RichFormatState = { RichFormatState() }
    /// The text view takes the keyboard (the Edit Message key).
    public var focusBody: () -> Void = {}
    /// Selects all the text (a rig drive uses it to show the format bar over a selection).
    public var selectAll: () -> Void = {}
    /// The text the editor shows now (nil with no editor on screen): the socket compares it with the session's document.
    public var viewText: () -> String? = { nil }
    /// A pick from an inline list: the typed trigger and its query are replaced by this text.
    public var replaceTrigger: (String) -> Void = { _ in }
    public init() {}
}

/// The format at the caret, for the control socket and the toolbar's pressed look.
public struct RichFormatState: Equatable, Sendable {
    public var bold = false, italic = false, underline = false, strike = false
    /// A `RichColor` name; nil is the default colour.
    public var color: String?
    /// "bullet", "number" or "none".
    public var list = "none"
    public var quote = false
    public var indent = 0
    /// How many characters are selected.
    public var selection = 0
    public var bodyFocused = false
    public init() {}
}

/// The one editor state both composers share (Mac now, iPhone later): the document, the AI-edit undo
/// stack, and the MIME body. The platform view reads and writes `document`; nothing else holds text.
@MainActor @Observable
public final class RichTextSession {
    public private(set) var document = RichDocument()
    /// Bumped when the document changes from outside the text view, so the view reloads itself.
    public private(set) var revision = 0
    /// Where the selection sits in the editor's own coordinates (top-left origin); nil with no selection. The contextual toolbar anchors here.
    public var selectionRect: CGRect?
    /// True when the next reload is an AI rewrite or its undo: the view cross-fades it. Typing never sets it.
    @ObservationIgnored private var fadeNext = false
    @ObservationIgnored public var hooks = RichTextViewHooks()
    /// The inline list the caret opened (`;name`, `:smi`), nil when none; set by the text view.
    public var triggerState: RichTriggerState?
    /// While `triggerState` is set the text view offers Up, Down, Return, Tab and Escape here first; true means taken.
    @ObservationIgnored public var listKeyHandler: ((RichListKey) -> Bool)?
    /// The toolbar's link and colour buttons ask the composer to open its link and colour popovers.
    @ObservationIgnored public var requestLink: () -> Void = {}
    @ObservationIgnored public var requestColor: () -> Void = {}
    /// ⇧⌘Z in the text (the composer's instant send): asked only for that exact chord, so ⌘Z stays the text's undo.
    @ObservationIgnored public var instantSend: () -> Bool = { false }
    /// ⌘V in the text, asked first: true when the pasteboard held files (or a picture copied as data, no text) and they were attached.
    /// A pasted file never goes into the text as its name (owner 2026-11-12: "copy-paste of file should add attachment").
    @ObservationIgnored public var pasteFiles: () -> Bool = { false }
    /// The pasteboard the text reads a paste from: nil for the system's; a rig names its private one, so a test never reads the owner's.
    @ObservationIgnored public var pasteboardName: String?
    @ObservationIgnored private var undoStack: RichUndoStack
    /// True once the owner typed after the last AI edit: then the text view's own undo goes first.
    @ObservationIgnored public private(set) var typedSinceRewrite = false

    /// `undoDepth` is UNDO_DEPTH (docs/CONSTANTS.md), passed by the composer from `WhooshConstants`.
    public init(undoDepth: Int) { undoStack = RichUndoStack(depth: undoDepth) }

    // MARK: MIME

    /// The HTML part of the draft's MIME body.
    public var mimeHTML: String { RichHTML.serialize(document) }

    public func load(html: String) {
        document = RichHTML.parse(html); undoStack.clear(); typedSinceRewrite = false; revision += 1
    }

    // MARK: edits from the platform view

    /// The text view changed: take its document. Paragraph ids carry over by position so undo entries stay valid.
    public func viewDidChange(_ new: RichDocument) {
        var d = new
        for i in d.paragraphs.indices where i < document.paragraphs.count { d.paragraphs[i].id = document.paragraphs[i].id }
        document = d; typedSinceRewrite = true
    }

    // MARK: edits from outside (fixture, AI)

    public func reset() { document = RichDocument(); undoStack.clear(); typedSinceRewrite = false; revision += 1 }

    public func setBold(_ text: String) { replaceAll([RichParagraph(runs: [RichRun(text, bold: true)])]) }
    public func setList(_ items: [String]) { replaceAll(items.map { RichParagraph($0, isListItem: true) }) }
    public func setLink(_ text: String, href: String) { replaceAll([RichParagraph(runs: [RichRun(text, href: href)])]) }
    public func setPicture(cid: String) { replaceAll([RichParagraph(runs: [RichRun("", cid: cid)])]) }
    public func setPlain(_ text: String) { replaceAll(text.isEmpty ? [] : text.components(separatedBy: "\n").map { RichParagraph($0) }) }

    /// Paste: the HTML is parsed to the composer's own set, so the source font and style never arrive.
    public func paste(html: String) {
        let p = RichHTML.parse(html)
        if document.isEmpty { replaceAll(p.paragraphs); return }
        document.paragraphs += p.paragraphs; revision += 1; typedSinceRewrite = true
    }

    private func replaceAll(_ ps: [RichParagraph]) { document = RichDocument(paragraphs: ps); revision += 1; typedSinceRewrite = true }

    // MARK: AI rewrite and its undo

    /// An AI edit replaces one paragraph. The paragraph before it is kept whole for ⌘Z.
    @discardableResult
    public func applyRewrite(paragraph index: Int, text: String) -> Bool {
        guard document.paragraphs.indices.contains(index) else { return false }
        let old = document.paragraphs[index]
        undoStack.push(before: old, index: index)
        document.paragraphs[index].runs = text.isEmpty ? [] : [RichRun(text)]
        fadeNext = true; revision += 1; typedSinceRewrite = false
        return true
    }

    /// ⌘Z: true when it put back an AI-edited paragraph. When the owner typed since, the view's own undo goes first.
    @discardableResult
    public func undoRewrite() -> Bool {
        guard !typedSinceRewrite, undoStack.undo(in: &document) else { return false }
        fadeNext = true; revision += 1; return true
    }

    /// The view asks once per reload whether to cross-fade it.
    public func consumeFade() -> Bool { defer { fadeNext = false }; return fadeNext }

    public var canUndoRewrite: Bool { !typedSinceRewrite && !undoStack.isEmpty }
}
