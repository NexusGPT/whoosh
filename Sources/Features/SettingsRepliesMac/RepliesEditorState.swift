import Foundation
import Observation
import CoreGraphics

/// Every long text of the Replies pages, edited by ONE component (RepliesTextEditor), always live (owner A124: "not at all like nexus which
/// is like notion"). The key names the text; what it saves to is the store's (RepliesEditorSaves.swift).
struct RepliesEditorKey: Hashable, Sendable {
    let raw: String
    static let prompt = RepliesEditorKey(raw: "prompt")
    static let retrievalPrompt = RepliesEditorKey(raw: "retrievalPrompt")
    static let voiceText = RepliesEditorKey(raw: "voiceText")
    static let aiDescription = RepliesEditorKey(raw: "aiDescription")
    static let newNote = RepliesEditorKey(raw: "note.new")
    static func note(_ id: String) -> RepliesEditorKey { RepliesEditorKey(raw: "note.\(id)") }
    var noteId: String? { raw.hasPrefix("note.") && raw != "note.new" ? String(raw.dropFirst(5)) : nil }
    /// Variables (« @nom » pills) belong to the two prompts; the other texts are plain words.
    var offersVariables: Bool { self == .prompt || self == .retrievalPrompt }
}

/// The live state of one editor: the saved text (the core's), the draft (what is typed, kept while the page is left), the @ list, the words
/// under the editor, and what the socket reads (cursor, undo depth, last key's cost). Kept by the store, so leaving the page keeps it.
@MainActor @Observable
final class RepliesEditorState {
    let key: RepliesEditorKey
    /// The stored form the core holds (`{{id}}` for a variable).
    var saved = ""
    /// The stored form of what is in the editor now; nil until the owner changes something.
    var draft: String?
    var dirty: Bool { draft.map { $0 != saved } ?? false }
    var text: String { draft ?? saved }
    /// The @ list: open while the text before the caret is « @ » and letters; its query, the variables it shows, the one chosen.
    var menuQuery: String?
    var menuIndex = 0
    var caret: CGRect = .zero
    /// The card's frame in the window's content coordinates (top-left), for the socket: the text must be drawn inside it.
    var cardFrame: CGRect = .zero
    /// « @mot » that is no variable, said in one line under the editor.
    var unknown: [String] = []
    /// Esc with changes asks once; a second Esc reverts.
    var confirmRevert = false
    var cursor = 0
    var undoDepth = 0
    var lastKeyMs = 0.0
    /// Counts outside replacements (a revert, a save, a variable from the list), so the text view takes the new text.
    var revision = 0
    /// hid-ai round 20: counts the times the card must be measured again (the text was replaced from outside, or the card's real width is not the width it
    /// was measured at): SwiftUI asks the text area's size again when it changes (RepliesTextArea.layoutTick).
    var layoutTick = 0
    /// hid-ai round 24 (main114 voice dark 700x480: « My voice » cut inside its card, after round 20's re-measure): the text's height as AppKit laid it out at the
    /// card's REAL width, written by the coordinator when the text view's frame changes; the card takes it (RepliesTextEditor), whatever SwiftUI measured.
    var textHeight: CGFloat = 0

    init(key: RepliesEditorKey) { self.key = key }

    func items(_ vars: [ReplyPromptVariable]) -> [ReplyPromptVariable] {
        guard let q = menuQuery?.lowercased(), !q.isEmpty else { return vars }
        return vars.filter { RepliesVariables.name($0).lowercased().hasPrefix(q) || $0.id.hasPrefix(q) }
    }

    /// The core's value changed (a load): it becomes the saved text; a draft equal to it is no longer a change.
    func take(saved s: String) {
        guard s != saved else { return }
        saved = s
        if draft == s { draft = nil }
        if draft == nil { revision += 1 }
    }

    func revert() { draft = nil; confirmRevert = false; revision += 1 }
}
