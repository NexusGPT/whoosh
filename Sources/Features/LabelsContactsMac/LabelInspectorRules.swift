import SwiftUI

/// One rule row of a chain: kind pop-up, the values (chips, then the field still being typed), its count, Remove (always drawn).
struct RuleRowView: View {
    @Bindable var model: LabelInspectorModel
    let ref: ChainRef
    let card: String
    let rule: RuleDraft
    let index: Int
    var focus: FocusState<LabelFocus?>.Binding
    @State private var hover = false  // local-ui: Remove shows on hover
    /// the count beside a row is the live proof of the first Entry chain (the one the list shows); the others carry none
    private var count: Int? { ref == .entry && index < model.proof.perRule.count ? model.proof.perRule[index] : nil }
    private func geo(_ part: String) -> String { LabelGeo.chainCtl(card, "r\(index).\(part)") }
    /// the first Entry chain keeps the ids of the round 2 inspector (`rule.0.value`); the others are named by their card
    private var ids: String { card == "entry" ? "\(LabelGeo.base).rule.\(index)" : "\(LabelGeo.base).\(card).rule.\(index)" }
    private var error: String? { ref == .entry ? model.refusal.flatMap { $0.field == "rules" ? $0.words : nil } : nil }
    private var badRow: Bool { error != nil && (rule.kind == .account || index == 0) }

    var body: some View {
        VStack(spacing: 0) {
            HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s8) {
                kindMenu.whooshAx(geo("kind"))
                opMenu.whooshAx(geo("op"))
                values.frame(maxWidth: .infinity, alignment: .leading).whooshAx(geo("values"))
                countText.whooshAx(geo("count"))
                removeButton.whooshAx(geo("remove"))
            }
            .padding(.horizontal, WhooshSpacing.s14).padding(.vertical, WhooshSpacing.s8).frame(minHeight: WhooshMetrics.listRow)
            if badRow, let error { InlineError(words: error).padding(.horizontal, WhooshSpacing.s14).padding(.bottom, WhooshSpacing.s8) }
            else if rule.hasBlankValue { InlineError(words: Words.text("This rule has no value, so it matches nothing.")).padding(.horizontal, WhooshSpacing.s14).padding(.bottom, WhooshSpacing.s8).accessibilityIdentifier("\(ids).incomplete") }
        }
        .onHover { hover = $0 }
        .accessibilityElement(children: .contain).accessibilityIdentifier(ids)
    }

    /// The FIELD menu: Domain, From, Subject... Changing it keeps the typed values where they still fit (text stays text); an account's values are ids, so crossing
    /// to or from Account starts the values over (`RuleKind.keepsValues(to:)`).
    private var kindMenu: some View {
        Menu {
            ForEach(RuleKind.allCases) { k in
                Button(k.fieldTitle) { model.editRule(ref, rule.id) { $0.setField(k) } }
                    .accessibilityIdentifier("\(ids).kind.\(k.rawValue)")
            }
        } label: { PopPill { Text(rule.kind.fieldTitle) } }
        .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden).fixedSize()
        .help(Words.text("What this rule looks at"))
        .accessibilityIdentifier("\(ids).kind")
    }

    /// The CONDITION menu: the forms this field offers (see `RuleKind.operators`).
    private var opMenu: some View {
        Menu {
            ForEach(rule.kind.operators) { o in
                Button(o.title) { model.editRule(ref, rule.id) { $0.op = o } }
                    .accessibilityIdentifier("\(ids).op.\(o.rawValue)")
            }
        } label: { PopPill { Text(rule.op.title) } }
        .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden).fixedSize()
        .help(Words.text("How the field is compared"))
        .accessibilityIdentifier("\(ids).op")
    }

    @ViewBuilder private var values: some View {
        FlowLayout(spacing: WhooshSpacing.s4) {
            let shown = rule.expanded || rule.values.count <= 3 ? rule.values : Array(rule.values.prefix(3))
            ForEach(shown, id: \.self) { v in chip(v) }
            if shown.count < rule.values.count {
                Button { model.editRule(ref, rule.id) { $0.expanded = true } } label: {
                    Text(Words.text("+%lld more", rule.values.count - shown.count)).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                        .padding(.horizontal, WhooshSpacing.s6).frame(height: WhooshSpacing.s20).background(WhooshPalette.fill, in: Capsule())
                }.buttonStyle(.plain).help(Words.text("Show every value"))
            }
            if rule.kind == .account { accountAdder } else if rule.values.isEmpty || focus.wrappedValue == .rule(rule.id) || !rule.pending.isEmpty { field } else { addValue }
        }
        .padding(.horizontal, WhooshSpacing.s4).padding(.vertical, WhooshSpacing.s2)
        .background(rule.kind == .account ? AnyShapeStyle(.clear) : AnyShapeStyle(WhooshPalette.field), in: RoundedRectangle(cornerRadius: WhooshRadius.item, style: .continuous))
        .overlay { if rule.kind != .account { ring } }
    }

    private var ring: some View {
        let on = focus.wrappedValue == .rule(rule.id)
        return RoundedRectangle(cornerRadius: WhooshRadius.item, style: .continuous)
            .strokeBorder(badRow ? WhooshPalette.warning : (on ? WhooshPalette.accent : WhooshPalette.separator), lineWidth: on || badRow ? WhooshSpacing.s2 : WhooshMetrics.separator)
            .animation(WhooshMotion.hover, value: on)
    }

    private var field: some View {
        TextField(rule.values.isEmpty ? rule.kind.placeholder : "", text: Binding(get: { rule.pending }, set: { v in model.editRule(ref, rule.id) { $0.pending = v } }))
            .textFieldStyle(.plain).whooshText(WhooshTypography.subhead).frame(minWidth: WhooshSpacing.s32 + WhooshSpacing.s24)
            .focused(focus, equals: .rule(rule.id))
            .onSubmit {
                if rule.pending.trimmingCharacters(in: .whitespaces).isEmpty { _ = model.isNew ? model.create() : model.apply() }
                else { model.commitPending(ref, rule.id) }
            }
            .accessibilityIdentifier("\(ids).value")
    }

    /// A committed rule shows a quiet "+" that gives the field back for one more value.
    private var addValue: some View {
        Button { focus.wrappedValue = .rule(rule.id) } label: {
            Image(systemName: "plus").font(WhooshTypography.caption.font).imageScale(.small).foregroundStyle(WhooshPalette.accent).frame(width: WhooshSpacing.s20, height: WhooshSpacing.s20)
        }.buttonStyle(.plain).help(Words.text("Add another value")).accessibilityIdentifier("\(ids).addvalue")
    }

    private var accountAdder: some View {
        Menu {
            ForEach(AccountBook.shared.accounts(core: model.core).filter { !rule.values.contains($0.id) }) { a in
                Button(a.name) { model.editRule(ref, rule.id) { $0.values.append(a.id) } }.accessibilityIdentifier("\(ids).account.\(a.id)")
            }
        } label: { Text(rule.values.isEmpty ? Words.text("Pick an account") : "+").whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.accent).padding(.horizontal, WhooshSpacing.s6) }
            .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden).fixedSize().help(Words.text("Add an account"))
            .accessibilityIdentifier("\(ids).account")
    }

    private func chip(_ v: String) -> some View {
        HStack(spacing: WhooshSpacing.s4) {
            Text(rule.kind == .account ? (AccountBook.shared.account(v)?.name ?? v) : v).whooshText(WhooshTypography.caption).lineLimit(1)
            Button { model.editRule(ref, rule.id) { $0.values.removeAll { $0 == v } } } label: {
                Image(systemName: "xmark").font(WhooshTypography.caption.font).imageScale(.small).foregroundStyle(WhooshPalette.tertiaryLabel)
            }.buttonStyle(.plain).help(Words.text("Remove %@", v)).foregroundStyle(hover ? WhooshPalette.secondaryLabel : WhooshPalette.tertiaryLabel)
        }
        .foregroundStyle(WhooshPalette.label).padding(.horizontal, WhooshSpacing.s6).frame(height: WhooshSpacing.s20)
        .background(WhooshPalette.fill, in: Capsule())
    }

    private var countText: some View {
        Group {
            if model.counting && !rule.isEmpty { Text("…") } else if let c = count, !rule.isEmpty { Text("\(c)").contentTransition(.numericText(value: Double(c))) } else { Text("") }
        }
        .whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.tertiaryLabel).frame(width: WhooshSpacing.s20, alignment: .trailing)
        .animation(.snappy, value: count).help(Words.text("Messages this rule matches"))
    }

    private var removeButton: some View {
        Button { model.removeRule(ref, rule.id) } label: {
            Image(systemName: "minus").font(WhooshTypography.caption.font).imageScale(.small).foregroundStyle(WhooshPalette.secondaryLabel)
                .frame(width: WhooshSpacing.s16, height: WhooshSpacing.s16).overlay { Circle().strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator) }
                .contentShape(Circle())   // the whole circle takes the click: a plain button hits only what it draws, here a hairline glyph and a stroke
        }
        .buttonStyle(.plain).help(Words.text("Remove this rule"))   // drawn at rest on every row (it was opacity 0 until hover or focus: the owner could not find it, 2026-11-12)
        .accessibilityIdentifier("\(ids).remove")
    }
}
