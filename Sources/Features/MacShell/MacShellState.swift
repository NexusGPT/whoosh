import Observation
import SwiftUI

/// The window state only the shell owns: which pane has focus, which side panes are collapsed, the pane
/// widths as drawn, the text size step. Data stays in AppModel and the core; nothing here copies a count.
@MainActor @Observable
public final class MacShellState {
    public static let shared = MacShellState()

    public enum Pane: String, CaseIterable, Sendable { case sidebar, list, reader }

    public var focusedPane: Pane = .list
    public var sidebarCollapsed = false
    public var listCollapsed = false
    public private(set) var paneWidths: [Pane: CGFloat] = [:]
    public var textScaleIndex = MacShellGeometry.textScales.firstIndex(of: 1) ?? 0
    public var messageWindows: [String] = []
    /// True after a key moved the focus between panes (a click clears it): the focused pane then shows the keyboard-focus ring.
    public var focusByKey = false
    /// The last message link ⌃/ copied (whoosh://message/<id>); the shell row reports it.
    public private(set) var lastCopiedLink: String?
    func noteCopied(_ link: String) { lastCopiedLink = link }

    private init() {}

    public var textScale: CGFloat { MacShellGeometry.textScales[textScaleIndex] }

    /// The panes a focus move can land on: a collapsed pane is skipped, the reader always stays.
    var visiblePanes: [Pane] {
        Pane.allCases.filter { p in
            switch p {
            case .sidebar: return !sidebarCollapsed
            case .list: return !listCollapsed
            case .reader: return true
            }
        }
    }

    func moveFocus(_ delta: Int) -> Bool {
        let panes = visiblePanes
        guard panes.count > 1 else { return false }
        let i = panes.firstIndex(of: focusedPane) ?? 0
        focusedPane = panes[(i + delta + panes.count) % panes.count]
        return true
    }

    /// ← and → (Superhuman Focus): one pane to the left or right among the visible ones; false at the edge, where the key passes on.
    func shiftFocus(_ delta: Int) -> Bool {
        let panes = visiblePanes
        guard let i = panes.firstIndex(of: focusedPane), panes.indices.contains(i + delta) else { return false }
        focusedPane = panes[i + delta]
        focusByKey = true
        return true
    }

    func toggle(_ pane: Pane) -> Bool {
        switch pane {
        case .sidebar: sidebarCollapsed.toggle()
        case .list: listCollapsed.toggle()
        case .reader: return false
        }
        if !visiblePanes.contains(focusedPane) { focusedPane = .reader }
        return true
    }

    func stepText(_ delta: Int) -> Bool {
        let next = textScaleIndex + delta
        guard MacShellGeometry.textScales.indices.contains(next) else { return false }
        textScaleIndex = next
        return true
    }

    func resetText() -> Bool {
        textScaleIndex = MacShellGeometry.textScales.firstIndex(of: 1) ?? 0
        return true
    }

    func record(_ widths: [Pane: CGFloat]) {
        if widths != paneWidths { paneWidths = widths }
    }

    /// Esc walks one step back toward the list (owner 2026-11-13: "impossible to go back to list with esc"): an open reply closes first (its words stay as the draft),
    /// then a mail that is open or a reader that holds the pane focus gives the keyboard back to the list (the reader may stay drawn beside it; J, K and E act on rows).
    /// False when there is nothing to walk back from (the list already owns the keys), so the caller's own Esc follows. Nothing is laid over the window.
    @discardableResult
    func backToList(_ m: AppModel) -> Bool {
        guard m.overlay == nil else { return false }
        let reply = QuickReplyModel.shared
        // an idle reply (open, no word, the cursor elsewhere: the state after a send landed) folds and this same Esc goes on to the list
        if reply.isOpen, [QuickReplyIds.surface, SurfaceSlot.macMain.id, ReaderState.surface].contains(m.surface), !reply.foldIfIdle(m) { return reply.close(m) }
        let readerOpen = m.surface == ReaderState.surface
        guard readerOpen || (focusedPane == .reader && !listCollapsed) else { return false }
        m.surface = SurfaceSlot.macMain.id
        // No outline is drawn around the list: the selected row is the mark. The outline (`focusByKey`) is for a move between panes by the arrow keys; since
        // 46ebcdc0d this step switched it on, and every Esc out of a mail left a blue outline around the list column until the next click (lead, 13 Nov,
        // pictures of a walk on main148: Enter, then Esc).
        if !listCollapsed { focusedPane = .list; focusByKey = false }
        return true
    }

    /// Esc on the window, after any later feature (thread, palette, trainer) declined it:
    /// an overlay closes, another surface returns to the inbox, a side pane hands focus back to the list.
    func escape(_ m: AppModel) -> Bool {
        if m.overlay != nil { m.overlay = nil; return true }
        if m.surface != "mac-main" { m.surface = "mac-main"; return true }
        if focusedPane != .list, !listCollapsed { focusedPane = .list; return true }
        return false
    }
}

/// The shell hands these to the panes it hosts; the sidebar, list and reader lanes read them.
private struct FocusedPaneKey: EnvironmentKey { static let defaultValue: MacShellState.Pane? = nil }
private struct TextScaleKey: EnvironmentKey { static let defaultValue: CGFloat = 1 }

public extension EnvironmentValues {
    var macShellFocusedPane: MacShellState.Pane? {
        get { self[FocusedPaneKey.self] }
        set { self[FocusedPaneKey.self] = newValue }
    }
    /// ⌘+ / ⌘− step: a pane multiplies its token sizes by this (1 = the locked ramp).
    var whooshTextScale: CGFloat {
        get { self[TextScaleKey.self] }
        set { self[TextScaleKey.self] = newValue }
    }
}
