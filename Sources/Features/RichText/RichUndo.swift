import Foundation

/// Undo for AI edits, one entry per rewritten paragraph. Typing keeps the text view's own undo; an AI
/// rewrite replaces whole paragraphs, so its undo puts back the whole previous paragraph, words and
/// formatting, found by the paragraph's id, not by its position.
public struct RichUndoStack: Sendable {
    struct Entry: Sendable { let before: RichParagraph; let index: Int }
    private var entries: [Entry] = []
    private let depth: Int

    /// `depth` is UNDO_DEPTH from docs/CONSTANTS.md, passed in by the caller.
    public init(depth: Int) { self.depth = depth }

    public var isEmpty: Bool { entries.isEmpty }
    public var count: Int { entries.count }

    mutating func push(before: RichParagraph, index: Int) {
        entries.append(Entry(before: before, index: index))
        if entries.count > depth { entries.removeFirst(entries.count - depth) }
    }

    /// Restore the most recent entry into `doc`. Returns false when nothing is left to undo.
    mutating func undo(in doc: inout RichDocument) -> Bool {
        guard let e = entries.popLast() else { return false }
        if let i = doc.paragraphs.firstIndex(where: { $0.id == e.before.id }) {
            doc.paragraphs[i] = e.before
        } else {
            doc.paragraphs.insert(e.before, at: min(e.index, doc.paragraphs.count))   // the paragraph was deleted since
        }
        return true
    }

    mutating func clear() { entries.removeAll() }
}
