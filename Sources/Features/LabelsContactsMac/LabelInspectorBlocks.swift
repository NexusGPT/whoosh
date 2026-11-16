import SwiftUI

/// The Label card under the three flow cards: name, inside, colour, applies to, pinned as a tab. (The description moved into the AI card.)
struct LabelBlocks: View {
    @Bindable var model: LabelInspectorModel
    var focus: FocusState<LabelFocus?>.Binding
    private var d: LabelDraft { model.draft }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            FormSectionHeader(title: Words.text("Label"))
            GroupedCard {
                row("name", Words.text("Name")) { nameField }
                row("inside", Words.text("Inside")) { insideMenu }
                row("colour", Words.text("Colour")) { ColourSwatches(chosen: d.colour) { c in model.edit { $0.colour = c } } }
                row("scope", Words.text("Applies to")) { scopeMenu }
                row("pinned", Words.text("Pinned as a tab")) {
                    Toggle(Words.text("Pinned as a tab"), isOn: Binding(get: { d.pinned }, set: { v in model.edit { $0.pinned = v } }))
                        .toggleStyle(.switch).labelsHidden().controlSize(.small)
                        .help(Words.text("Pinned as a tab")).accessibilityIdentifier("mac-labels.inspector.pinned")
                }
            }
            .whooshAx(LabelGeo.card("label"))
            if let r = model.refusal, r.field != "rules", !FlowRefusal.codes.contains(r.field) { InlineError(words: r.words) }
        }
        .animation(WhooshMotion.hover, value: model.refusal?.words)
    }

    /// One shared form-grid row of the Label card, measured for the geometry check.
    private func row<C: View>(_ key: String, _ title: String, @ViewBuilder _ control: () -> C) -> some View {
        FormGridRow(title: title, probeId: LabelGeo.fit("label", key)) { control().whooshAx(LabelGeo.ctl("label", key)) }
            .whooshAx(LabelGeo.row("label", key))
    }

    private var nameField: some View {
        TextField(Words.text("Name"), text: Binding(get: { d.name }, set: { v in model.edit { $0.name = v } }))
            .textFieldStyle(.plain).whooshText(WhooshTypography.body).multilineTextAlignment(.leading)
            .padding(.horizontal, WhooshSpacing.s8).padding(.vertical, WhooshSpacing.s4)
            .frame(width: LabelGeo.fieldWidth)
            .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: WhooshRadius.item, style: .continuous))
            .overlay { fieldRing(focus.wrappedValue == .name, bad: model.refusal?.field == "name") }
            .focused(focus, equals: .name)
            .onSubmit { _ = model.isNew ? model.create() : model.apply() }
            .help(Words.text("Name"))
            .accessibilityIdentifier("mac-labels.inspector.name")
    }

    private func fieldRing(_ on: Bool, bad: Bool) -> some View {
        RoundedRectangle(cornerRadius: WhooshRadius.item, style: .continuous)
            .strokeBorder(bad ? WhooshPalette.warning : (on ? WhooshPalette.accent : WhooshPalette.separator), lineWidth: on || bad ? WhooshSpacing.s2 : WhooshMetrics.separator)
            .animation(WhooshMotion.hover, value: on)
    }

    /// Inside: Top level, then every owner label that can hold one more level (not this label, not what sits under it).
    private var insideMenu: some View {
        let all = model.core.labels().filter { $0.kind == .owner }
        let blocked = Set(descendants(of: model.labelId, in: all) + (model.labelId.map { [$0] } ?? []))
        let choices = all.filter { !blocked.contains($0.id) && Int($0.depth) < WhooshConstants.LABEL_DEPTH_MAX }
        return Menu {
            Button(Words.text("Top level")) { model.edit { $0.parentId = nil } }.accessibilityIdentifier("mac-labels.inspector.inside.top")
            ForEach(choices, id: \.id) { l in
                Button(String(repeating: "   ", count: Int(l.depth) - 1) + l.name) { model.edit { $0.parentId = l.id } }
                    .accessibilityIdentifier("mac-labels.inspector.inside.\(l.id)")
            }
        } label: { PopPill { Text(model.parentLabel?.name ?? Words.text("Top level")) } }
        .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden).fixedSize()
        .help(Words.text("Where this label sits. Labels nest up to %lld levels.", WhooshConstants.LABEL_DEPTH_MAX))
        .accessibilityIdentifier("mac-labels.inspector.inside")
    }

    private func descendants(of id: String?, in all: [MailLabel]) -> [String] {
        guard let id else { return [] }
        let kids = all.filter { $0.parentId == id }.map(\.id)
        return kids + kids.flatMap { descendants(of: $0, in: all) }
    }

    private var scopeMenu: some View {
        Menu {
            Button(Words.text("All mailboxes")) { model.edit { $0.scope = .all } }.accessibilityIdentifier("mac-labels.inspector.scope.all")
            Divider()
            ForEach(AccountBook.shared.accounts(core: model.core)) { a in
                Button(a.name) { model.edit { $0.scope = .account(accountId: a.id) } }.accessibilityIdentifier("mac-labels.inspector.scope.\(a.id)")
            }
        } label: { PopPill { Text(model.scopeAccountName ?? Words.text("All mailboxes")) } }
        .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden).fixedSize()
        .help(Words.text("All mailboxes, or one account"))
        .accessibilityIdentifier("mac-labels.inspector.scope")
    }
}
