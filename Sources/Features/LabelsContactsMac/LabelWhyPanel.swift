import SwiftUI

/// « Pourquoi ici ? »: a small card over the foot that says, for the message in focus in pane 2, which card of this label placed it and why:
/// "Entry rules · chain 1 · Attachment: yes". It reads the core on open and when the focus moves; it never writes.
struct LabelWhyPanel: View {
    @Bindable var model: LabelInspectorModel

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
            ForEach(Array(model.whyLines.enumerated()), id: \.offset) { i, line in
                Text(line).whooshText(i == 0 ? WhooshTypography.caption : WhooshTypography.subhead, emphasized: i > 0)
                    .foregroundStyle(i == 0 ? WhooshPalette.secondaryLabel : WhooshPalette.label).lineLimit(2).fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(.horizontal, WhooshSpacing.s12).padding(.vertical, WhooshSpacing.s8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).fill(WhooshPalette.panel))
        .overlay { RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator) }
        .accessibilityElement(children: .combine).accessibilityIdentifier("mac-labels.inspector.whypanel")
        .whooshAx(LabelGeo.free("whypanel"))
    }
}
