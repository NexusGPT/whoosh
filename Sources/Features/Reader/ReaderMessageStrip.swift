import SwiftUI

/// One button of the strip: its control id (`mac-main.reader.strip.<id>`), the engine command it runs and who answers
/// that command today (`door`, read by the probe). The key shown is the active preset's key for the command.
struct ReaderStripItem: Identifiable {
    let id: String
    let symbol: String
    let label: String
    let command: String
    let door: String

    var key: String { ReaderKeyLabel.chord(for: command) }

    /// Delete and reply are greyed while the quick reply is open: the field holds the keys, and the reply is already there.
    /// Reply all (the reply box with every recipient) and forward (the composer window) each have a live door now
    /// (ReplyDoors in Shortcuts/Commands.swift), so both are drawn live like the others.
    func enabled(quickOpen: Bool) -> Bool { quickOpen ? !["delete", "reply"].contains(id) : true }

    func tooltip(quickOpen: Bool) -> String {
        let base = key.isEmpty ? label : "\(label)  \(key)"
        return enabled(quickOpen: quickOpen) ? base : "\(base) · \(ReaderWords.closeReplyFirst)"
    }

    static var all: [ReaderStripItem] {
        [ReaderStripItem(id: "delete", symbol: "trash", label: Words.text("Delete"), command: "delete", door: "reader"),
         ReaderStripItem(id: "reply", symbol: "arrowshape.turn.up.left", label: Words.text("Reply"), command: "reply", door: "quickreply"),
         ReaderStripItem(id: "replyall", symbol: "arrowshape.turn.up.left.2", label: Words.text("Reply all"), command: "reply-all", door: "quickreply"),
         ReaderStripItem(id: "forward", symbol: "arrowshape.turn.up.right", label: Words.text("Forward"), command: "forward", door: "composer"),
         ReaderStripItem(id: "archive", symbol: "archivebox", label: Words.text("Archive"), command: "archive", door: "inboxlist")]
    }
}

/// The key the active preset binds to an engine command, in the engine's spelling ("⇧#", "R", "Enter"). Reply all has
/// no row of its own in the Superhuman preset: Enter in an open thread is it.
enum ReaderKeyLabel {
    /// Always called on the main thread (a view, a probe, a click); the engine is main-actor state.
    static func chord(for command: String) -> String {
        MainActor.assumeIsolated {
            let engine = ShortcutEngine.shared
            if let c = engine.chord(for: command) { return c }
            if command == "reply-all" { return engine.chord(for: "enter") ?? "" }
            return readerChord(for: command) ?? ""
        }
    }

    /// The row chord, else the first context chord the active preset gives the command (Tab for cycle-links).
    @MainActor static func readerChord(for command: String) -> String? {
        let engine = ShortcutEngine.shared
        return engine.chord(for: command) ?? engine.preset.contextKeys.first { $0.command == command }?.chord
    }
}

/// The strip on the open message's header (Apple Mail's, in Whoosh's glass): delete, reply, reply all, forward, archive
/// as one rounded group at the header's right. It fades in over HOVER_REVEAL_MS while the pointer is on the header, and
/// stays when the reader pane holds the keyboard focus. Each button runs the engine command its key runs.
struct ReaderMessageStrip: View {
    static let buttonWidth = WhooshSpacing.s24 + WhooshSpacing.s4
    let visible: Bool
    let quickOpen: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: WhooshRadius.field + WhooshSpacing.s2, style: .continuous)
        HStack(spacing: 0) {
            ForEach(ReaderStripItem.all) { item in
                let enabled = item.enabled(quickOpen: quickOpen)
                Button { _ = ReaderActions.run(item.command) } label: {
                    Image(systemName: item.symbol)
                        .font(.system(size: WhooshTypography.caption.size, weight: .medium))
                        .frame(width: Self.buttonWidth, height: WhooshSpacing.s24)
                        .contentShape(Rectangle())
                }
                .buttonStyle(ReaderStripButtonStyle(enabled: enabled))
                .disabled(!enabled)
                .help(item.tooltip(quickOpen: quickOpen))
                .whooshAx("mac-main.reader.strip.\(item.id)")
                .accessibilityLabel(item.label)
            }
        }
        .padding(WhooshSpacing.s2)
        .background(WhooshPalette.background.opacity(0.6), in: shape)
        .whooshGlass(in: shape)
        .overlay(shape.strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator))
        .opacity(visible ? 1 : 0)
        .animation(WhooshMotion.animation(Animation.easeOut(duration: Double(WhooshConstants.HOVER_REVEAL_MS) / 1000), reduceMotion: reduceMotion), value: visible)
        .allowsHitTesting(visible)
        .accessibilityIdentifier("mac-main.reader.strip")
    }
}

/// Hover tints one step, a press darkens one step more on the same frame (no animation on the press itself).
struct ReaderStripButtonStyle: ButtonStyle {
    let enabled: Bool
    func makeBody(configuration: Configuration) -> some View {
        Inner(configuration: configuration, enabled: enabled)
    }
    private struct Inner: View {
        let configuration: ButtonStyleConfiguration
        let enabled: Bool
        @State private var hovering = false  // local-ui: hover tint only
        var body: some View {
            configuration.label
                .foregroundStyle(enabled ? (hovering ? WhooshPalette.label : WhooshPalette.secondaryLabel) : WhooshPalette.tertiaryLabel)
                .background(RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous)
                    .fill(configuration.isPressed ? WhooshPalette.separator : WhooshPalette.hover)
                    .opacity(enabled && (hovering || configuration.isPressed) ? 1 : 0))
                .onHover { hovering = $0 }
                .animation(WhooshMotion.hover, value: hovering)
        }
    }
}
