#if os(macOS)
import AppKit
import QuartzCore

/// The NSTextView every long text of the Replies pages is typed in (one click puts the cursor where it lands; no edit mode). What AppKit
/// does is kept: ⌘A ⌘C ⌘X ⌘V, option-arrows, spell check, dictation, the system undo manager. Added: the @ list's keys while it is open,
/// ⌘S and ⌘↩ save, Esc reverts after one confirm line, and copy/paste carry pills as « @nom » so a pasted « @tone » is a pill again.
final class RepliesTextView: NSTextView, ChordOwningTextView {
    weak var owner: RepliesEditorCoordinator?

    /// A REAL mouse click, counted for the socket (item e2084fe8, "one click places the cursor"): how many mouse-downs this view needed to become the first
    /// responder (1 when the first click took it), nil while it was focused any other way. Read by the editors row as `mouse_downs_to_focus`.
    private(set) var mouseDownsToFocus: Int?
    /// Every mouse-down this view received, for the socket's `mouse_downs_seen` (hid-ai round 19).
    private(set) var mouseDownsSeen = 0
    private var downsBeforeFocus = 0
    private var inMouseDown = false
    /// hid-ai round 23 (main112 a124-prompt-editor:real-mouse-click "focused True after None mouse-downs (the view received 1 mouse-downs in all)"): the
    /// window makes the clicked view the first responder BEFORE it hands it the mouse-down (NSWindow's own mouse-down path), so the view was already
    /// focused when its mouseDown ran and the click was never counted; the focus looked like one taken another way. A focus taken in the same turn of the
    /// run loop as the mouse-down that follows it is that click's.
    private var focusedThisTurn = false

    override func mouseDown(with event: NSEvent) {
        mouseDownsSeen += 1
        if focusedThisTurn {   // the window gave the focus for this very click
            focusedThisTurn = false
            mouseDownsToFocus = downsBeforeFocus + 1; downsBeforeFocus = 0
            inMouseDown = true; super.mouseDown(with: event); inMouseDown = false
            return
        }
        let had = window?.firstResponder === self
        if !had { downsBeforeFocus += 1 }
        inMouseDown = true
        super.mouseDown(with: event)   // AppKit tracks the drag here until the mouse-up
        inMouseDown = false
        if !had, window?.firstResponder === self { mouseDownsToFocus = downsBeforeFocus; downsBeforeFocus = 0 }
    }

    override func becomeFirstResponder() -> Bool {
        let ok = super.becomeFirstResponder()
        if ok, !inMouseDown {
            // not yet known whether a click follows in this turn: the count waits one turn; with no mouse-down by then, the focus came another way
            mouseDownsToFocus = nil; focusedThisTurn = true
            DispatchQueue.main.async { [weak self] in
                MainActor.assumeIsolated { if let v = self, v.focusedThisTurn { v.focusedThisTurn = false; v.downsBeforeFocus = 0 } }
            }
        }
        return ok
    }

    /// ⌘S and ⌘↩ save: the shell's monitor leaves them to this view while it holds the focus (keys-r6, TextKeys.swift).
    func ownsChord(_ chord: String) -> Bool { chord == "⌘S" || chord == "⌘Enter" }

    /// The keys themselves reach this view (keys-r6); the list's Return, Tab, ↑ ↓ and Esc are its commands (`textView(_:doCommandBy:)`).
    override func keyDown(with event: NSEvent) {
        let t0 = CACurrentMediaTime()
        defer { owner?.state.lastKeyMs = (CACurrentMediaTime() - t0) * 1000 }
        super.keyDown(with: event)
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        guard window?.firstResponder === self else { return super.performKeyEquivalent(with: event) }
        let k = event.charactersIgnoringModifiers?.lowercased()
        // ⌘Z / ⇧⌘Z: one step per typed word, and an open list closes first (RepliesEditorCoordinator.undoStep)
        if k == "z", flags == .command { owner?.undoStep(); return true }
        if k == "z", flags == [.command, .shift] { owner?.redoStep(); return true }
        guard flags == .command else { return super.performKeyEquivalent(with: event) }
        if k == "s" || event.keyCode == 36 { owner?.save(); return true }
        return super.performKeyEquivalent(with: event)
    }

    override func copy(_ sender: Any?) {
        let r = selectedRange()
        guard r.length > 0, let st = textStorage, let o = owner else { return }
        let pb = NSPasteboard.general
        pb.clearContents()
        pb.setString(RepliesPills.shown(st.attributedSubstring(from: r), o.variables), forType: .string)
    }

    override func cut(_ sender: Any?) {
        let r = selectedRange()
        guard r.length > 0 else { return }
        copy(sender)
        insertText("", replacementRange: r)   // through the text system: one undoable change
    }

    override func paste(_ sender: Any?) {
        guard let text = NSPasteboard.general.string(forType: .string) else { return }
        paste(text: text)
    }
    /// ⌘V's path after the pasteboard read: « @nom » of a known variable becomes its pill, at the caret or over the selection.
    /// hid-ai: the socket's `paste-sample` seam calls this with its words, so a test copy never writes the owner's clipboard
    /// (another process may write it between the seam's write and its read).
    func paste(text: String) {
        guard let o = owner else { return }
        let piece = RepliesPills.fromPlain(text, o.key.offersVariables ? o.variables : [])
        let at = selectedRange().location
        typingAttributes = RepliesLineStyle.body
        insertText(piece, replacementRange: selectedRange())
        // only the pasted pills are variables: a character that is not a pill's attachment never keeps the attribute (round 6, see typingAttributes)
        if let st = textStorage, at + piece.length <= st.length {
            let ns = st.string as NSString
            var stray: [NSRange] = []
            st.enumerateAttribute(RepliesPills.variable, in: NSRange(location: at, length: piece.length)) { value, range, _ in
                guard value != nil else { return }
                for i in range.location..<(range.location + range.length) where ns.character(at: i) != 0xFFFC { stray.append(NSRange(location: i, length: 1)) }
            }
            if !stray.isEmpty {
                for r in stray { st.removeAttribute(RepliesPills.variable, range: r) }
                didChangeText()   // the editor's stored form is read again from the corrected text
            }
        }
    }
    override func pasteAsPlainText(_ sender: Any?) { paste(sender) }

    /// A pill's attributes never become typing attributes (hid-ai round 6). AppKit gives the text typed next the attributes of the text just deleted:
    /// after Backspace took a pill whole, every letter typed or pasted carried the pill's variable id and the stored form read each one as `{{tone}}`
    /// (main88 and main92 settingsreplies-check editor-pill-is-one-unit: one paste of « Écris comme @tone. », 14 characters, left 14 pills).
    override var typingAttributes: [NSAttributedString.Key: Any] {
        get { var a = super.typingAttributes; a[RepliesPills.variable] = nil; a[.attachment] = nil; return a }
        set {
            var a = newValue
            a[RepliesPills.variable] = nil
            a[.attachment] = nil
            super.typingAttributes = a
        }
    }
}
#endif
