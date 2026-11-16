import Foundation
#if os(macOS)
import AppKit
#endif

/// The editors on the socket. Controls (listed): `editor.<key>` (a click: the cursor where it lands, here at the end), `.save`, `.revert`.
/// Seams (not listed; the typed keys themselves go through the rig's real key path, `keyevent`): `.cursor.start|middle|end` (a click at that
/// place), `.at` (the « @ » key: `keyevent` cannot type « @ »), `.mouse-click` (a real left-mouse-down and up on the text view), `.paste-sample` (« Écris comme @tone. » through ⌘V's path after the pasteboard read).
@MainActor
enum RepliesEditorDoors {
    /// Every place a long text is edited on the Replies pages: one component (the class rule of A124).
    static func places(_ s: RepliesStore) -> [(key: RepliesEditorKey, page: RepliesPage)] {
        [(.prompt, .prompt), (.retrievalPrompt, .retrieval), (.voiceText, .voice), (.aiDescription, .filter), (.newNote, .prompt)]
            + s.notes.map { (RepliesEditorKey.note($0.id), RepliesPage.prompt) }
    }

    static func add(_ s: RepliesStore, _ add: TextDoors.Add) {
        for (k, page) in places(s) {
            add(page, "editor.\(k.raw)", "text") { _ = cursor(k, at: .max) }
            add(page, "editor.\(k.raw).save", "button") { s.save(k) }
            add(page, "editor.\(k.raw).revert", "button") { let e = s.editor(k); if e.confirmRevert { e.revert() } else if e.dirty { e.confirmRevert = true } }
        }
    }

    static func seams(_ s: RepliesStore, _ out: inout [String: (AppModel) -> Bool]) {
        for (k, _) in places(s) {
            let P = RepliesAx.id("editor", k.raw)
            out["\(P).cursor.start"] = { _ in cursor(k, at: 0) }
            out["\(P).cursor.middle"] = { _ in cursor(k, at: -1) }
            out["\(P).cursor.end"] = { _ in cursor(k, at: .max) }
            #if os(macOS)
            out["\(P).at"] = { _ in guard let tv = RepliesEditorViews.live[k] else { return false }; tv.insertText("@", replacementRange: tv.selectedRange()); return true }
            out["\(P).mouse-click"] = { _ in mouseClick(k) }
            out["\(P).paste-sample"] = { _ in
                guard let tv = RepliesEditorViews.live[k] else { return false }
                tv.paste(text: RepliesDoors.samplePaste); return true   // ⌘V's own path after the pasteboard read; the owner's clipboard is left alone
            }
            #endif
        }
    }

    /// A click's effect: the keys go to the text view and the cursor sits at `at` (-1: the middle; past the end: the end).
    static func cursor(_ k: RepliesEditorKey, at: Int) -> Bool {
        #if os(macOS)
        // a text view that is not in a window yet (the page was just opened, SwiftUI has not placed it) takes no focus: the click is refused, never answered "true" (round 8, main76: voiceText had no frame and took no key)
        guard let tv = RepliesEditorViews.live[k], let win = tv.window else { return false }
        let n = tv.textStorage?.length ?? 0
        win.makeFirstResponder(tv)
        tv.setSelectedRange(NSRange(location: at < 0 ? n / 2 : min(at, n), length: 0))
        return true
        #else
        return false
        #endif
    }

    /// A real mouse click on the text view: the editor is first released, then a left-mouse-down and its up go through the window's own event path (the up is
    /// queued first, so the view's mouse tracking ends at once). The view must take the focus by itself; the row's `mouse_downs_to_focus` says in how many downs.
    static func mouseClick(_ k: RepliesEditorKey) -> Bool {
        #if os(macOS)
        guard let tv = RepliesEditorViews.live[k], let win = tv.window else { return false }
        // hid-ai round 19 (main111 a124-prompt-editor:real-mouse-click "focused False after None mouse-downs"): the hidden copy is never the active app and its
        // windows are never key, so AppKit took this click as an inactive window's (the event loop does not hand queued events to an inactive window: the
        // drag verb found the same, Sources/App/PointerDrag.swift:25 and its note). As the drag does, the click runs with the app pretending to be active and
        // its window key, inside the app only (never the front app); an owner's click lands on their active window. Test copy only.
        let hidden = HiddenRig.isOn
        if hidden { HiddenRig.pretendActive = true; win.makeKey() }
        defer { if hidden { HiddenRig.pretendActive = false } }
        _ = win.makeFirstResponder(nil)
        let p = tv.convert(NSPoint(x: tv.visibleRect.midX, y: tv.visibleRect.midY), to: nil)
        func ev(_ t: NSEvent.EventType) -> NSEvent? {
            NSEvent.mouseEvent(with: t, location: p, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: win.windowNumber,
                               context: nil, eventNumber: 0, clickCount: 1, pressure: t == .leftMouseDown ? 1 : 0)
        }
        guard let down = ev(.leftMouseDown), let up = ev(.leftMouseUp) else { return false }
        NSApp.postEvent(up, atStart: false)
        win.sendEvent(down)
        return true
        #else
        return false
        #endif
    }

    /// hid-ai round 20: true when the card cuts a text that is under its ceiling (the text taller than the card, or scrolled while it would fit): the
    /// picture of main111's prompt pane, dark, 700x480. null when the editor is not on screen. A socket read only (it lays the text out to measure it).
    private static func textCut(_ k: RepliesEditorKey) -> Any {
        #if os(macOS)
        guard let tv = RepliesEditorViews.live[k], let lm = tv.layoutManager, let tc = tv.textContainer, let clip = tv.enclosingScrollView?.contentView else { return NSNull() }
        lm.ensureLayout(for: tc)
        let used = ceil(lm.usedRect(for: tc).height), box = clip.bounds.height, y = clip.bounds.origin.y
        return used <= RepliesTextArea.ceiling && (used > box + 1 || y > 0.5)
        #else
        return NSNull()
        #endif
    }

    /// Every mouse-down the text view received (round 19): tells a click that never reached the view (0) from one that reached it and took no focus.
    private static func mouseSeen(_ k: RepliesEditorKey) -> Any {
        #if os(macOS)
        return RepliesEditorViews.live[k].map { $0.mouseDownsSeen } ?? NSNull()
        #else
        return NSNull()
        #endif
    }

    private static func mouseDowns(_ k: RepliesEditorKey) -> Any {
        #if os(macOS)
        return RepliesEditorViews.live[k]?.mouseDownsToFocus ?? NSNull()
        #else
        return NSNull()
        #endif
    }

    private static func frame(_ r: CGRect) -> ControlRow { ["x": Double(r.minX), "y": Double(r.minY), "w": Double(r.width), "h": Double(r.height)] }
    private static func textFrame(_ k: RepliesEditorKey) -> CGRect? {
        #if os(macOS)
        return RepliesEditorViews.textFrame(k)
        #else
        return nil
        #endif
    }
    private static var focusedKey: RepliesEditorKey? {
        #if os(macOS)
        return RepliesEditorViews.focused?.owner?.key
        #else
        return nil
        #endif
    }

    static func rows(_ s: RepliesStore) -> [ControlRow] {
        places(s).map { k, page in
            let e = s.editor(k)
            var marked = RepliesVariables.pieces(e.text, s.variables).map { p -> String in
                switch p { case .words(let w): return w; case .variable(let name): return "[@\(name)]" }
            }.joined()
            var live = false
            #if os(macOS)
            if let st = RepliesEditorViews.live[k]?.textStorage { marked = RepliesPills.marked(st, s.variables); live = true }
            #endif
            return ["ax": RepliesAx.id("editor", k.raw), "key": k.raw, "page": page.rawValue, "component": String(describing: RepliesTextEditor.self), "live": live,
                    "text": marked, "stored": e.text, "lines": RepliesLineKind.kinds(e.text), "cursor": e.cursor, "undo_depth": e.undoDepth,
                    "dirty": e.dirty, "save_shown": e.dirty, "line": RepliesWords.editorLine(e) ?? NSNull(), "unknown": e.unknown,
                    "confirm": e.confirmRevert, "menu_open": e.menuQuery != nil, "menu_query": e.menuQuery ?? NSNull(),
                    "menu_items": e.items(s.variables).map(\.id), "menu_index": e.menuIndex, "last_key_ms": e.lastKeyMs,
                    "versions": k == .prompt ? s.versions.count : NSNull(), "double_click": false, "mouse_downs_to_focus": mouseDowns(k), "mouse_downs_seen": mouseSeen(k), "text_cut": textCut(k),
                    "focused": focusedKey == k, "card_frame": frame(e.cardFrame), "text_frame": textFrame(k).map(frame) ?? NSNull()]
        }
    }
}
