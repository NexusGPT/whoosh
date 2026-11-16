import Foundation
import Observation
import SwiftUI

/// Holding ⌘ (owner 2026-11-08 16:01, docs/PRD.md "## Hold ⌘"): after HINT_HOLD_MS the shell shows each control's key in its corner.
/// This file holds the state and the table that says which control shows which command's key; the overlay draws them
/// (MacHintOverlay.swift). Positions come from the frame registry (FrameProbe, read only): it is switched on when the hold
/// completes and off after the fade, so nothing is measured while ⌘ is not held.
struct MacHint: Identifiable, Equatable {
    let ax: String, label: String, rect: CGRect
    var id: String { ax }
}

@MainActor @Observable
final class MacHintState {
    static let shared = MacHintState()
    private(set) var visible = false
    private(set) var badges: [MacHint] = []
    private(set) var strips: [MacHintStrip] = []
    private var pending: Task<Void, Never>?
    private var follow: Task<Void, Never>?
    private var held = false
    /// While ⌘ is held the caps follow their controls: the list can still move its rows (a scroll to the focused row, the
    /// undo capsule or the suggestion entering the list's bottom inset), and a cap computed once would float off its icon
    /// (main38: J0.15 float-above red at 960x632). Re-read every HINT_FOLLOW_MS; assigned only when a frame moved.
    private static let followNs = UInt64(WhooshConstants.HINT_FOLLOW_MS) * 1_000_000
    private init() {}

    /// ⌘ went down alone: show after HINT_HOLD_MS if it is still held and nothing else was pressed.
    func holdBegan() {
        guard !held else { return }
        held = true
        pending = Task { @MainActor in
            try? await Task.sleep(nanoseconds: UInt64(WhooshConstants.HINT_HOLD_MS) * 1_000_000)
            if !Task.isCancelled, held { show() }
        }
    }

    /// ⌘ released, or another key / modifier joined it: no badges (a pressed hinted key acts as usual).
    func holdEnded() {
        held = false
        pending?.cancel(); pending = nil
        hide()
    }

    func show() {
        FrameProbe.gate.on = true   // the readers mount and publish their frames
        pending = Task { @MainActor in
            try? await Task.sleep(nanoseconds: 60_000_000)
            guard !Task.isCancelled else { return }
            let all = MacHintTable.compute()
            badges = all.badges; strips = all.strips
            visible = true
            follow?.cancel()
            follow = Task { @MainActor in
                while !Task.isCancelled, visible {
                    try? await Task.sleep(nanoseconds: Self.followNs)
                    guard !Task.isCancelled, visible else { return }
                    let now = MacHintTable.compute()
                    if now.badges != badges { badges = now.badges; strips = now.strips }
                }
            }
        }
    }

    func hide() {
        let wasShown = visible
        visible = false
        follow?.cancel(); follow = nil
        guard wasShown || FrameProbe.gate.on else { return }
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: UInt64(WhooshConstants.POPOVER_MS) * 1_000_000 + 40_000_000)
            guard !visible, !held else { return }
            badges = []; strips = []
            FrameProbe.gate.on = false
        }
    }
}

/// Which control shows which command's key. Controls are found by their accessibility ids in the frame registry.
@MainActor
enum MacHintTable {
    static var preset: KeyPreset { ShortcutEngine.shared.preset }

    /// The key written for a command in the active preset: its row chord, else a sequence ("G I"), else a second key.
    static func keyLabel(_ command: String) -> String? {
        if let c = preset.chord(for: command) { return c }
        // Reply all is Enter while a thread is open (Superhuman's meaning); with nothing open it has no key and its icon is disabled.
        if command == "reply-all" { return MacShellGeometry.mutant != "no-replyall-cap" && ShortcutEngine.threadIsOpen(AppModel.shared) ? "Enter" : nil }
        if let s = preset.sequences.first(where: { $0.command == command }) {
            return s.first + " " + (s.second.hasPrefix("⇧") && s.second.count == 2 ? String(s.second.dropFirst()) : s.second)
        }
        return preset.alsoKeys.first { $0.command == command }?.chord
    }

    /// The command an accessibility id stands for, nil when it has no key.
    static func command(for ax: String) -> String? {
        if ax.hasPrefix("mac-main.list.row."), let r = ax.range(of: ".act.") { return String(ax[r.upperBound...]) }
        if ax.hasPrefix("mac-main.sidebar.mailbox.") {
            let slug = String(ax.dropFirst("mac-main.sidebar.mailbox.".count))
            return SidebarFeature.goMailboxes.first { $0.1 == slug }?.0
        }
        if ax.hasPrefix("mac-main.splits."), let i = Int(ax.dropFirst("mac-main.splits.".count)), (0...8).contains(i) { return "switch-tab-\(i + 1)" }
        if ax.hasPrefix("mac-main.reader.strip.") {
            let id = String(ax.dropFirst("mac-main.reader.strip.".count))
            return ["replyall": "reply-all", "forward": "forward", "reply": "reply", "delete": "delete"][id]
        }
        switch ax {
        case QuickReplyIds.send: return "send"
        case QuickReplyIds.from: return "compose-from"
        default: return nil
        }
    }

    /// A control's frame in the window's content view, read from its AppKit anchor NOW (the source the socket's `measure` reads), so a cap
    /// follows its icon when a pane moves or changes width under it (the compact breakpoint at 960 pt: list right edge 521 against 621,
    /// 100 pt). The SwiftUI frame the probe caches is updated only by a SwiftUI layout pass and can lag that move; it stays the fallback
    /// for a control with no anchor in a window.
    static func liveFrame(_ ax: String) -> CGRect? {
        #if os(macOS)
        if let (r, _) = FrameProbe.contentFrame(ax) { return r }
        #endif
        return FrameProbe.frame(ax)
    }

    static func compute() -> (badges: [MacHint], strips: [MacHintStrip]) {
        if MacShellGeometry.mutant == "no-hints" { return ([], []) }
        let w = MacShellState.shared.paneWidths
        let listRight = (w[.sidebar] ?? 0) + (w[.list] ?? 0) + 2   // a tab clipped past the list's edge has no place to badge
        let found = FrameProbe.ids.compactMap { (ax: String) -> MacHint? in
            guard let r = liveFrame(ax), r.width > 0, r.height > 0, let c = command(for: ax), let k = keyLabel(c) else { return nil }
            if ax.hasPrefix("mac-main.splits."), r.minX > listRight - 48 { return nil }   // a sliver of a tab has no room for its key
            return MacHint(ax: ax, label: k, rect: r)
        }
        let strip = MacHintPlace.rowStrips(found.filter { $0.ax.contains(".act.") })
        return (MacHintPlace.place(found) + strip.caps, strip.strips)
    }

    private static func row(_ ax: String, _ label: String, _ r: CGRect, _ visible: Bool) -> ControlRow {
        ["ax": ax, "label": label, "x": r.minX, "y": r.minY, "w": r.width, "h": r.height, "visible": visible]
    }

    static func rows() -> [ControlRow] {
        let s = MacHintState.shared
        var all = s.badges
        let extra: [ControlRow] = s.strips.flatMap { st -> [ControlRow] in
            [row(st.id + ".strip", "", st.pill, s.visible)] + st.pairs.map { row($0.cap.ax + ".icon", "", $0.icon, s.visible) }
        }
        #if os(macOS)
        MacToolbarHost.shared.republish()
        if let bar = MacToolbarHost.shared.iconRow, s.visible, MacShellGeometry.mutant != "no-hints" { all += MacHintPlace.toolbarCaps(iconRow: bar) }
        #endif
        return all.map { row($0.ax, $0.label, $0.rect, s.visible) } + extra
    }
}
