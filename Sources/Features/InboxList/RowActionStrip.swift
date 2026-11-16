import SwiftUI

/// One control of the hover strip: its command id (the shortcuts engine's), glyph, word and Superhuman key.
struct StripAction: Identifiable, Sendable {
    let id: String
    let symbol: String
    let word: String
    let key: String
}

/// The strip on a row (docs/PRD.md "Controls on every email"): archive, delete, junk, snooze, label, each with its
/// glyph and its key in the tooltip, then "Why?". The key is read from the live preset, so a rebound key shows as bound.
@MainActor
enum RowStrip {
    static let items: [StripAction] = [
        StripAction(id: "archive", symbol: "archivebox", word: "Archive", key: "E"),
        StripAction(id: "delete", symbol: "trash", word: "Delete", key: "⇧#"),
        StripAction(id: "junk", symbol: "xmark.bin", word: "Junk", key: "⇧!"),
        StripAction(id: "snooze", symbol: "clock", word: "Snooze", key: "H"),
        StripAction(id: "label", symbol: "tag", word: "Label", key: "L"),
    ]

    /// the label control also names the key that takes a label off ("Label · L · Remove · Y")
    static func help(_ a: StripAction) -> String {
        let base = LiveKeys.help(a.word, a.id, a.key)
        return a.id == "label" ? base + " · " + Words.text("Remove") + " · " + LiveKeys.chord("remove-label", "Y") : base
    }
}

/// The hovered or settled row's actions. Each runs the engine's command for the row (a click counts as a mouse use of it).
struct RowActions: View {
    let tint: Color
    let messageId: String
    let onAction: (String) -> Void
    /// The narrow strip (archive, snooze, label): drawn when the full one would cut the sender's name (InboxRowView.lineOne).
    var short = false

    var body: some View {
        HStack(spacing: WhooshSpacing.s2) {
            ForEach(short ? RowStrip.items.filter { ["archive", "snooze", "label"].contains($0.id) } : RowStrip.items) { item in
                Button { onAction(item.id) } label: { glyph(item.symbol) }
                    .buttonStyle(.plain).foregroundStyle(tint).help(RowStrip.help(item)).accessibilityLabel(RowStrip.help(item))
                    .whooshAx("mac-main.list.row.\(messageId).act.\(item.id)")
            }
            if !short { Button { onAction("why") } label: { glyph("questionmark.circle") }
                .buttonStyle(.plain).foregroundStyle(tint).help(Words.text("Why is it here?")).accessibilityLabel(Words.text("Why is it here?")) }
        }
    }

    private func glyph(_ symbol: String) -> some View {
        Image(systemName: symbol).font(WhooshTypography.subhead.font(emphasized: false))
            .frame(width: WhooshMetrics.actionIcon + WhooshSpacing.s4, height: WhooshMetrics.listRowLine)
    }
}
