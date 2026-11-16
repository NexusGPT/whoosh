import Foundation
#if os(macOS)
import AppKit
#endif

/// The one key of a Replies editor that goes through the app's bindings: Esc. Since keys-r6 (Sources/Features/Shortcuts/TextKeys.swift)
/// a focused text view gets letters, Return, Tab, arrows, ⌘Z ⇧⌘Z ⌘A ⌘X ⌘C ⌘V itself, and the chords it lists (`ownsChord`: ⌘S, ⌘↩);
/// Esc goes to the screens first, so the editor takes it here while it holds the focus: it closes the @ list, else asks once, then reverts.
@MainActor
enum RepliesEditorKeys {
    static func bindings() -> [String: (AppModel) -> Bool] {
        #if os(macOS)
        return [
            "Escape": { _ in
                guard let tv = RepliesEditorViews.focused, let c = tv.owner else { return false }
                if c.state.menuQuery != nil { c.closeMenu() }
                else if c.state.dirty || c.state.confirmRevert { c.escape() }          // changes to lose: ask once, then revert
                else { tv.window?.makeFirstResponder(nil) }                            // nothing to lose: the pane lets go, so the window's own Escape is next (par-settingswindow, main92)
                return true
            },
        ]
        #else
        return [:]
        #endif
    }
}
