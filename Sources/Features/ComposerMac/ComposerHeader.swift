import SwiftUI

/// From, To, Cc and Subject: four quiet rows on one hairline grid. The label column is fixed so the fields align.
struct ComposerHeader: View {
    @Bindable var model: ComposerModel
    var field: FocusState<ComposerField?>.Binding
    let ring: Namespace.ID

    var body: some View {
        VStack(spacing: 0) {
            fromRow
            rule
            toRow
            if model.showCc { rule; ccRow }
            if model.showBcc { rule; bccRow }
            rule
            subjectRow
        }
        .whooshAx(ComposerIds.header)
    }

    private var rule: some View { Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator) }

    /// A row of the grid. The focus ring sits behind the row that holds the cursor.
    private func row<C: View>(_ f: ComposerField, _ label: String, @ViewBuilder _ content: () -> C) -> some View {
        HStack(alignment: .center, spacing: WhooshSpacing.s8) {
            Text(label).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                .frame(width: WhooshSpacing.s32 + WhooshSpacing.s16, alignment: .leading)
                .whooshAx("mac-composer.\(f.rawValue).label")      // a leaf text the socket's `labels` reads (clipped-label check, shots-sheet)
            content()
        }
        .frame(minHeight: WhooshMetrics.listRow - WhooshSpacing.s8)
        .background { ComposerRing(model: model, field: f, ns: ring) }
    }

    private var fromRow: some View {
        row(.from, ComposerWords.from) {
            Button { _ = model.fromOpen ? model.closeFromList() : model.focus(.from) } label: {
                HStack(spacing: WhooshSpacing.s8) {
                    if let a = model.from { AccountMark(account: a, size: .row) }
                    Text(model.from?.displayName ?? "").whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label)
                    Text(model.from?.address ?? "").whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                    Image(systemName: "chevron.up.chevron.down").font(.system(size: WhooshTypography.caption.size - WhooshSpacing.s2, weight: .semibold))
                        .foregroundStyle(WhooshPalette.secondaryLabel)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help(ComposerControlKeys.tooltip(ComposerIds.from))
            .whooshAx(ComposerIds.from)
            Spacer(minLength: 0)
            KeyCap("⌘⇧F")
        }
        .overlay(alignment: .topLeading) { if model.fromOpen { fromList.offset(x: WhooshSpacing.s32 + WhooshSpacing.s24, y: WhooshSpacing.s32) } }
        .zIndex(2)
    }

    private var fromList: some View {
        PickerPanel(width: WhooshSpacing.s32 * 10) {
            ScrollView {
                VStack(spacing: WhooshSpacing.s2) {
                    ForEach(Array(model.accounts.enumerated()), id: \.element.id) { i, a in
                        Button { model.fromId = a.id; model.fromOpen = false } label: {
                            HStack(spacing: WhooshSpacing.s8) {
                                AccountMark(account: a, size: .row, chosen: a.id == model.fromId)
                                Text(a.displayName).whooshText(WhooshTypography.body, emphasized: a.id == model.fromId).foregroundStyle(WhooshPalette.label)
                                Spacer(minLength: WhooshSpacing.s8)
                                Text(a.address).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                            }
                            .padding(.horizontal, WhooshSpacing.s8).frame(height: WhooshMetrics.sidebarRow + WhooshSpacing.s4)
                            .background(i == model.listIndex ? AnyShapeStyle(WhooshPalette.mine) : AnyShapeStyle(.clear), in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
                            .contentShape(Rectangle()).hoverAck(radius: WhooshRadius.control)
                        }
                        .buttonStyle(.plain).whooshAx(ComposerIds.fromChoice(i))
                    }
                }
            }.frame(height: min(CGFloat(model.accounts.count) * (WhooshMetrics.sidebarRow + WhooshSpacing.s6), WhooshSpacing.s32 * 8))
        }
        .popoverEntrance()
    }

    private var toRow: some View {
        row(.to, ComposerWords.to) {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: WhooshSpacing.s6) {
                    ForEach(Array(model.to.enumerated()), id: \.element) { i, a in RecipientChip(address: a, name: model.contactName(a), removeAx: ComposerIds.chipRemove("to", i)) { model.removeRecipient(a) } }
                    TextField("", text: $model.toInput)
                        .textFieldStyle(.plain).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label)
                        .frame(minWidth: WhooshSpacing.s32 * 4).focused(field, equals: .to)
                        .onSubmit { _ = model.commitRecipient() }
                        .onChange(of: model.toInput) { _, v in typed(.to, v) }
                        .onKeyPress(.downArrow) { model.moveCompletion(1, .to) ? .handled : .ignored }
                        .onKeyPress(.upArrow) { model.moveCompletion(-1, .to) ? .handled : .ignored }
                        .onKeyPress(.tab) { model.takeCompletion(.to) ? .handled : .ignored }
                        .accessibilityLabel(ComposerWords.to)
                }
            }
            if !model.showCc {
                Button(ComposerWords.cc) { _ = model.focus(.cc) }
                    .buttonStyle(.plain).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    .help(ComposerControlKeys.tooltip(ComposerIds.showCc))
                    .whooshAx(ComposerIds.showCc)
            }
            if !model.showBcc {
                Button(ComposerWords.bcc) { _ = model.focus(.bcc) }
                    .buttonStyle(.plain).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    .help(ComposerControlKeys.tooltip(ComposerIds.showBcc))
                    .whooshAx(ComposerIds.showBcc)
            }
        }
        .overlay(alignment: .topLeading) { offeredList(.to, "to").offset(x: WhooshSpacing.s32 + WhooshSpacing.s24, y: WhooshSpacing.s32) }
        .zIndex(1.3)   // a row's list opens over the rows under it: To over Cc over Bcc over Subject
    }

    /// The words of a recipient field changed: a comma or a semicolon takes the recipient, anything else highlights the list's first line again.
    private func typed(_ f: ComposerModel.RecipientField, _ v: String) {
        if v.hasSuffix(",") || v.hasSuffix(";") { _ = model.commitRecipient(f) } else { model.recipientTyped(f) }
    }

    /// The people offered under To, Cc or Bcc for the words typed there: ↑ ↓ choose, ↩ or ⇥ takes the highlighted one, a click takes its line.
    @ViewBuilder private func offeredList(_ f: ComposerModel.RecipientField, _ name: String) -> some View {
        let list = Array(model.offered(f).prefix(5))
        if !list.isEmpty {
            PickerPanel(width: WhooshSpacing.s32 * 10) {
                ForEach(Array(list.enumerated()), id: \.element.id) { i, c in
                    let on = i == model.completionIndex
                    Button { _ = model.pickCompletion(i, f) } label: {
                        PickerRow(title: c.name.isEmpty ? c.address : c.name, highlighted: on) {
                            Text(c.name.isEmpty ? "" : c.address).whooshText(WhooshTypography.caption).lineLimit(1)
                                .foregroundStyle(on ? WhooshPalette.onAccent : WhooshPalette.secondaryLabel)
                        }
                    }.buttonStyle(.plain).whooshAx(ComposerIds.completionLine(name, i))
                }
            }
            .popoverEntrance()
            .whooshAx(ComposerIds.completionList(name))
        }
    }

    private var ccRow: some View {
        row(.cc, ComposerWords.cc) {
            ForEach(Array(model.cc.enumerated()), id: \.element) { i, a in RecipientChip(address: a, name: model.contactName(a), removeAx: ComposerIds.chipRemove("cc", i)) { model.removeRecipient(a) } }
            TextField("", text: $model.ccInput).textFieldStyle(.plain).whooshText(WhooshTypography.body).focused(field, equals: .cc)
                .onSubmit { _ = model.commitRecipient(.cc) }.accessibilityLabel(ComposerWords.cc)
                .onChange(of: model.ccInput) { _, v in typed(.cc, v) }
                .onKeyPress(.downArrow) { model.moveCompletion(1, .cc) ? .handled : .ignored }
                .onKeyPress(.upArrow) { model.moveCompletion(-1, .cc) ? .handled : .ignored }
                .onKeyPress(.tab) { model.takeCompletion(.cc) ? .handled : .ignored }
                .onAppear { if model.focusRequest?.field == .cc { field.wrappedValue = .cc } }
        }
        .overlay(alignment: .topLeading) { offeredList(.cc, "cc").offset(x: WhooshSpacing.s32 + WhooshSpacing.s24, y: WhooshSpacing.s32) }
        .zIndex(1.2)
        .whooshAx("mac-composer.cc")
    }

    private var bccRow: some View {
        row(.bcc, ComposerWords.bcc) {
            ForEach(Array(model.bcc.enumerated()), id: \.element) { i, a in RecipientChip(address: a, name: model.contactName(a), removeAx: ComposerIds.chipRemove("bcc", i)) { model.removeRecipient(a) } }
            TextField("", text: $model.bccInput).textFieldStyle(.plain).whooshText(WhooshTypography.body).focused(field, equals: .bcc)
                .onSubmit { _ = model.commitRecipient(.bcc) }.accessibilityLabel(ComposerWords.bcc)
                .onChange(of: model.bccInput) { _, v in typed(.bcc, v) }
                .onKeyPress(.downArrow) { model.moveCompletion(1, .bcc) ? .handled : .ignored }
                .onKeyPress(.upArrow) { model.moveCompletion(-1, .bcc) ? .handled : .ignored }
                .onKeyPress(.tab) { model.takeCompletion(.bcc) ? .handled : .ignored }
                .onAppear { if model.focusRequest?.field == .bcc { field.wrappedValue = .bcc } }
        }
        .overlay(alignment: .topLeading) { offeredList(.bcc, "bcc").offset(x: WhooshSpacing.s32 + WhooshSpacing.s24, y: WhooshSpacing.s32) }
        .zIndex(1.1)
        .whooshAx("mac-composer.bcc")
    }

    private var subjectRow: some View {
        row(.subject, ComposerWords.subject) {
            TextField("", text: $model.subject).textFieldStyle(.plain).focused(field, equals: .subject)
                .whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label)
                .accessibilityLabel(ComposerWords.subject)
        }
    }
}
