import SwiftUI

/// The values the step needs, in two columns: a Copy button (with its key) for what is pasted into Entra, a tag for what is picked in a list.
/// The row for the current step glows; the rest stay as a reminder of the form.
struct EntraGuideValuesView: View {
    let model: EntraGuideModel
    @Environment(\.accessibilityReduceMotion) private var reduce

    var body: some View {
        // a link is long: its row takes the whole width (one column), the form's short values stay in two
        let wide = model.values.contains { $0.def.dynamic != nil }
        LazyVGrid(columns: wide ? [GridItem(.flexible())] : [GridItem(.flexible(), spacing: 8), GridItem(.flexible(), spacing: 8)], spacing: 8) {
            ForEach(model.values) { row in
                EntraGuideValueRow(model: model, row: row)
            }
        }
        .animation(.easeOut(duration: 0.08), value: model.number)
        .accessibilityElement(children: .contain).accessibilityIdentifier("\(EntraGuideWords.surface).values")
    }
}

struct EntraGuideValueRow: View {
    let model: EntraGuideModel
    let row: EntraGuideModel.ValueRow
    @Environment(\.accessibilityReduceMotion) private var reduce

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous)
        HStack(spacing: 10) {
            VStack(alignment: .leading, spacing: 0) {
                Text(Words.text(row.def.label)).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                Text(row.value).font(row.def.copy ? .system(size: 13, design: .monospaced) : .system(size: 13))
                    .foregroundStyle(WhooshPalette.label).lineLimit(1).truncationMode(row.def.dynamic == nil ? .tail : .middle).help(row.value)
            }
            Spacer(minLength: 0)
            if row.def.copy { copyButton } else {
                Text(EntraGuideWords.pickTag).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                    .padding(.horizontal, 8).frame(height: 20).background(WhooshPalette.fill, in: Capsule()).padding(.trailing, 6)
            }
        }
        .padding(.leading, 12).padding(.trailing, 6).frame(height: EntraGuideMetrics.valueHeight)
        .background(WhooshPalette.field, in: shape)
        .overlay(shape.strokeBorder(row.current ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(WhooshPalette.sheetFieldBorder), lineWidth: 1))
        .overlay { shape.stroke(WhooshPalette.focusHalo, lineWidth: 3).padding(-1.5).opacity(row.current ? 1 : 0) }
        .accessibilityElement(children: .contain).accessibilityIdentifier("\(EntraGuideWords.surface).value.\(row.def.label)")
    }

    private var copyButton: some View {
        let done = model.copied == row.number
        return Button { _ = model.copy(row.number) } label: {
            ZStack {
                HStack(spacing: 7) { Text(EntraGuideWords.copy).whooshText(WhooshTypography.body); Text("⌘\(row.number)").whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel) }
                    .opacity(done ? 0 : 1).scaleEffect(done ? 0.9 : 1)
                HStack(spacing: 6) { Image(systemName: "checkmark").font(.system(size: 10, weight: .bold)); Text(EntraGuideWords.copied).whooshText(WhooshTypography.body, emphasized: true) }
                    .foregroundStyle(WhooshPalette.guideOK).opacity(done ? 1 : 0).scaleEffect(done ? 1 : 0.9)
            }
            .frame(width: 84)
        }
        .buttonStyle(EntraGuideButtonStyle(look: .standard))
        .animation(EntraGuideMetrics.motion(.easeOut(duration: EntraGuideMetrics.fadeS), reduce), value: done)
        .help(EntraGuideWords.tooltip(EntraGuideWords.copy, "⌘\(row.number)"))
        .accessibilityIdentifier("\(EntraGuideWords.surface).copy.\(row.number)")
    }
}
