import Foundation
#if os(macOS)
import AppKit
#endif

/// `uistate.first_responder`: where the next key goes, as a stable id the checks can assert on
/// (docs/lanes/appshell.md "## Round 2"): `list`, `reader`, `search-field`, `quick-reply`, `palette`, `composer`, or `other:<ax-id or class>`.
/// Read from the key window's first responder: a text field's editor maps through its owning view's accessibility id
/// (the `.whooshAx` ids of the features), any other responder in the main window means the keys reach the list.
@MainActor
public enum FirstResponder {
    static func id() -> String {
        #if os(macOS)
        guard let w = NSApp.keyWindow ?? NSApp.mainWindow ?? WindowBridge.mainWindow() else { return "none" }
        if WindowBridge.surfaceId(of: w) == "mac-palette" { return "palette" }
        // The reply editor is a plain NSTextView in the reader pane: no view of its chain carries the field's id (the id is on a SwiftUI container), so the chain reads `reader`. The editor's own focus flag is the truth.
        if let tv = w.firstResponder as? NSTextView, !tv.isFieldEditor, w == WindowBridge.mainWindow(),
           QuickReplyModel.shared.isOpen, QuickReplyModel.shared.fieldFocused { return "quick-reply" }
        let chain = responderIds(w.firstResponder)
        for raw in chain {
            let s = raw.lowercased()
            if s.contains("palette") { return "palette" }
            if s.contains("quickreply") || s.contains("quick-reply") { return "quick-reply" }
            if s.contains("search") { return "search-field" }
            if s.contains("composer") { return "composer" }
            if s.contains("reader") { return "reader" }
            if s.hasPrefix("mac-main.sidebar") { return "sidebar" }   // macshell hook: ← moves the keyboard focus to the sidebar pane
            if s.contains("list") { return "list" }
        }
        let isText = w.firstResponder is NSTextView || w.firstResponder is NSTextField
        if !isText, (w.identifier?.rawValue ?? "").hasPrefix(SurfaceSlot.macMain.id) || w == WindowBridge.mainWindow() { return "list" }
        return "other:" + (chain.first ?? String(describing: type(of: w.firstResponder as Any)))
        #else
        return "none"
        #endif
    }

    #if os(macOS)
    /// Accessibility ids from the responder (its owning view when it is a field editor) up to the window.
    private static func responderIds(_ r: NSResponder?) -> [String] {
        var view: NSView? = r as? NSView
        if let tv = r as? NSTextView, tv.isFieldEditor, let d = tv.delegate as? NSView { view = d }
        var out: [String] = []
        while let v = view {
            let id = v.accessibilityIdentifier()
            if !id.isEmpty { out.append(id) }
            view = v.superview
        }
        return out
    }
    #endif
}
