import SwiftUI

/// The ⇧? sheet: every key of the preset in force with its action, the ones the owner already presses marked.
/// It scales in from POPOVER_SCALE_FROM over POPOVER_MS (a cross-fade under Reduced Motion). Esc, the scrim and the
/// show-keys key close it; none of them waits for the animation.
public struct TrainerOverlayView: View {
    @State private var model = AppModel.shared
    @State private var trainer = TrainerModel.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    public init() {}

    public var body: some View {
        ZStack {
            if trainer.isOpen(model) {
                Rectangle().fill(WhooshPalette.background.opacity(0.5)).ignoresSafeArea()
                    .contentShape(Rectangle())
                    .onTapGesture { _ = trainer.close(model) }
                    .transition(.opacity)
                    .whooshAx("mac-trainer.overlay.close")   // the scrim has its own id and a frame (the door of the same id runs the same close)
                    .accessibilityHidden(true)
                KeysSheet(rows: trainer.keyRows, presetName: TrainerModel.shared.engine.preset.name)
                    .transition(reduceMotion ? .opacity : .opacity.combined(with: .scale(scale: CGFloat(WhooshConstants.POPOVER_SCALE_FROM))))
            }
        }
        .animation(.easeOut(duration: reduceMotion ? WhooshMotion.readerCrossFade : Double(WhooshConstants.POPOVER_MS) / 1000),
                   value: trainer.isOpen(model))
    }
}

struct KeysSheet: View {
    let rows: [TrainerModel.KeyRow]
    let presetName: String

    private var learned: Int { rows.filter(\.used).count }
    private let columns = [GridItem(.flexible(), spacing: WhooshSpacing.s24, alignment: .top),
                           GridItem(.flexible(), spacing: WhooshSpacing.s24, alignment: .top)]

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s16) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                    Text("Keys").whooshText(WhooshTypography.title3, emphasized: true).foregroundStyle(WhooshPalette.label)
                    Text("\(presetName) · \(learned) of \(rows.count) learned")
                        .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                        .contentTransition(.numericText())
                }
                Spacer(minLength: WhooshSpacing.s16)
                HStack(spacing: WhooshSpacing.s6) {
                    Text("Close").whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    KeyCap("Esc")
                }
            }
            Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator)
            LazyVGrid(columns: columns, alignment: .leading, spacing: WhooshSpacing.s2) {
                ForEach(rows, id: \.command) { KeyLine(row: $0) }
            }
        }
        .padding(WhooshSpacing.s24)
        .frame(width: WhooshMetrics.readerMeasure)
        .background(WhooshPalette.background, in: RoundedRectangle(cornerRadius: WhooshRadius.sheet, style: .continuous))
        .whooshGlass(in: RoundedRectangle(cornerRadius: WhooshRadius.sheet, style: .continuous))
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("mac-trainer.overlay")
    }
}

/// One key: its action, its cap, and a mark that says learned in shape (a check) as well as colour.
struct KeyLine: View {
    let row: TrainerModel.KeyRow

    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Image(systemName: row.used ? "checkmark.circle.fill" : "circle")
                .font(WhooshTypography.subhead.font(emphasized: false))
                .foregroundStyle(row.used ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(WhooshPalette.tertiaryLabel))
                .frame(width: WhooshMetrics.actionIcon)
            Text(row.action.prefix(1).uppercased() + row.action.dropFirst())
                .whooshText(WhooshTypography.body).foregroundStyle(row.used ? WhooshPalette.label : WhooshPalette.secondaryLabel)
                .lineLimit(1)
            Spacer(minLength: WhooshSpacing.s8)
            KeyCap(row.chord)
        }
        .frame(minHeight: WhooshMetrics.actionButton)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("\(row.action), key \(row.chord), \(row.used ? "learned" : "not used yet")")
    }
}
