import SwiftUI

/// The head: the glass chip (grey envelope until the provider is known, then its mark), the title and one grey line that says where things stand.
struct AddAccountHeadView: View {
    let model: AddAccountModel
    /// The mark's surface id, given by whoever draws the head ("add-account" in the sheet, "welcome" in the inline card); never read from the shared model's style, which flips to .inline while the sheet fades out.
    let surface: String
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let added = model.stage == .added
        HStack(spacing: WhooshSpacing.s12) {
            let kind = added ? model.added.last?.kind : model.headKind
            if kind == nil || kind == .unknown {
                BrandMark(size: added ? AddAccountMetrics.addedChip : AddAccountMetrics.headChip, surface: surface)   // A86: the app's own mark in liquid glass when no provider is known
            } else {
                MarkChip(kind: kind, diameter: added ? AddAccountMetrics.addedChip : AddAccountMetrics.headChip)
            }
            VStack(alignment: .leading, spacing: 0) {
                Text(title).whooshText(WhooshTypography.title3).foregroundStyle(WhooshPalette.label)
                Text(lead).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    .id(lead).transition(.opacity)
            }
            Spacer(minLength: 0)
        }
        .animation(AddAccountMetrics.motion(AddAccountMetrics.swap, reduceMotion), value: lead)
        .animation(AddAccountMetrics.motion(AddAccountMetrics.swap, reduceMotion), value: model.headKind)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("\(AddAccountWords.surface).head")
    }

    var title: String {
        if model.stage == .added { return AddAccountWords.added }
        return model.style == .inline ? AddAccountWords.welcomeTitle : AddAccountWords.title
    }

    /// The one line under the title, by where the flow stands.
    var lead: String {
        let W = AddAccountWords.self
        switch model.stage {
        case .entry, .detecting: return model.style == .inline ? W.welcomeLead : W.leadEntry
        case .summary:
            if model.hasProblem { return model.errorIsServer ? W.leadRefused(model.headKind?.name ?? W.leadServer) : W.leadFound }
            return model.mode == .manual ? W.leadManual : W.leadFound
        case .validating: return W.leadChecking(model.headKind?.name ?? W.leadServer)
        case .added: return "\(model.added.last?.address ?? "") · \(model.added.last?.kind.name ?? "")"
        }
    }
}

extension AddAccountModel {
    /// The refusal came from a server (it names a host); a missing password or setting is found here, before anything is sent.
    var errorIsServer: Bool { !errorHost.isEmpty || errorEngineCode != nil }
    /// SPEC A87: a server refusal with a known cause, whose raw words wait behind Details
    var hasDetails: Bool { errorIsServer && (errorCause != nil || errorEngineCode != nil) && !errorDetails.isEmpty }
}
