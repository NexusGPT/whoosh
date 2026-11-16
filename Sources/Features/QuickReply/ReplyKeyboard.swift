import SwiftUI
#if os(macOS)
import AppKit

/// The reply's hold on the keyboard, as a STATE the shell's key monitor honours (never a timer).
///
/// Owner, 13 Nov 07:28: "when hitting "R" I should have cursor INSIDE the ansser, and here I cannot type, it stays on list". The first fix asked
/// SwiftUI for the focus at 0, 80 and 240 ms. On a loaded machine the mail's body (a web view) or the pane's host took the keyboard after the last
/// ask, and the next letters met no text: lost (the reply check on main124: "456" typed after R, the window's first responder the web view).
///
/// The rule: while `QuickReplyModel.wantsKeys` is true, a key a text would take is first given back its text. `ChordKit.handle` asks `reclaim`
/// before it treats a key as a command: the reply's own text view becomes the first responder and the same event is then delivered to it. A key
/// pressed before the box is in the window (R and a letter in the same instant) goes into the draft itself.
/// `wantsKeys` ends at a send, at Esc (the reply closes) and at a click outside the reply's box.
@MainActor
enum ReplyKeyboard {
    weak static var textAnchor: NSView?
    weak static var boxAnchor: NSView?
    private static var monitor: Any?

    /// Installed once (the reply feature's first key table): the click that leaves the reply.
    static func installOnce() {
        guard monitor == nil else { return }
        ChordKit.reclaimTyping = { e in reclaim(e) }
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { e in
            let q = QuickReplyModel.shared
            if q.wantsKeys, let box = boxAnchor, let w = box.window, e.window === w, !box.convert(box.bounds, to: nil).contains(e.locationInWindow) { q.wantsKeys = false }
            return e
        }
    }

    /// The reply's own text view: the one drawn over the anchor laid behind the editor.
    static func textView() -> NSTextView? {
        guard let a = textAnchor, a.window != nil else { return nil }
        let target = a.convert(a.bounds, to: nil)
        var root: NSView? = a.superview
        for _ in 0..<8 {
            guard let r = root else { return nil }
            if let t = find(in: r, over: target) { return t }
            root = r.superview
        }
        return nil
    }

    private static func find(in v: NSView, over target: NSRect) -> NSTextView? {
        if let t = v as? NSTextView, !t.isFieldEditor, t.isEditable, t.convert(t.bounds, to: nil).intersects(target) { return t }
        for s in v.subviews { if let t = find(in: s, over: target) { return t } }
        return nil
    }

    /// 0: the key is not the reply's. 1: the reply's text was made the first responder; the monitor delivers this same event to it and runs no
    /// command. 2: the key went into the draft itself (the box is not in the window yet); the event ends here.
    static func reclaim(_ e: NSEvent) -> Int {
        let q = QuickReplyModel.shared, m = AppModel.shared
        guard q.wantsKeys, q.showsBox, q.phase == .editing, m.overlay == nil, !q.fromOpen, !q.emojiOpen, q.header.focus == .none, q.ai.focus == .reply,
              QuickReplyFeature.holdsKeys(m), let w = WindowBridge.mainWindow(), (e.window ?? w) === w, !(w.firstResponder is NSText) else { return 0 }
        if let text = textView(), w.makeFirstResponder(text) { return 1 }
        // The box is not in the window yet (the key came in the instant after R): a typed character goes into the draft; the editor shows it.
        let flags = e.modifierFlags.intersection([.control, .option, .command])
        guard flags.isEmpty, let typed = e.characters, !typed.isEmpty, typed.unicodeScalars.allSatisfy({ $0.value >= 0x20 && $0.value != 0x7F && !(0xF700...0xF8FF).contains($0.value) }) else { return 0 }
        q.draft += typed
        return 2
    }
}

/// A view with no drawing laid behind the reply's editor (or its whole box), so AppKit code can find where it is.
struct ReplyAnchor: NSViewRepresentable {
    enum Role { case text, box }
    let role: Role
    func makeNSView(context: Context) -> NSView {
        let v = NSView(frame: .zero)
        if role == .text { ReplyKeyboard.textAnchor = v } else { ReplyKeyboard.boxAnchor = v }
        return v
    }
    func updateNSView(_ v: NSView, context: Context) {
        if role == .text { ReplyKeyboard.textAnchor = v } else { ReplyKeyboard.boxAnchor = v }
    }
}
#endif
