import SwiftUI

/// A hairline between the rows of a card, inset like the shared card's.
struct Hair: View {
    var body: some View {
        Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator).padding(.horizontal, WhooshSpacing.s12)
    }
}

/// One chain of a card, top to bottom: "File when [All of these] are true" (and « Autre règle » on the first Entry chain), the eight facts
/// as a grid, the text rows under the grid (sender, subject, domain... each with its own picker, value and remove), and Add rule.
/// `card` names it for the geometry check and the socket: "entry", "other1", "other2"..., "exit".
struct ChainView: View {
    @Bindable var model: LabelInspectorModel
    let ref: ChainRef
    let card: String
    let lead: String
    var focus: FocusState<LabelFocus?>.Binding
    var offersOther = false
    var removable = false

    private var chain: ChainDraft { model.chain(ref) ?? ChainDraft(rows: []) }
    private var ids: String { card == "entry" ? LabelGeo.base : "\(LabelGeo.base).\(card)" }

    var body: some View {
        VStack(spacing: 0) {
            head.whooshAx(LabelGeo.chainRow(card, "head"))
            Hair()
            FactsGrid(model: model, ref: ref, card: card)
            Hair()
            ForEach(Array(chain.rows.enumerated()), id: \.element.id) { i, rule in
                // A new row fades in; a removed row leaves AT ONCE. While a removed row faded out it still took clicks, so a second click on the same spot (the row
                // that moved up under the pointer) fell on the leaving row and removed nothing (owner 13 Nov: "still cannot delete rules").
                RuleRowView(model: model, ref: ref, card: card, rule: rule, index: i, focus: focus).transition(.asymmetric(insertion: .opacity, removal: .identity))
                Hair()
            }
            addRow
        }
        .animation(WhooshMotion.hover, value: chain.rows.map(\.id))
    }

    private var head: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: WhooshSpacing.s8) { phrase; Spacer(minLength: WhooshSpacing.s8); trailing }
            VStack(alignment: .leading, spacing: WhooshSpacing.s4) { phrase; trailing }
        }
        .padding(.horizontal, WhooshSpacing.s12).padding(.vertical, WhooshSpacing.s6)
        .frame(maxWidth: .infinity, minHeight: WhooshMetrics.listRow, alignment: .leading)
    }

    private var phrase: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Text(lead).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
            Menu {
                Button(Words.text("All of these")) { model.setMode(ref, .all) }.accessibilityIdentifier("\(ids).match.all")
                Button(Words.text("Any of these")) { model.setMode(ref, .any) }.accessibilityIdentifier("\(ids).match.any")
            } label: { PopPill { Text(chain.matchMode == .all ? Words.text("All of these") : Words.text("Any of these")) } }
                .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden).fixedSize()
                .help(Words.text("All of these conditions, or any one of them"))
                .accessibilityIdentifier("\(ids).match").whooshAx(LabelGeo.chainCtl(card, "match"))
            Text(chain.matchMode == .all ? Words.text("are true") : Words.text("is true"))
                .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
        }
    }

    @ViewBuilder private var trailing: some View {
        if offersOther {
            Button { _ = model.addOtherChain() } label: {
                HStack(spacing: WhooshSpacing.s6) {
                    KeyCap("A")
                    Text(Words.text("Other rule")).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.accent)
                }.contentShape(Rectangle())
            }
            .buttonStyle(.plain).help(Words.text("A second set of conditions: a message that fits either one is caught"))
            .accessibilityIdentifier("\(LabelGeo.base).other.add").whooshAx(LabelGeo.chainCtl(card, "other"))
        }
        if removable { removeButton }
    }

    private var removeButton: some View {
        Button { if case .other(let id) = ref { model.removeChain(id) } } label: {
            Image(systemName: "xmark").font(WhooshTypography.caption.font).imageScale(.small).foregroundStyle(WhooshPalette.secondaryLabel)
                .frame(width: WhooshSpacing.s20, height: WhooshSpacing.s20).overlay { Circle().strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator) }
                .contentShape(Circle())
        }
        .buttonStyle(.plain).help(Words.text("Remove this other rule"))
        .accessibilityIdentifier("\(ids).remove").whooshAx(LabelGeo.chainCtl(card, "remove"))
    }

    /// The add control keeps the card's inset: the padding sits outside the button, so its frame (and the click) lies inside the inset
    /// like every other control of the card, while the row keeps the 36 pt height of v4.
    private var addRow: some View {
        Button { model.addRule(ref) } label: {
            HStack(spacing: WhooshSpacing.s6) {
                Image(systemName: "plus").font(WhooshTypography.caption.font).imageScale(.small)
                Text(Words.text("Add rule")).whooshText(WhooshTypography.subhead)
                Spacer(minLength: 0)
            }
            .foregroundStyle(WhooshPalette.accent)
            .frame(minHeight: WhooshMetrics.listRow).contentShape(Rectangle())
        }
        .buttonStyle(.plain).help(Words.text("Add rule"))
        .accessibilityIdentifier("\(ids).rule.add").whooshAx(LabelGeo.chainCtl(card, "add"))
        .padding(.horizontal, WhooshSpacing.s12)
    }
}

/// « ou »: the seam between two chains of one card, which are alternatives.
struct OrSeam: View {
    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator)
            Text(Words.text("or")).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
            Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator)
        }
        .padding(.horizontal, WhooshSpacing.s12).padding(.vertical, WhooshSpacing.s4)
        .accessibilityHidden(true)
    }
}
