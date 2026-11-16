import SwiftUI

/// Manual setup, unfolded below the step line only when needed: incoming and sending side by side, then the username and the sign-in route.
struct AddAccountManualView: View {
    @Bindable var model: AddAccountModel
    let narrow: Bool
    @FocusState private var focus: AddAccountModel.Field?
    @State private var driven = false

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top, spacing: 16) {
                column(AddAccountWords.incomingCap, host: "imapHost", port: "imapPort", security: "imapSecurity", placeholder: "imap.\(model.domain)")
                column(AddAccountWords.outgoingCap, host: "smtpHost", port: "smtpPort", security: "smtpSecurity", placeholder: "smtp.\(model.domain)")
            }
            HStack(alignment: .top, spacing: 16) {
                labelled(AddAccountWords.username) { field("username", placeholder: model.address) }
                labelled(AddAccountWords.signInWithCap) { staticField(AddAccountWords.auth("password")) }.frame(width: narrow ? 132 : 150)
            }
        }
        .padding(.top, 14).overlay(alignment: .top) { Rectangle().fill(WhooshPalette.separator).frame(height: 1) }
        .padding(.top, 14)
        .onChange(of: model.focus) { _, f in if let f, f != .address, f != .password, f != focus { driven = true; focus = f } }
        .accessibilityElement(children: .contain).accessibilityIdentifier("\(AddAccountWords.surface).manual-setup")
    }

    private func column(_ cap: String, host: String, port: String, security: String, placeholder: String) -> some View {
        labelled(cap) {
            field(host, placeholder: placeholder)
            HStack(spacing: 6) {
                field(port, placeholder: "", width: narrow ? AddAccountMetrics.portWidthNarrow : AddAccountMetrics.portWidth)
                HStack(spacing: 2) { ForEach(["ssl", "starttls"], id: \.self) { seg(security, $0) } }
                    .padding(2).frame(height: AddAccountMetrics.smallField).frame(maxWidth: .infinity)
                    .background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
            }
        }
    }

    private func labelled<C: View>(_ cap: String, @ViewBuilder _ content: () -> C) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(cap).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
            content()
        }.frame(maxWidth: .infinity, alignment: .leading)
    }

    private func field(_ id: String, placeholder: String, width: CGFloat? = nil) -> some View {
        let f = AddAccountModel.Field(id)
        return TextField(placeholder, text: Binding(get: { model.text(f) }, set: { if $0 != model.text(f) { model.setFocus(f); model.set(f, $0) } }))
            .textFieldStyle(.plain).whooshText(WhooshTypography.body).focused($focus, equals: f)
            .onSubmit { _ = model.primaryAction() }   // Return in a field is the card's Return (see AddAccountFieldsView)
            .padding(.horizontal, 10).frame(width: width, height: AddAccountMetrics.smallField)
            .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous)
                .strokeBorder(focus == f ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(WhooshPalette.sheetFieldBorder), lineWidth: 1))
            .accessibilityIdentifier("\(AddAccountWords.surface).summary.\(id)")
    }

    private func staticField(_ text: String) -> some View {
        Text(text).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label).padding(.horizontal, 10)
            .frame(maxWidth: .infinity, alignment: .leading).frame(height: AddAccountMetrics.smallField)
            .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous).strokeBorder(WhooshPalette.sheetFieldBorder, lineWidth: 1))
            .accessibilityIdentifier("\(AddAccountWords.surface).summary.authMethod")
    }

    private func seg(_ field: String, _ value: String) -> some View {
        let on = model.draft[field] == value
        return Button { model.edit(field, value) } label: {
            Text(AddAccountWords.security(value)).whooshText(WhooshTypography.subhead, emphasized: on)
                .foregroundStyle(on ? WhooshPalette.label : WhooshPalette.secondaryLabel).frame(maxWidth: .infinity, maxHeight: .infinity)
                .background { if on { RoundedRectangle(cornerRadius: WhooshRadius.item, style: .continuous).fill(WhooshPalette.thumb) } }
        }
        .buttonStyle(.plain).accessibilityIdentifier("\(AddAccountWords.surface).summary.\(field).\(value)")
    }
}
