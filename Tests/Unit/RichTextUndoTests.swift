import Foundation
import Testing
#if canImport(AppKit)
import AppKit
@testable import WhooshUI

// The composer body's undo (main81 crash: an undo after typed text asked AppKit for a range of text the model had replaced).
// The text view is built in memory, never in a window; its undo manager is given by a delegate so the test owns it.
// MUTANTS:
//   typingThenAnOutsideReloadThenUndoDoesNotThrowAndLeavesAWholeText : reload goes back to a bare setAttributedString
//   typingThenUndoRemovesTheTypedText                                : the swap drops the typing entries of a view that was not replaced
//   anOutsideReplacementThenUndoGivesOneOfTheTwoAllowedResults       : the swap leaves half of the old entries

@MainActor private final class UndoHost: NSObject, NSTextViewDelegate {
    let um = UndoManager()
    override init() { super.init(); um.groupsByEvent = false }
    func undoManager(for view: NSTextView) -> UndoManager? { um }
}

@MainActor private func makeView(_ host: UndoHost) -> WhooshTextView {
    let tv = WhooshTextView(frame: NSRect(x: 0, y: 0, width: 300, height: 200))
    tv.isRichText = true; tv.allowsUndo = true; tv.isEditable = true
    tv.delegate = host
    return tv
}

@MainActor private func type(_ s: String, into tv: WhooshTextView, _ um: UndoManager) {
    um.beginUndoGrouping()
    tv.insertText(s, replacementRange: NSRange(location: (tv.string as NSString).length, length: 0))
    um.endUndoGrouping()
}

@MainActor @Test func typingThenAnOutsideReloadThenUndoDoesNotThrowAndLeavesAWholeText() {
    let host = UndoHost(); let tv = makeView(host)
    type("Files text.", into: tv, host.um)
    type(" Undo me.", into: tv, host.um)
    RichStorageSwap.replace(in: tv, with: NSAttributedString(string: "Hi"))
    if host.um.canUndo { host.um.undo() }   // before the fix this raised NSRangeException (a shorter text, longer old ranges)
    #expect(tv.string == "Hi" || tv.string == "Files text. Undo me.", "half a text: \(tv.string)")
}

@MainActor @Test func typingThenUndoRemovesTheTypedText() {
    let host = UndoHost(); let tv = makeView(host)
    type("Files text.", into: tv, host.um)
    #expect(host.um.canUndo)
    host.um.undo()
    #expect(tv.string == "")
}

@MainActor @Test func anOutsideReplacementThenUndoGivesOneOfTheTwoAllowedResults() {
    let host = UndoHost(); let tv = makeView(host)
    RichStorageSwap.replace(in: tv, with: NSAttributedString(string: "Draft from the writer"))
    if host.um.canUndo { host.um.undo() }
    #expect(tv.string == "Draft from the writer" || tv.string == "", "half a text: \(tv.string)")
    #expect(!host.um.canUndo || tv.string == "", "an undo step is left that the model does not know")
}

@MainActor @Test func anOutsideReplacementDropsTheTypingEntriesOfThatView() {
    let host = UndoHost(); let tv = makeView(host)
    type("abc", into: tv, host.um)
    RichStorageSwap.replace(in: tv, with: NSAttributedString(string: "x"))
    #expect(!host.um.canUndo)
}
#endif
