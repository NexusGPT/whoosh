import SwiftUI

/// Bring-your-own sign-in, in the sheet under the found line: one field for the organisation's app ID (once), then Sign in with its state.
struct AddAccountOAuthView: View {
    @Bindable var model: AddAccountModel
    @FocusState private var focused: Bool

    var body: some View {
        if let k = model.oauthKind, model.stage == .summary {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Text(AddAccountWords.appIdCap).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
                    Spacer(minLength: 8)
                    if k == .google {
                        Button { _ = model.openGuide() } label: { Text(AddAccountWords.guideLink).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.accent) }
                            .buttonStyle(.plain).accessibilityIdentifier("\(AddAccountWords.surface).app-id-guide")
                    }
                    // SPEC A87: beside the field for every provider: the bundled guide opens in the person's own viewer
                    Button { _ = model.openGuide() } label: { Text(AddAccountWords.guideLinkShort).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.accent) }
                        .buttonStyle(.plain).accessibilityIdentifier("\(AddAccountWords.surface).oauth.guide")
                }
                HStack(spacing: 8) {
                    if model.usesDefaultClient {
                        // SPEC A88: Whoosh's own registration: no ID to paste; the field waits behind the link below
                        Button { _ = model.openOwnId() } label: { Text(AddAccountWords.useOwnId).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.accent) }
                            .buttonStyle(.plain).accessibilityIdentifier("\(AddAccountWords.surface).oauth.own-id")
                        Spacer(minLength: 8)
                    } else if model.oauthSaved(k) { saved } else { field }
                    Button { _ = model.signInTapped() } label: { Text(AddAccountWords.signIn) }
                        .buttonStyle(AddAccountSmallButton()).disabled(!model.canSignIn)
                        .accessibilityIdentifier("\(AddAccountWords.surface).oauth.sign-in")
                }
                if model.oauthPhase != .idle { status }
                else if !model.oauthSaved(k) && !model.usesDefaultClient { Text(AddAccountWords.appIdHint).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel) }
            }
            .padding(.top, 14).transition(.opacity)
            .accessibilityElement(children: .contain).accessibilityIdentifier("\(AddAccountWords.surface).oauth")
        }
    }

    private var field: some View {
        TextField("", text: Binding(get: { model.text(.appId) }, set: { if $0 != model.text(.appId) { model.setFocus(.appId); model.set(.appId, $0) } }))
            .textFieldStyle(.plain).whooshText(WhooshTypography.body).focused($focused)
            .onSubmit { _ = model.primaryAction() }   // Return in the field is the card's Return (see AddAccountFieldsView)
            .padding(.horizontal, 10).frame(height: AddAccountMetrics.smallField)
            .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous)
                .strokeBorder(focused ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(WhooshPalette.sheetFieldBorder), lineWidth: 1))
            .onChange(of: focused) { _, on in if on { model.setFocus(.appId) } }
            .onChange(of: model.focus) { _, f in if f == .appId, !focused { focused = true } }
            .accessibilityIdentifier("\(AddAccountWords.surface).oauth.field")
    }

    private var saved: some View {
        HStack(spacing: 8) {
            Image(systemName: "checkmark.circle.fill").font(.system(size: 12)).foregroundStyle(WhooshPalette.accent)
            Text(AddAccountWords.appIdSaved).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label)
            Spacer(minLength: 8)
            Button { _ = model.changeAppId() } label: { Text(AddAccountWords.change).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.accent) }
                .buttonStyle(.plain).accessibilityIdentifier("\(AddAccountWords.surface).oauth.change")
        }
        .padding(.horizontal, 10).frame(maxWidth: .infinity, alignment: .leading).frame(height: AddAccountMetrics.smallField)
        .background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
    }

    @ViewBuilder private var status: some View {
        HStack(alignment: .top, spacing: 7) {
            switch model.oauthPhase {
            case .waiting: ProgressView().controlSize(.small).scaleEffect(0.7).frame(width: 14, height: 14)
            case .done: Image(systemName: "checkmark.circle.fill").font(.system(size: 12)).foregroundStyle(WhooshPalette.accent)
            default: Image(systemName: "exclamationmark.circle").font(.system(size: 12)).foregroundStyle(WhooshPalette.warning)
            }
            Text(model.oauthPhase.words).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label).fixedSize(horizontal: false, vertical: true)
        }
        .accessibilityIdentifier("\(AddAccountWords.surface).oauth.state")
    }
}

/// The small filled control beside the field.
struct AddAccountSmallButton: ButtonStyle {
    @Environment(\.isEnabled) private var enabled
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.onAccent)
            .padding(.horizontal, 14).frame(height: AddAccountMetrics.smallField)
            .background(WhooshPalette.filledControl, in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
            .opacity(enabled ? (configuration.isPressed ? 0.82 : 1) : 0.4)
    }
}
