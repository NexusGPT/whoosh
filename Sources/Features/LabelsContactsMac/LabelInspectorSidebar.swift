import SwiftUI

/// What the sidebar draws for the inspector (the sidebar lane mounts both, see "labels r2 hook: sidebar"): the edit ring on
/// the label being modified, and the "New" row under its parent while a new label is being defined.
struct EditRing: View {
    var body: some View {
        RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous)
            .strokeBorder(WhooshPalette.focusHalo, lineWidth: WhooshMetrics.separator)
            .overlay(alignment: .trailing) { if LabelInspectorModel.shared.dirty && !LabelInspectorModel.shared.isNew { ChangeDot().padding(.trailing, WhooshSpacing.s32 + WhooshSpacing.s4) } }
            .allowsHitTesting(false).transition(.opacity)
            .accessibilityIdentifier("mac-labels.inspector.editring")
    }
}

struct NewLabelRow: View {
    let name: String
    let depth: Int
    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Image(systemName: "tag").font(.system(size: WhooshTypography.body.size)).foregroundStyle(WhooshPalette.accent).frame(width: WhooshMetrics.actionIcon)
            Text(name.isEmpty ? Words.text("New label") : name).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label).lineLimit(1)
            Text(Words.text("New")).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.tertiaryLabel)
            Spacer(minLength: WhooshSpacing.s4)
        }
        .padding(.leading, WhooshSpacing.s8 + CGFloat(depth) * WhooshSpacing.s12 + WhooshSpacing.s16).padding(.trailing, WhooshSpacing.s8)
        .frame(height: WhooshMetrics.sidebarRow)
        .background(RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous).fill(WhooshPalette.hover))
        .overlay { EditRing() }
        .transition(.opacity)
        .accessibilityElement(children: .combine).accessibilityIdentifier("mac-labels.inspector.newrow")
    }
}
