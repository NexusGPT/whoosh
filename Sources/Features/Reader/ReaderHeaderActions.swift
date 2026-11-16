import SwiftUI

/// One of the thread's actions in the header band: its control id (`mac-main.reader.key.<id>`), the engine command it
/// runs and the key the active preset shows for it.
struct ReaderAction: Identifiable {
    let id: String
    let command: String
    let label: String
    let symbol: String
    var key: String { ReaderKeyLabel.chord(for: command) }
    var help: String { key.isEmpty ? label : "\(label)  \(key)" }

    static var all: [ReaderAction] {
        [ReaderAction(id: "archive", command: "archive", label: Words.text("Archive"), symbol: "archivebox"),
         ReaderAction(id: "snooze", command: "snooze", label: Words.text("Snooze"), symbol: "clock"),
         ReaderAction(id: "label", command: "label", label: Words.text("Label"), symbol: "tag"),
         ReaderAction(id: "pin", command: "pin", label: Words.text("Pin"), symbol: "pin"),
         ReaderAction(id: "reply", command: "reply", label: Words.text("Reply"), symbol: "arrowshape.turn.up.left"),
         ReaderAction(id: "more", command: "palette", label: Words.text("More"), symbol: "ellipsis")]
    }
}

/// The thread's actions as icon buttons in the header band, each with its key in the tooltip. A press runs the engine
/// command the key runs (ReaderActions.run), so a preset that moves a key moves nothing here but the tooltip.
struct ReaderHeaderActions: View {
    var body: some View {
        HStack(spacing: WhooshSpacing.s2) {
            ForEach(ReaderAction.all) { a in
                ReaderIconButton(symbol: a.symbol, help: a.help) { _ = ReaderActions.run(a.command) }
                    .whooshAx("mac-main.reader.key.\(a.id)")
                    .accessibilityLabel(a.label)
                    .overlay(alignment: .bottom) { ReaderHoldCap(key: a.key) }
            }
            // labels r2 hook: the label inspector's button, far right of the toolbar (⌘I)
            Rectangle().fill(WhooshPalette.separator).frame(width: 1, height: WhooshSpacing.s16).padding(.horizontal, WhooshSpacing.s4)
            InspectorToggleButton(on: false) { _ = LabelInspector.toggle() }
        }
        .fixedSize()
    }
}

struct ReaderIconButton: View {
    let symbol: String
    let help: String
    let action: () -> Void
    @State private var hovering = false  // local-ui: hover tint only

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: WhooshTypography.body.size, weight: .medium))
                .foregroundStyle(hovering ? WhooshPalette.label : WhooshPalette.secondaryLabel)
                .frame(width: WhooshSpacing.s32, height: Self.height)
                .background { RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).fill(WhooshPalette.hover).opacity(hovering ? 1 : 0) }
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .animation(WhooshMotion.hover, value: hovering)
        .help(help)
    }
    static let height = WhooshSpacing.s24 + WhooshSpacing.s4
}
