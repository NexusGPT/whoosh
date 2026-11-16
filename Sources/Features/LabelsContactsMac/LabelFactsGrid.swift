import SwiftUI

/// The three states a fact can be in are told by a colour as well as a word: "yes" green, "no" red, "—" (any) the plain label. The drawing's
/// own values (docs/design/boards/labels-flow-r1/base.css `--yes`, `--no`); now Sources/Design tokens (WhooshPalette.factYes, factNo, flowIndigo, flowTeal).
enum FlowTone {
    static let yes = WhooshPalette.factYes
    static let no = WhooshPalette.factNo
    static let indigo = WhooshPalette.flowIndigo
    static let teal = WhooshPalette.flowTeal
}

/// « — / oui / non »: one fact's three states as a segmented switch. The thumb slides on the tab spring (transform only); the change lands
/// on the frame of the click.
struct TriState: View {
    let state: FactState
    let name: String
    let id: String
    let pick: (FactState) -> Void
    @Namespace private var thumb
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        HStack(spacing: 0) { ForEach([FactState.any, .yes, .no], id: \.self) { segment($0) } }
            .background {   // the one thumb, never a source: it takes the chosen state's frame
                RoundedRectangle(cornerRadius: WhooshRadius.keyCap, style: .continuous).fill(WhooshPalette.thumb)
                    .overlay { RoundedRectangle(cornerRadius: WhooshRadius.keyCap, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator) }
                    .matchedGeometryEffect(id: key(state), in: thumb, isSource: false)
            }
            .padding(WhooshSpacing.s2)
            .background(RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous).fill(WhooshPalette.fill))
            .animation(WhooshMotion.animation(WhooshMotion.tabThumb, reduceMotion: reduceMotion), value: state)
            .accessibilityElement(children: .contain)
    }

    private func word(_ s: FactState) -> String {
        switch s { case .any: "—"; case .yes: Words.text("yes"); case .no: Words.text("no") }
    }
    private func key(_ s: FactState) -> String { switch s { case .any: "any"; case .yes: "yes"; case .no: "no" } }
    private func tint(_ s: FactState) -> AnyShapeStyle {
        switch s { case .any: AnyShapeStyle(WhooshPalette.label); case .yes: AnyShapeStyle(FlowTone.yes); case .no: AnyShapeStyle(FlowTone.no) }
    }

    private func segment(_ s: FactState) -> some View {
        let on = state == s
        return Button { pick(s) } label: {
            Text(word(s)).whooshText(WhooshTypography.caption, emphasized: on)
                .foregroundStyle(on ? tint(s) : AnyShapeStyle(WhooshPalette.secondaryLabel))
                .frame(minWidth: WhooshSpacing.s32 - WhooshSpacing.s2, minHeight: WhooshSpacing.s20)
                .matchedGeometryEffect(id: key(s), in: thumb, isSource: true)   // hid-ai round 4: one matched-geometry source per id (the MatchedGeometryScope trap of main88): each item is the source of its own id, one thumb follows the chosen one
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(Words.text("%@: %@", name, word(s)))
        .accessibilityAddTraits(on ? .isSelected : [])
        .accessibilityIdentifier("\(id).\(key(s))")
    }
}

/// One fact of the grid: its name (bold once it is set) and its three states.
struct FactCell: View {
    let name: String
    let state: FactState
    let id: String
    let pick: (FactState) -> Void

    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Text(name).whooshText(WhooshTypography.subhead, emphasized: state != .any)
                .foregroundStyle(state == .any ? WhooshPalette.secondaryLabel : WhooshPalette.label).lineLimit(1)
                .animation(WhooshMotion.hover, value: state)
            Spacer(minLength: WhooshSpacing.s8)
            TriState(state: state, name: name, id: id, pick: pick)
        }
        .frame(height: WhooshSpacing.s24 + WhooshSpacing.s2)
    }
}

/// The eight facts of a chain, two to a row when the card is wide enough for a name beside its switch, one to a row below that.
struct FactsGrid: View {
    @Bindable var model: LabelInspectorModel
    let ref: ChainRef
    let card: String

    private var facts: [FactState] { model.chain(ref)?.facts ?? Array(repeating: .any, count: ChainDraft.order.count) }

    var body: some View {
        ViewThatFits(in: .horizontal) {
            Grid(horizontalSpacing: WhooshSpacing.s24, verticalSpacing: WhooshSpacing.s4) {
                ForEach(0..<ChainDraft.order.count / 2, id: \.self) { r in GridRow { cell(2 * r); cell(2 * r + 1) } }
            }
            VStack(spacing: WhooshSpacing.s4) { ForEach(0..<ChainDraft.order.count, id: \.self) { cell($0) } }
        }
        .padding(.horizontal, WhooshSpacing.s12).padding(.vertical, WhooshSpacing.s6)
        .frame(maxWidth: .infinity, alignment: .leading)
        .whooshAx(LabelGeo.chainRow(card, "facts"))
    }

    private func cell(_ i: Int) -> some View {
        let fact = ChainDraft.order[i], key = messageFactKey(fact: fact)
        return FactCell(name: FlowFacts.title(fact), state: facts[i], id: "\(LabelGeo.base).\(card).fact.\(key)") { model.setFact(ref, i, $0) }
            .whooshAx(LabelGeo.chainCtl(card, "fact.\(key)"))
    }
}
