import SwiftUI

/// One of a label's three cards, read only: its name, what it does, its count and whether it is on, then its rules as lines.
/// The card never edits; the page says where the rules are edited. An off card keeps its words and fades, as on the Mac.
struct LabelCardPhone: View {
    let card: PhoneCardRead
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            head
            ForEach(card.chains) { c in
                PhoneHair()
                chain(c)
            }
            ForEach(Array(card.lines.enumerated()), id: \.offset) { _, line in
                PhoneHair()
                Text(line).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.label)
                    .padding(.horizontal, PhoneMetrics.gutter).padding(.vertical, WhooshSpacing.s10)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            if let note = card.note {
                PhoneHair()
                Text(note).phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.warning)
                    .padding(.horizontal, PhoneMetrics.gutter).padding(.vertical, WhooshSpacing.s10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .accessibilityIdentifier("iphone-labels-contacts.label.card.\(card.key).note")
            }
        }
        .opacity(card.on ? 1 : 0.4)
        .animation(reduceMotion ? nil : WhooshMotion.hover, value: card.on)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("iphone-labels-contacts.label.card.\(card.key)")
    }

    private var head: some View {
        HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s8) {
            VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                Text(card.title).phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.label)
                Text(card.subtitle).phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
            }
            Spacer(minLength: WhooshSpacing.s8)
            VStack(alignment: .trailing, spacing: WhooshSpacing.s2) {
                Text(card.count).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel).contentTransition(.numericText())
                HStack(spacing: WhooshSpacing.s4) {
                    Circle().fill(card.on ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(WhooshPalette.tertiaryLabel))
                        .frame(width: WhooshSpacing.s6, height: WhooshSpacing.s6).accessibilityHidden(true)
                    Text(Words.text(card.on ? "On" : "Off")).phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                }
            }
        }
        .padding(.horizontal, PhoneMetrics.gutter).padding(.vertical, WhooshSpacing.s12)
        .frame(minHeight: PhoneMetrics.roundButton)
    }

    private func chain(_ c: PhoneChainRead) -> some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s4) {
            HStack(spacing: WhooshSpacing.s6) {
                Text(c.lead).phoneText(PhoneType.footnote, emphasized: true)
                if let mode = c.mode { Text("· \(mode)").phoneText(PhoneType.footnote) }
            }
            .foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
            ForEach(Array(c.lines.enumerated()), id: \.offset) { _, line in
                Text(line).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.label).fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(.horizontal, PhoneMetrics.gutter).padding(.vertical, WhooshSpacing.s10)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
