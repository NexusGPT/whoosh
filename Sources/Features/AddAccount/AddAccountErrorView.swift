import SwiftUI

/// A refusal, quietly: an orange dot, who refused, the server's exact words (selectable), and what to do with the page that fixes it.
struct AddAccountErrorView: View {
    let model: AddAccountModel

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Circle().fill(WhooshPalette.warning).frame(width: WhooshMetrics.warningDot, height: WhooshMetrics.warningDot)
                Text(header).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label)
                Spacer(minLength: 0)
            }
            if model.errorIsServer, model.hasDetails {
                // SPEC A87: the plain sentence, then the server's own words behind Details (closed by default)
                Text(model.errorText).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label)
                    .fixedSize(horizontal: false, vertical: true).accessibilityIdentifier("\(AddAccountWords.surface).error.sentence")
                if !model.consentPaths.isEmpty {
                    // A97 + A98: the approval link for the mailbox's organisation (with Copy), then the two admin paths, before the raw words
                    VStack(alignment: .leading, spacing: 4) {
                        if !model.consentLink.isEmpty { consentLinkRow }
                        ForEach(Array(model.consentPaths.enumerated()), id: \.offset) { i, line in
                            let n = i + (model.consentLink.isEmpty ? 1 : 2)
                            HStack(alignment: .firstTextBaseline, spacing: 6) {
                                Text(verbatim: String(n)).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
                                Text(line).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).fixedSize(horizontal: false, vertical: true)
                            }.accessibilityIdentifier("\(AddAccountWords.surface).error.path.\(n)")
                        }
                    }.padding(.vertical, 2)
                }
                Button { _ = model.toggleDetails() } label: {
                    HStack(spacing: 4) {
                        Image(systemName: model.detailsOpen ? "chevron.down" : "chevron.right").font(.system(size: 9, weight: .semibold))
                        Text(model.detailsOpen ? AddAccountWords.detailsHide : AddAccountWords.detailsShow).whooshText(WhooshTypography.caption)
                    }.foregroundStyle(WhooshPalette.secondaryLabel)
                }
                .buttonStyle(.plain).accessibilityIdentifier("\(AddAccountWords.surface).error.details")
                if model.detailsOpen {
                    Text(model.errorDetails).font(.system(size: 11.5, design: .monospaced)).lineSpacing(3)
                        .foregroundStyle(WhooshPalette.secondaryLabel).textSelection(.enabled).fixedSize(horizontal: false, vertical: true)
                        .accessibilityIdentifier("\(AddAccountWords.surface).error.answer")
                }
            } else if model.errorIsServer {
                Text(model.errorText).font(.system(size: 11.5, design: .monospaced)).lineSpacing(3)
                    .foregroundStyle(WhooshPalette.secondaryLabel).textSelection(.enabled).fixedSize(horizontal: false, vertical: true)
                    .accessibilityIdentifier("\(AddAccountWords.surface).error.answer")
            }
            if !advice.isEmpty || !model.errorLink.isEmpty {
                HStack(alignment: .firstTextBaseline, spacing: 4) {
                    if !advice.isEmpty { Text(advice).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).fixedSize(horizontal: false, vertical: true) }
                    if !model.errorLink.isEmpty {
                        Button { _ = model.open(link: model.errorLink) } label: {
                            Text(model.provider?.linkWords ?? AddAccountWords.serverSettings).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.accent)
                        }
                        .buttonStyle(.plain).accessibilityIdentifier("\(AddAccountWords.surface).error.link")
                    }
                    Spacer(minLength: 0)
                }
            }
        }
        .padding(.horizontal, 14).padding(.top, 11).padding(.bottom, 12).frame(maxWidth: .infinity, alignment: .leading)
        .background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous))
        .accessibilityElement(children: .contain).accessibilityIdentifier("\(AddAccountWords.surface).error")
    }

    /// A98: line 1 of the consent help: the clickable link (opens in the browser) and Copy beside it.
    private var consentLinkRow: some View {
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            Text(verbatim: "1").whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
            Button { _ = model.open(link: model.consentLink) } label: {
                Text(model.consentLinkLine).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.accent)
                    .lineLimit(2).truncationMode(.middle).multilineTextAlignment(.leading)
            }
            .buttonStyle(.plain).accessibilityIdentifier("\(AddAccountWords.surface).error.consent-link")
            Spacer(minLength: 4)
            Button { _ = model.copy(link: model.consentLink) } label: {
                HStack(spacing: 3) {
                    Image(systemName: "doc.on.doc").font(.system(size: 9, weight: .semibold))
                    Text(AddAccountWords.copy).whooshText(WhooshTypography.caption)
                }.foregroundStyle(WhooshPalette.secondaryLabel)
            }
            .buttonStyle(.plain).accessibilityIdentifier("\(AddAccountWords.surface).error.consent-copy")
        }
    }

    private var header: String { model.errorIsServer ? model.errorHeader : model.errorText }

    /// What the person does next: the provider's app-password rule, or the coming-soon sign-in.
    private var advice: String {
        if let p = model.provider, !p.steps.isEmpty, model.errorIsServer, model.errorEngineCode == nil { return AddAccountWords.appPasswordNeeded(p.name) }
        return ""
    }
}

/// "Stop checking? Nothing is saved yet." with its two buttons, below the step line while the Esc question stands.
struct AddAccountStopRow: View {
    let model: AddAccountModel
    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "clock").font(.system(size: 12)).foregroundStyle(WhooshPalette.secondaryLabel)
            Text(AddAccountWords.stopQuestion).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label)
            Spacer(minLength: 8)
            ForEach(AddAccountControls.stopRow()) { c in AddAccountButton(model: model, control: c) }
        }
        .accessibilityElement(children: .contain).accessibilityIdentifier("\(AddAccountWords.surface).stop")
    }
}
