import SwiftUI

/// The step line and the one grey line of words under it: the Keychain note, the step being asked with "n of 5", the servers that were
/// found with Details, the check with "n of 2", or the account connecting. The slot swaps its words in place, one cross-fade per step.
struct AddAccountStepView: View {
    @Bindable var model: AddAccountModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private static var order: [String] { AddAccountWords.stepOrder }

    var body: some View {
        let W = AddAccountWords.self
        VStack(alignment: .leading, spacing: 0) {
            AddAccountHairlineView(count: segments.count, done: segments.done, partial: segments.partial)
            line.padding(.top, 9).frame(minHeight: 18, alignment: .leading)
                .id(lineKey).transition(.opacity)
            if !noteLines.isEmpty {
                VStack(alignment: .leading, spacing: 0) { ForEach(Array(noteLines.enumerated()), id: \.offset) { _, n in n } }.padding(.top, 8)
            }
        }
        .animation(AddAccountMetrics.motion(AddAccountMetrics.swap, reduceMotion), value: lineKey)
        .accessibilityIdentifier("\(W.surface).step")
    }

    // MARK: what the hairline shows
    private var segments: (count: Int, done: Int, partial: Double) {
        switch model.stage {
        case .detecting: return (Self.order.count, min(model.steps.count, Self.order.count - 1), 0.55)
        case .validating: return (2, 0, 0.6)
        default: return (0, 0, 0)
        }
    }

    /// The running step: the one after those already reported.
    var runningStep: String { Self.order[min(model.steps.count, Self.order.count - 1)] }
    private var lineKey: String { "\(model.stage)-\(model.steps.count)-\(model.hasProblem)-\(model.manualOpen)-\(model.notice)" }

    @ViewBuilder private var line: some View {
        let W = AddAccountWords.self
        switch model.stage {
        case .entry:
            if model.notice.isEmpty { words(icon: "lock", W.keychain) } else { words(icon: "exclamationmark.circle", model.notice, tint: WhooshPalette.warning) }
        case .detecting:
            running(W.stepWords(runningStep), W.stepOf(min(model.steps.count + 1, Self.order.count), Self.order.count))
        case .summary:
            if model.hasProblem { EmptyView() }
            else if model.showsFoundLine { found }
            else { words(icon: "magnifyingglass", model.settingsWords) }
        case .validating:
            running(W.checkingServer, W.stepOf(1, 2))
        case .added:
            // the sync state is read at draw time; the line looks again once a second, as the accounts list does
            TimelineView(.periodic(from: .now, by: 1)) { _ in
                let s = AddAccountRows.addedStep(model.added.last)
                switch s.state {
                case "failed": failed(s)
                case "fetched": words(icon: "checkmark", s.words)
                default: running(s.words, s.count, spin: true)
                }
            }
        }
    }

    /// A failed step: the catalogue sentence for the code; Details opens the engine's words under it (the sign-in refusals do the same).
    private func failed(_ s: AddAccountRows.AddedStep) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            words(icon: "exclamationmark.circle", s.words, tint: WhooshPalette.warning)
            if !s.details.isEmpty {
                Button { _ = model.toggleStepDetails() } label: {
                    Text(model.stepDetailsOpen ? AddAccountWords.detailsHide : AddAccountWords.detailsShow).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.accent)
                }
                .buttonStyle(.plain).padding(.leading, 21).accessibilityIdentifier("\(AddAccountWords.surface).step.details")
                if model.stepDetailsOpen {
                    Text(s.details).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                        .textSelection(.enabled).fixedSize(horizontal: false, vertical: true).padding(.leading, 21)
                        .accessibilityIdentifier("\(AddAccountWords.surface).step.details.text")
                }
            }
        }
    }

    private func words(icon: String, _ text: String, tint: ThemeColor = WhooshPalette.secondaryLabel) -> some View {
        HStack(alignment: .top, spacing: 7) {
            Image(systemName: icon).font(.system(size: 12)).foregroundStyle(tint).frame(width: 14, height: 18)
            Text(text).whooshText(WhooshTypography.subhead).foregroundStyle(tint).fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
        }
    }

    private func running(_ text: String, _ count: String, spin: Bool = true) -> some View {
        HStack(spacing: 7) {
            ProgressView().controlSize(.small).scaleEffect(0.7).frame(width: 14, height: 14)
            Text(text).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label).lineLimit(1)
            Spacer(minLength: 8)
            Text(count).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
        }
    }

    /// The servers on one grey line: mark, incoming, sending, which step found them, and Details.
    private var found: some View {
        HStack(spacing: 7) {
            MarkChip(kind: model.headKind, diameter: AddAccountMetrics.foundChip)
            Text("\(model.draft.imapHost):\(model.draft.imapPort)").foregroundStyle(WhooshPalette.label)
            Text("·")
            Text("\(model.draft.smtpHost):\(model.draft.smtpPort)").foregroundStyle(WhooshPalette.label)
            Text("· \(AddAccountWords.foundBy(model.draft.source))").lineLimit(1)
            Spacer(minLength: 8)
            Button { _ = model.toggleManual() } label: { Text(AddAccountWords.details).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.accent) }
                .buttonStyle(.plain).accessibilityIdentifier("\(AddAccountWords.surface).details")
        }
        .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
    }

    // MARK: the note lines under the words
    private var noteLines: [AnyView] {
        let W = AddAccountWords.self
        switch model.stage {
        case .entry: return []
        case .detecting:
            let misses = model.steps.filter { !$0.found }.map { W.missed($0.id) }
            return misses.isEmpty ? [] : [AnyView(plain(W.nothingIn(misses)))]
        case .summary where model.showsFoundLine: return providerNotes
        case .validating:
            let c = model.draft
            return [AnyView(plain(W.imapSteps(c.imapHost, c.imapPort)))]
        default: return []
        }
    }

    private var providerNotes: [AnyView] {
        let W = AddAccountWords.self
        var out: [AnyView] = []
        if let p = model.provider, !p.steps.isEmpty {
            out.append(AnyView(HStack(spacing: 4) {
                Text(W.appPasswordNote(p.name)).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                Button { _ = model.open(link: p.link) } label: { Text(p.linkWords).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.accent) }
                    .buttonStyle(.plain).accessibilityIdentifier("\(W.surface).app-password.link")
            }))
        }
        if model.oauthKind == .microsoft && !model.oauthSaved(.microsoft) {
            out.append(AnyView(plain(W.microsoftNoPassword).accessibilityIdentifier("\(W.surface).provider-note")))
            out.append(AnyView(guideButton.padding(.top, 2)))
        }
        return out
    }

    /// "How to get your app ID": opens the bundled guide.
    var guideButton: some View {
        Button { _ = model.openGuide() } label: { Text(AddAccountWords.guideLink).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.accent) }
            .buttonStyle(.plain).accessibilityIdentifier("\(AddAccountWords.surface).app-id-guide")
    }

    private func plain(_ t: String) -> some View {
        Text(t).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).fixedSize(horizontal: false, vertical: true)
    }
}

extension AddAccountModel {
    /// The words under the fields while Manual setup stands in for a found line: nothing was found, or nothing was asked yet.
    var settingsWords: String { steps.isEmpty ? AddAccountWords.typedSettingsNote : AddAccountWords.noSettings(domain) }
}
