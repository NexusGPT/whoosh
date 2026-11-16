import SwiftUI

#if os(iOS)
private extension View {
    func addressField() -> some View { textInputAutocapitalization(.never).keyboardType(.emailAddress).autocorrectionDisabled() }
}
#else
private extension View { func addressField() -> some View { self } }
#endif

/// One row of the header grid: a label, the content, a hairline under it. 44 pt high at least (Apple's target).
struct ComposerPhoneRow<Content: View>: View {
    let label: String
    @ViewBuilder var content: Content

    var body: some View {
        VStack(spacing: 0) {
            HStack(alignment: .center, spacing: WhooshSpacing.s8) {
                Text(label).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.secondaryLabel)
                content
            }
            .padding(.horizontal, PhoneMetrics.gutter)
            .frame(minHeight: PhoneMetrics.roundButton)
            Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator).padding(.leading, PhoneMetrics.gutter)
        }
    }
}

/// To, Cc and Bcc: chips for the addresses already in, then the field. Return, a comma or leaving the field turns the typed address into a chip.
struct ComposerPhoneRecipients: View {
    enum Kind { case to, cc, bcc
        var field: ComposerPhoneState.Field { switch self { case .to: .to; case .cc: .cc; case .bcc: .bcc } }
        var key: String { field.rawValue }
        var label: String { switch self { case .to: ComposerWords.to; case .cc: ComposerWords.cc; case .bcc: ComposerWords.bcc } }
    }
    let kind: Kind
    @Bindable var model: ComposerModel
    var focus: FocusState<ComposerPhoneState.Field?>.Binding

    private var addresses: [String] { switch kind { case .to: model.to; case .cc: model.cc; case .bcc: model.bcc } }
    private var input: Binding<String> {
        switch kind { case .to: $model.toInput; case .cc: $model.ccInput; case .bcc: $model.bccInput }
    }
    private var recipientField: ComposerModel.RecipientField { switch kind { case .to: .to; case .cc: .cc; case .bcc: .bcc } }

    var body: some View {
        ComposerPhoneRow(label: kind.label) {
            HStack(spacing: WhooshSpacing.s6) {
                ForEach(Array(addresses.enumerated()), id: \.element) { i, a in chip(a, index: i) }
                TextField("", text: input)
                    .focused(focus, equals: kind.field)
                    .addressField()
                    .phoneText(PhoneType.body)
                    .submitLabel(.next)
                    .onSubmit { _ = model.commitRecipient(recipientField) }
                    .onChange(of: input.wrappedValue) { _, v in
                        if let c = v.last, c == "," || c == ";" { _ = model.commitRecipient(recipientField) }
                    }
                    .accessibilityLabel(kind.label)
                    .accessibilityIdentifier(kind == .to ? ComposerPhoneIds.to : kind == .cc ? ComposerPhoneIds.cc : ComposerPhoneIds.bcc)
                    .frame(minWidth: PhoneMetrics.roundButton * 2)
            }
        }
    }

    private func chip(_ address: String, index i: Int) -> some View {
        HStack(spacing: WhooshSpacing.s2) {
            Text(address).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.label).lineLimit(1)
            Button { model.removeRecipient(address) } label: {
                Image(systemName: "xmark").font(.system(size: PhoneType.footnote.size, weight: .semibold)).foregroundStyle(WhooshPalette.secondaryLabel)
                    .frame(width: PhoneMetrics.segmentHeight, height: PhoneMetrics.roundButton).contentShape(Rectangle())
            }
            .buttonStyle(PressStyle()).accessibilityLabel(Words.text("Remove %@", address))
            .accessibilityIdentifier(ComposerPhoneIds.chipRemove(kind.key, i))
        }
        .padding(.leading, WhooshSpacing.s10)
        .frame(height: PhoneMetrics.segmentHeight)
        .background(Capsule().fill(PhoneGround.trackFill))
    }
}

/// The From row: the account that sends, with its real mark; a tap opens the list of accounts (a sheet, `ComposerPhoneFromList`).
struct ComposerPhoneFrom: View {
    let state: ComposerPhoneState
    private let m = AppModel.shared

    var body: some View {
        let a = state.current(m)
        ComposerPhoneRow(label: ComposerWords.from) {
            Button { state.fromOpen = true } label: {
                HStack(spacing: WhooshSpacing.s8) {
                    if let a { PhoneTile(account: a, small: true) }
                    Text(a?.address ?? "").phoneText(PhoneType.body).foregroundStyle(WhooshPalette.label).lineLimit(1)
                    Spacer(minLength: 0)
                    Image(systemName: "chevron.up.chevron.down").font(.system(size: PhoneType.footnote.size, weight: .semibold))
                        .foregroundStyle(WhooshPalette.tertiaryLabel)
                }
                .frame(minHeight: PhoneMetrics.roundButton).contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(ComposerWords.from)
            .accessibilityValue(a?.address ?? "")
            .accessibilityIdentifier(ComposerPhoneIds.from)
        }
    }
}

/// The accounts a message can go out from: each with its mark and address; the one that sends carries a check.
struct ComposerPhoneFromList: View {
    let state: ComposerPhoneState
    private let m = AppModel.shared

    var body: some View {
        VStack(spacing: 0) {
            Text("From").phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.label)
                .frame(maxWidth: .infinity).frame(height: PhoneMetrics.groupRow).accessibilityAddTraits(.isHeader)
            ScrollView {
                VStack(spacing: 0) {
                    ForEach(Array(state.accountChoices(m).enumerated()), id: \.element.id) { i, a in
                        Button { state.pickFrom(i, m) } label: {
                            HStack(spacing: WhooshSpacing.s12) {
                                PhoneTile(account: a, small: true)
                                VStack(alignment: .leading, spacing: 0) {
                                    Text(a.name).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.label)
                                    Text(a.needsAttention ? "Sign in again" : a.address).phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                                }
                                Spacer(minLength: 0)
                                if a.id == state.model.fromId { Image(systemName: "checkmark").foregroundStyle(WhooshPalette.accent) }
                            }
                            .padding(.horizontal, PhoneMetrics.gutter).frame(height: PhoneMetrics.accountRow).contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .accessibilityIdentifier(ComposerPhoneIds.fromChoice(i))
                    }
                }
            }
        }
        .background(PhoneGround.list)
    }
}
