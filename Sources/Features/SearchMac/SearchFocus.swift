import SwiftUI
#if os(macOS)
import AppKit
#endif

/// Who has the keyboard, for the search field. The field's text lives in `AppModel.typed` only while the field holds the keys: that buffer is
/// shared with the quick reply, the palette and the notch, so a field that kept showing it after the keys moved on showed the reply's words too
/// (the owner, 2026-11-08 11:30: "hello" typed in the reply appeared in the search bar). The field now holds the keys only from ⌘K, `/` or a click on it,
/// lends itself the buffer then, and gives it back on Esc, on a click anywhere else, or when another surface or row takes the focus.
@MainActor
extension SearchMacModel {
    static let surfaceId = "mac-search"

    /// the field holds the keyboard and the shared typed buffer
    public var holds: Bool { owns && app.surface == Self.surfaceId }

    /// the field's text: the shared buffer while it holds the keys, its own kept text otherwise
    public var text: String {
        get { holds ? app.typed : held }
        set { held = newValue; if holds { app.typed = newValue } }
    }

    /// ⌘K, `/`, a click on the field: it takes the keys and lends itself the buffer (the buffer's other user is kept to hand back).
    func claim() {
        guard !holds else { return }
        if app.surface != Self.surfaceId { stash = app.typed }
        app.typed = held
        owns = true; dismissed = false; highlight = 0
        claimedAt = app.focusIndex; claimedRows = app.rows.count; claimedId = app.focusedMessageId; claimedTime = Date()
        app.surface = Self.surfaceId
        // The field sits in the list column (layout L1): the pane focus comes to the list with the keys. Left on the reader (an opened result or thread
        // sets it, SearchMacActs.openResult, ReaderFeature.openFocused), the reader holds the focus by the pane alone (ReaderFeature.hasFocus) and,
        // being ahead of the search in key order, takes Enter as "open and reply": main93, J5.4 after J2 and par-bar-shows-search-keys-during-search
        // ended on the reader with the quick reply open and nothing launched.
        let shell = MacShellState.shared
        if shell.focusedPane == .reader, !shell.listCollapsed { shell.focusedPane = .list }
    }

    /// Esc, a click elsewhere, another row or surface: the field lets go. Its text stays (until Esc or the clear button), its dropdown closes,
    /// the buffer goes back to what it held before, and the keys are the list's again.
    func release(_ reason: String = "call") {
        guard owns else { return }
        releasedBy = reason
        let lent = app.surface == Self.surfaceId
        if lent, app.typed == held { app.typed = stash }   // nobody else rewrote the buffer: hand it back as it was
        owns = false; dismissed = true
        if lent {
            app.surface = "mac-main"
            // The text cursor leaves in the same turn as the keys (not one view update later): a key typed in between would otherwise reach a field the
            // model no longer owns. "Shortcuts are contextual": the place holding the cursor says what a key means, so the two never disagree.
            #if os(macOS)
            if let key = (NSApp as NSApplication?)?.keyWindow, key.firstResponder is NSText { key.makeFirstResponder(nil) }
            #endif
        }
    }

    /// Return listed the results: the keys leave the field for THEM, and the screen stays the search (no `release`: that leaves for the inbox).
    /// ↑ ↓ (and J K) move, Return opens, and every key of the list acts on the selected result, the mail the reader shows (AppModel.shownRow):
    /// R replies, E marks done, H snoozes. `/`, ⌘K or a click on the field gives the cursor back with the words kept. It is the foot strip's
    /// "After Enter" state (MacFootHints.searchSet) and the preset this app copies. Lead, 13 Nov, owner A162, real-engine walk r1 on main117:
    /// with the cursor left in the field, a real R typed "r" into the search words and the reply's text followed it there.
    func toResults() {
        guard holds else { return }
        if app.typed == held { app.typed = stash }   // nobody else rewrote the buffer: hand it back as it was
        owns = false; dismissed = true; caret = false
        releasedBy = "results-listed"
        #if os(macOS)
        // the text cursor leaves in the same turn as the keys (as in `release`); the hidden test copy has no key window, so the main window is asked too
        for w in [(NSApp as NSApplication?)?.keyWindow, WindowBridge.mainWindow()] {
            if let w, w.firstResponder is NSText { w.makeFirstResponder(nil) }
        }
        #endif
    }

    /// The field let go but still shows words: Esc (with the list holding the keys) clears them.
    func releasedWithText(_ m: AppModel) -> Bool { active && !holds && m.surface == "mac-main" && m.overlay == nil }

    /// Another surface took the keys (the quick reply on R, the reader, the palette): the field lets go without touching the buffer.
    func surfaceMoved() { if owns, app.surface != Self.surfaceId { owns = false; dismissed = true; releasedBy = "surface-moved:" + app.surface } }

    /// A row took the focus (a click on a list row, J or K): the field lets go. Only a row change AFTER the claim counts: SwiftUI
    /// can deliver a click's focusIndex change (0 to 1) after `/` already claimed the field, and that late change must not release it.
    /// A change of the list's own contents is not a row taking the focus: new mail or a sync that shrinks the list moves `focusIndex` (AppModel.refresh
    /// clamps it, AppModel.swift:76) with no pointer or key anywhere near the list. That change re-anchors the claim and leaves the field holding the keys.
    func focusMoved() {
        guard holds, !naming, app.focusIndex != claimedAt else { return }
        // a change of the list's own contents, or the same mail at another index (a refresh after a reply was put away as a draft re-orders the rows),
        // is not a row taking the focus: re-anchor and keep the keys (main93 to main100: ⌘K after Enter and Esc on the list let go at once, the bar trace
        // said surface mac-main and holds False right after ⌘K)
        if app.rows.count != claimedRows || app.focusedMessageId == claimedId || !app.rows.contains(where: { $0.messageId == claimedId }) {
            claimedRows = app.rows.count; claimedAt = app.focusIndex; claimedId = app.focusedMessageId; return
        }
        release("list-took-the-focus")
    }

    /// A mouse press at this point (top-left origin, the window's content): inside the field or its dropdown it keeps the keys, anywhere else it lets go.
    func pressed(at p: CGPoint) {
        guard holds, !naming else { return }
        if fieldFrame.contains(p) || (dropdownVisible && dropdownFrame.contains(p)) { return }
        release("press-outside-the-field")
    }
}

/// Reports a view's frame in the window (top-left origin) to a closure.
struct FrameReader: View {
    let onChange: (CGRect) -> Void
    var body: some View {
        GeometryReader { g in
            Color.clear
                .onAppear { onChange(g.frame(in: .global)) }
                .onChange(of: g.frame(in: .global)) { _, f in onChange(f) }
        }
    }
}

/// While the field is on screen, a mouse press outside it (and its dropdown) makes it let go. The press itself still goes on to what was clicked.
@MainActor
final class ClickAway {
    static let shared = ClickAway()
    #if os(macOS)
    private var token: Any?
    func start(_ model: SearchMacModel) {
        guard token == nil else { return }
        token = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak model] e in
            MainActor.assumeIsolated {
                guard let model, model.holds, let v = e.window?.contentView else { return }
                let p = v.convert(e.locationInWindow, from: nil)
                model.pressed(at: CGPoint(x: p.x, y: v.isFlipped ? p.y : v.bounds.height - p.y))
            }
            return e
        }
    }
    func stop() { if let t = token { NSEvent.removeMonitor(t); token = nil } }
    #else
    func start(_ model: SearchMacModel) {}
    func stop() {}
    #endif
}
