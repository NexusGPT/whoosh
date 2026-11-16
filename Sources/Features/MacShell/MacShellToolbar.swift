import SwiftUI

/// The window's action controls, at the trailing end of the title band (owner 2026-11-08, "you don't show enough the shortcuts"):
/// compose, reply, reply all, forward, archive, delete, junk, snooze, label, more. Each runs the engine's command through the
/// same CommandRegistry a key does, shows its key in the tooltip, and greys out with no message selected (compose needs none).
struct MacToolbarItem: Identifiable {
    let command: String, symbol: String, word: String, needsSelection: Bool
    var id: String { command }
    static let all: [MacToolbarItem] = [
        .init(command: "compose", symbol: "square.and.pencil", word: "Compose", needsSelection: false),
        .init(command: "reply", symbol: "arrowshape.turn.up.left", word: "Reply", needsSelection: true),
        .init(command: "reply-all", symbol: "arrowshape.turn.up.left.2", word: "Reply all", needsSelection: true),
        .init(command: "forward", symbol: "arrowshape.turn.up.right", word: "Forward", needsSelection: true),
        .init(command: "archive", symbol: "archivebox", word: "Archive", needsSelection: true),
        .init(command: "delete", symbol: "trash", word: "Delete", needsSelection: true),
        .init(command: "junk", symbol: "xmark.bin", word: "Junk", needsSelection: true),
        .init(command: "snooze", symbol: "clock", word: "Snooze", needsSelection: true),
        .init(command: "label", symbol: "tag", word: "Label", needsSelection: true),
    ]
    /// What "More" holds.
    static let more: [MacToolbarItem] = [
        .init(command: "unread", symbol: "envelope.badge", word: "Mark read or unread", needsSelection: true),
        .init(command: "star", symbol: "star", word: "Star", needsSelection: true),
        .init(command: "mute", symbol: "bell.slash", word: "Mute", needsSelection: true),
        .init(command: "move", symbol: "folder", word: "Move", needsSelection: true),
        .init(command: "pin", symbol: "pin", word: "Pin", needsSelection: true),
    ]
}

@MainActor
enum MacToolbarRun {
    static func enabled(_ i: MacToolbarItem, _ m: AppModel) -> Bool {
        if i.command == "reply-all" { return m.shownMessageId != nil && ShortcutEngine.threadIsOpen(m) }   // its key is Enter inside a thread
        return !i.needsSelection || m.shownMessageId != nil
    }
    static func help(_ i: MacToolbarItem) -> String {
        Words.text(i.word) + (MacHintTable.keyLabel(i.command).map { "  \(KeyCap.glyph(for: $0))" } ?? "")
    }
    @discardableResult static func run(_ i: MacToolbarItem, _ m: AppModel) -> Bool {
        guard MacShellGeometry.mutant != "dead-toolbar", enabled(i, m), let h = CommandRegistry.handler(i.command) else { return false }
        return h(m)
    }
}

struct MacToolbarButton: View {
    let item: MacToolbarItem
    let model: AppModel
    @State private var hover = false
    var body: some View {
        let on = MacToolbarRun.enabled(item, model)
        Button { MacToolbarRun.run(item, model) } label: {
            Image(systemName: item.symbol).font(.system(size: WhooshMetrics.actionIcon - 1, weight: .regular))
                .foregroundStyle(on ? WhooshPalette.secondaryLabel : WhooshPalette.tertiaryLabel)
                .frame(width: MacToolbarView.cell, height: MacToolbarView.cell)
                .background(hover && on ? AnyShapeStyle(WhooshPalette.hover) : AnyShapeStyle(.clear), in: RoundedRectangle(cornerRadius: WhooshRadius.row, style: .continuous))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(!on)
        .onHover { hover = $0 }
        .animation(.easeOut(duration: Double(WhooshConstants.HOVER_TINT_MS) / 1000), value: hover)
        .help(MacToolbarRun.help(item))
        .accessibilityLabel(Words.text(item.word))
        .accessibilityIdentifier("mac-main.toolbar.\(item.command)")
    }
}

struct MacToolbarView: View {
    @State private var model = AppModel.shared
    @State private var hints = MacHintState.shared
    static let cell: CGFloat = 28
    static var height: CGFloat { cell + 1 + WhooshMetrics.keyCap }
    var body: some View {
        VStack(spacing: 1) {
            icons
            capRow   // ⌘ held: one key under each icon
        }
        .padding(.horizontal, WhooshSpacing.s4)
        .frame(maxHeight: .infinity, alignment: .top)
        .ignoresSafeArea()
    }

    private var capRow: some View {
        HStack(spacing: WhooshSpacing.s2) {
            ForEach(MacToolbarItem.all) { i in
                ZStack { if let k = MacHintTable.keyLabel(i.command) { KeyCap(k).fixedSize() } }.frame(width: Self.cell, height: WhooshMetrics.keyCap)
            }
            Color.clear.frame(width: Self.cell, height: 1)
        }
        .opacity(hints.visible ? 1 : 0)
        .animation(.easeOut(duration: Double(WhooshConstants.POPOVER_MS) / 1000), value: hints.visible)
        .allowsHitTesting(false)
    }

    private var icons: some View {
        HStack(spacing: WhooshSpacing.s2) {
            ForEach(MacToolbarItem.all) { MacToolbarButton(item: $0, model: model) }
            Menu {
                ForEach(MacToolbarItem.more) { i in
                    Button { MacToolbarRun.run(i, model) } label: {
                        Label(MacToolbarRun.help(i), systemImage: i.symbol)
                    }.disabled(!MacToolbarRun.enabled(i, model))
                }
            } label: {
                Image(systemName: "ellipsis").font(.system(size: WhooshMetrics.actionIcon - 1, weight: .regular))
                    .foregroundStyle(model.shownMessageId != nil ? WhooshPalette.secondaryLabel : WhooshPalette.tertiaryLabel)
                    .frame(width: Self.cell, height: Self.cell)
            }
            .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
            .disabled(model.shownMessageId == nil)
            .help(Words.text("More"))
            .accessibilityIdentifier("mac-main.toolbar.more")
        }
    }

    static var width: CGFloat { CGFloat(MacToolbarItem.all.count + 1) * (cell + WhooshSpacing.s2) + WhooshSpacing.s8 }
}

/// Rows for `rows mac-main.toolbar`: one per control, with its enabled state and tooltip.
@MainActor
enum MacToolbarRows {
    static func rows(_ m: AppModel) -> [ControlRow] {
        (MacToolbarItem.all + MacToolbarItem.more).map { i in
            ["ax": "mac-main.toolbar.\(i.command)", "command": i.command, "enabled": MacToolbarRun.enabled(i, m),
             "tooltip": MacToolbarRun.help(i), "in_menu": MacToolbarItem.more.contains { $0.command == i.command }]
        }
    }
}

#if os(macOS)
import AppKit

/// Puts the toolbar into the window's frame view, on the traffic lights' line, pinned to the trailing edge. One per window.
@MainActor
final class MacToolbarHost {
    static let shared = MacToolbarHost()
    private var host: NSHostingView<MacToolbarView>?
    /// The icon row in window points (origin top-left); kept here because the frame registry is cleared when its probe goes idle.
    private(set) var iconRow: CGRect?

    func republish() { if let r = iconRow { FrameProbe.set("mac-main.toolbar", r) } }

    func install(in window: NSWindow) {
        guard let frameView = window.contentView?.superview else { return }
        if let h = host, h.superview === frameView { place(h, window: window, frameView: frameView); return }
        host?.removeFromSuperview()
        let h = NSHostingView(rootView: MacToolbarView())
        h.safeAreaRegions = []
        h.sizingOptions = []
        h.autoresizingMask = [.minYMargin, .minXMargin]
        frameView.addSubview(h)
        host = h
        place(h, window: window, frameView: frameView)
    }

    private func place(_ h: NSHostingView<MacToolbarView>, window: NSWindow, frameView: NSView) {
        let barHeight = window.frame.height - window.contentLayoutRect.height
        let line = (barHeight > 0 ? barHeight : WhooshMetrics.sidebarTop - WhooshSpacing.s20) / 2
        let size = CGSize(width: MacToolbarView.width, height: MacToolbarView.height)
        let top = line - MacToolbarView.cell / 2   // the icon row sits on the lights' line; the cap row hangs below it
        let y = frameView.isFlipped ? top : frameView.bounds.height - top - size.height
        h.frame = CGRect(x: frameView.bounds.width - size.width - WhooshSpacing.s12, y: y, width: size.width, height: size.height)
        iconRow = CGRect(x: h.frame.minX, y: top, width: size.width, height: MacToolbarView.cell)
        republish()
    }
}
#endif
