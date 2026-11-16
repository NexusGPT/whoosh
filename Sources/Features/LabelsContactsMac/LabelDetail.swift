import SwiftUI

/// The label pane: where the label sits ("Clients > Provanta"), how deep, a rename field, a field that makes a
/// label inside it, and the accounts whose mail carries it. A refused change says why in words, under the field.
struct LabelDetail: View {
    @Bindable var model: LabelsContactsModel

    var body: some View {
        if let id = model.selectedLabelId, let l = model.labels.item(id) {
            let depth = model.labels.depth(id)
            VStack(alignment: .leading, spacing: WhooshSpacing.s20) {
                VStack(alignment: .leading, spacing: WhooshSpacing.s4) {
                    Text(model.labels.pathText(id)).whooshText(WhooshTypography.subhead)
                        .foregroundStyle(WhooshPalette.secondaryLabel)
                        .accessibilityIdentifier("mac-labels-contacts.label.path")
                    Text(l.name).whooshText(WhooshTypography.largeTitle, emphasized: true).foregroundStyle(WhooshPalette.label)
                    Text(LabelsWords.level(depth, WhooshConstants.LABEL_DEPTH_MAX)).whooshText(WhooshTypography.caption)
                        .foregroundStyle(WhooshPalette.tertiaryLabel).contentTransition(.numericText())
                }
                field(LabelsWords.name, text: $model.renameText, prompt: LabelsWords.labelName, ax: "rename") { model.renameSelected(model.renameText) }
                if depth < WhooshConstants.LABEL_DEPTH_MAX {
                    field(LabelsWords.newInside(l.name), text: $model.newLabelName, prompt: LabelsWords.forExample, ax: "create") {
                        model.createLabel(model.newLabelName)
                    }
                } else {
                    Text(LabelsWords.holdsNone(l.name, WhooshConstants.LABEL_DEPTH_MAX))
                        .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                }
                if let words = model.labelRefusal {
                    Label(words, systemImage: "exclamationmark.circle")
                        .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.warning)
                        .transition(.opacity)
                        .accessibilityIdentifier("mac-labels-contacts.labels.refusal")
                }
                carriers(id)
                Spacer(minLength: 0)
                KeyHint("drag", LabelsWords.dropHint)
            }
            .animation(WhooshMotion.hover, value: model.labelRefusal)
            .padding(.horizontal, WhooshMetrics.readerSide).padding(.top, WhooshMetrics.readerTop).padding(.bottom, WhooshMetrics.readerSide)
            .frame(maxWidth: WhooshMetrics.readerMeasure + 2 * WhooshMetrics.readerSide, maxHeight: .infinity, alignment: .topLeading)
            .frame(maxWidth: .infinity, alignment: .leading)
        } else {
            EmptyState(symbol: "tag", title: LabelsWords.pickLabel, line: LabelsWords.nestLine(WhooshConstants.LABEL_DEPTH_MAX))
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private func field(_ title: String, text: Binding<String>, prompt: String, ax: String, submit: @escaping () -> Bool) -> some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            Text(title).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
            TextField(prompt, text: text)
                .textFieldStyle(.plain).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label)
                .onSubmit { _ = submit() }
                .padding(.horizontal, WhooshSpacing.s12).padding(.vertical, WhooshSpacing.s10)
                .background(RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).fill(WhooshPalette.field))
                .overlay(RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).strokeBorder(WhooshPalette.separator))
                .accessibilityIdentifier("mac-labels-contacts.label.\(ax)")
        }
    }

    @ViewBuilder private func carriers(_ id: String) -> some View {
        let list = model.accountsCarrying(id)
        if list.isEmpty {
            Text(LabelsWords.noMail)
                .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
        }
        ForEach(list, id: \.id) { c in
            if let a = model.source.account(c.id) {
                HStack(spacing: WhooshSpacing.s8) {
                    AccountMark(name: a.name, address: a.address, colour: a.colour, icon: a.iconId.map { .library($0) } ?? .providerDefault)
                    Text(a.name).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label)
                    Spacer()
                    Text("\(c.count)").whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                }
            }
        }
    }
}
