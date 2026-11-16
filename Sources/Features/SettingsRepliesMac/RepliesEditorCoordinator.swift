#if os(macOS)
import AppKit

/// Between the text view and its RepliesEditorState: each change writes the draft (stored form), restyles the edited paragraphs, finds an
/// open « @ » before the caret, and turns a fully typed « @nom » into its pill; a reload takes a revert or a save from outside.
@MainActor
final class RepliesEditorCoordinator: NSObject, NSTextViewDelegate {
    let state: RepliesEditorState
    var key: RepliesEditorKey { state.key }
    var variables: [ReplyPromptVariable] { RepliesStore.shared.variables }
    weak var textView: RepliesTextView?
    /// The editor's own undo manager: ⌘Z here never undoes a change made in another field of the window.
    let undo = UndoManager()
    /// The span the coming change touches (its start, the length of what replaces it): the paragraphs restyled after it.
    private var pending: NSRange?
    var applied = -1
    /// True when the next change starts a new undo step: after a word ends, after a pill, a paste, an undo (the coalescing is broken then).
    private var stepOpen = true
    /// Where the « @ » of the open list sits; an Esc closes the list until a new « @ » is typed.
    private var atLocation: Int?
    private var closedAt: Int?
    nonisolated(unsafe) private var observers: [NSObjectProtocol] = []
    /// hid-ai round 20: the width SwiftUI last measured the card at, and whether the whole text fitted under the card's ceiling then.
    var measuredWidth: CGFloat = -1
    var fitsWhole = true

    init(state: RepliesEditorState) {
        self.state = state
        super.init()
        // undo depth for the socket: a new undo step counts in textDidChange (stepOpen); each undo -1, each redo +1 (only the kind crosses)
        for (name, step) in [(NSNotification.Name.NSUndoManagerDidUndoChange, -1), (.NSUndoManagerDidRedoChange, 1)] {
            observers.append(NotificationCenter.default.addObserver(forName: name, object: undo, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { if let s = self { s.state.undoDepth = max(0, s.state.undoDepth + step) } }
            })
        }
    }

    deinit { for o in observers { NotificationCenter.default.removeObserver(o) } }

    func undoManager(for view: NSTextView) -> UndoManager? { undo }

    /// hid-ai round 20: when the card's real width is not the width it was measured at, SwiftUI is asked to measure it again; while the whole text fits,
    /// the text shows from its top (a card never keeps a scroll it does not need). Frame changes only: no timer, nothing per key.
    func watchWidth(of scroll: NSScrollView) {
        // round 24: the text view grows and shrinks with its text at the width it really has; its frame change gives the card that height (state.textHeight)
        if let tv = scroll.documentView as? NSTextView {
            tv.postsFrameChangedNotifications = true
            observers.append(NotificationCenter.default.addObserver(forName: NSView.frameDidChangeNotification, object: tv, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.textLaidOut() }
            })
        }
        let clip = scroll.contentView
        clip.postsFrameChangedNotifications = true
        observers.append(NotificationCenter.default.addObserver(forName: NSView.frameDidChangeNotification, object: clip, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.clipChanged(clip) }
        })
    }

    private func clipChanged(_ clip: NSClipView) {
        let w = clip.bounds.width
        if measuredWidth > 0, abs(w - measuredWidth) > 0.5 { remeasure() }
        if fitsWhole, clip.bounds.origin.y != 0, let tv = textView, tv.window?.firstResponder !== tv { tv.scroll(.zero) }
    }

    /// The laid-out text's height at the text view's real width; the card follows it on the next turn (never inside a layout pass), only when it changed.
    private func textLaidOut() {
        guard let tv = textView, let lm = tv.layoutManager, let tc = tv.textContainer, tv.frame.width > 0 else { return }
        lm.ensureLayout(for: tc)
        let h = ceil(lm.usedRect(for: tc).height)
        guard abs(h - state.textHeight) > 0.5 else { return }
        DispatchQueue.main.async { [weak self] in MainActor.assumeIsolated { self?.state.textHeight = h } }
    }

    /// Asks SwiftUI for the card's size again, on the next turn (never inside a layout pass).
    func remeasure() {
        DispatchQueue.main.async { [weak self] in MainActor.assumeIsolated { self?.state.layoutTick += 1 } }
    }

    /// The list's keys as the text system names them: Return and Tab place, ↑ ↓ choose, Esc closes; with no list, Esc is the editor's
    /// (one confirm line, then revert). Everything else is the text view's own.
    func textView(_ textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        if state.menuQuery != nil {
            switch selector {
            case #selector(NSResponder.insertNewline(_:)), #selector(NSResponder.insertTab(_:)): return acceptMenu()
            case #selector(NSResponder.moveDown(_:)): moveMenu(1); return true
            case #selector(NSResponder.moveUp(_:)): moveMenu(-1); return true
            case #selector(NSResponder.cancelOperation(_:)): closeMenu(); return true
            default: return false
            }
        }
        if selector == #selector(NSResponder.cancelOperation(_:)) { escape(); return true }
        return false
    }

    /// Takes the state's text (first show, a revert, a save of a new note): pills built, every line styled once.
    func reload() {
        guard let tv = textView, let st = tv.textStorage else { return }
        applied = state.revision
        let keep = tv.selectedRange()
        st.setAttributedString(RepliesPills.attributed(state.text, key.offersVariables ? variables : []))
        RepliesLineStyle.restyle(st, around: NSRange(location: 0, length: st.length))
        tv.setSelectedRange(NSRange(location: min(keep.location, st.length), length: 0))
        tv.typingAttributes = RepliesLineStyle.body
        state.unknown = key.offersVariables ? RepliesPills.unknown(state.text) : []
        if tv.window?.firstResponder !== tv { tv.scroll(.zero) }   // a text replaced from outside shows from its top (round 20)
        remeasure()                                                  // and its card is measured for it (round 20)
    }

    // MARK: changes

    func textView(_ textView: NSTextView, shouldChangeTextIn range: NSRange, replacementString text: String?) -> Bool {
        pending = NSRange(location: range.location, length: (text as NSString?)?.length ?? 0)
        return true
    }

    func textDidChange(_ n: Notification) {
        guard let tv = textView, let st = tv.textStorage else { return }
        if let p = pending { pending = nil; RepliesLineStyle.restyle(st, around: p) }   // the edited paragraphs only, never the whole text
        if undo.isUndoing, state.menuQuery != nil { closeMenu() }   // an undo from anywhere (the Edit menu too) closes the list
        countStep(tv, st)
        if key.offersVariables { pillTypedName(tv, st) }
        let stored = RepliesPills.stored(st)
        state.draft = stored == state.saved ? nil : stored
        state.confirmRevert = false
        state.unknown = key.offersVariables ? RepliesPills.unknown(stored) : []
        findAt(tv)
    }

    func textViewDidChangeSelection(_ n: Notification) {
        guard let tv = textView else { return }
        state.cursor = tv.selectedRange().location
        findAt(tv)
    }

    /// « @ » then letters right before the caret, at a line start or after a non-letter: the list opens, its query is the letters.
    private func findAt(_ tv: NSTextView) {
        guard key.offersVariables, let st = tv.textStorage, tv.selectedRange().length == 0 else { return close() }
        let caret = tv.selectedRange().location, ns = st.string as NSString
        var i = caret
        while i > 0, let u = UnicodeScalar(ns.character(at: i - 1)), CharacterSet.letters.union(.decimalDigits).contains(u) { i -= 1 }
        guard i > 0, ns.character(at: i - 1) == 64 /* @ */, i - 1 != closedAt else { return close() }
        if i - 1 > 0, let p = UnicodeScalar(ns.character(at: i - 2)), CharacterSet.letters.union(.decimalDigits).contains(p) { return close() }
        atLocation = i - 1
        let q = ns.substring(with: NSRange(location: i, length: caret - i))
        if state.menuQuery != q { state.menuIndex = 0 }
        state.menuQuery = q
        // the « @ »'s rectangle in the editor's own coordinates (top-left origin): the list opens under it
        if let lm = tv.layoutManager, let tc = tv.textContainer {
            let g = lm.glyphRange(forCharacterRange: NSRange(location: i - 1, length: 1), actualCharacterRange: nil)
            let r = lm.boundingRect(forGlyphRange: g, in: tc).offsetBy(dx: tv.textContainerOrigin.x, dy: tv.textContainerOrigin.y)
            // in what the scroll view shows (the list sits over the visible text, scrolled or not)
            let shown = tv.enclosingScrollView?.contentView.bounds.origin ?? .zero
            state.caret = r.offsetBy(dx: -shown.x, dy: -shown.y)
        }
    }

    private func close() { state.menuQuery = nil; atLocation = nil }
    func closeMenu() { closedAt = atLocation; close() }
    func moveMenu(_ d: Int) {
        let n = state.items(variables).count
        guard n > 0 else { return }
        state.menuIndex = (state.menuIndex + d + n) % n
    }

    /// Return or Tab in the list: « @ » and the letters typed become the chosen variable's pill (one undoable change).
    @discardableResult
    func acceptMenu() -> Bool {
        let items = state.items(variables)
        guard let at = atLocation, items.indices.contains(state.menuIndex), let tv = textView else { return false }
        return insert(items[state.menuIndex], replacing: NSRange(location: at, length: tv.selectedRange().location - at))
    }

    /// A variable from the list or from Variables: its pill at the caret (or over `range`), then a space so typing goes on in words.
    @discardableResult
    func insert(_ v: ReplyPromptVariable, replacing range: NSRange? = nil) -> Bool {
        guard let tv = textView else { return false }
        let s = NSMutableAttributedString(attributedString: RepliesPills.pill(v))
        s.append(NSAttributedString(string: " ", attributes: RepliesLineStyle.body))
        tv.insertText(s, replacementRange: range ?? tv.selectedRange())
        close()
        return true
    }

    /// « @tone » typed to the end and followed by a space or a mark: the words become the pill.
    private func pillTypedName(_ tv: NSTextView, _ st: NSTextStorage) {
        let caret = tv.selectedRange().location, ns = st.string as NSString
        guard caret >= 2, let last = UnicodeScalar(ns.character(at: caret - 1)), !CharacterSet.letters.union(.decimalDigits).contains(last) else { return }
        for v in variables.sorted(by: { RepliesVariables.name($0).count > RepliesVariables.name($1).count }) {
            let word = "@" + RepliesVariables.name(v), len = (word as NSString).length
            let start = caret - 1 - len
            guard start >= 0, ns.substring(with: NSRange(location: start, length: len)) == word else { continue }
            if start > 0, let p = UnicodeScalar(ns.character(at: start - 1)), CharacterSet.letters.contains(p) { continue }
            tv.insertText(RepliesPills.pill(v), replacementRange: NSRange(location: start, length: len))   // a second change, its own undo step
            return
        }
    }

    /// Typing is one undo step per word, as a Mac text view groups it: the coalescing breaks after a space, a mark or a new line, and
    /// after any change that is not a typed letter (a pill, a paste, a deletion), so ⌘Z takes back the last word, the last pill.
    private func countStep(_ tv: NSTextView, _ st: NSTextStorage) {
        if stepOpen && !undo.isUndoing && !undo.isRedoing { state.undoDepth += 1; stepOpen = false }
        let caret = tv.selectedRange().location, ns = st.string as NSString
        let typedLetter = caret > 0 && caret <= ns.length && (UnicodeScalar(ns.character(at: caret - 1)).map { CharacterSet.letters.union(.decimalDigits).contains($0) } ?? false)
        if !typedLetter || NSApp.currentEvent?.type != .keyDown { tv.breakUndoCoalescing(); stepOpen = true }
    }
    func undoStep() { closeMenu(); textView?.breakUndoCoalescing(); stepOpen = true; if undo.canUndo { undo.undo() } }
    func redoStep() { if undo.canRedo { undo.redo() }; stepOpen = true }

    // MARK: keys

    func save() { RepliesStore.shared.save(key) }

    /// Esc: nothing to lose does nothing; with changes, a first Esc asks in one line, a second reverts to the saved text.
    func escape() {
        guard state.dirty else { state.confirmRevert = false; return }
        if state.confirmRevert { state.revert() } else { state.confirmRevert = true }
    }
}
#endif
