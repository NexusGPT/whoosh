import SwiftUI
import UniformTypeIdentifiers

/// One decision page: ONE ordered list (rules, the model entry, the default last); first match wins.
/// Drag an entry to change its priority; the default never moves. Then the preview on the last 20, writing nothing.
struct DecisionPageView: View {
    let store: AISettingsStore
    let pageId: String
    @State private var dragging: UUID?
    var body: some View {
        if let p = store.pageIndex(pageId) {
            let page = store.pages[p]
            VStack(alignment: .leading, spacing: WhooshSpacing.s12) {
                Button { store.section = .decisions } label: {
                    Label(AIWords.navDecisions, systemImage: "chevron.left").whooshText(WhooshTypography.subhead)
                        .foregroundStyle(WhooshPalette.accent)
                }.buttonStyle(.plain).padding(.top, WhooshSpacing.s16).accessibilityIdentifier("mac-settings-ai.decisions.back")
                Text(Words.text(page.title)).font(.system(size: 15, weight: .semibold)).foregroundStyle(WhooshPalette.label).padding(.horizontal, WhooshSpacing.s4)
                AIHeader(title: page.title, lead: AIWords.settles(Words.text(page.settles)))
                if let proposal = store.proposals[pageId] { ProposalBanner(store: store, pageId: pageId, path: proposal) }
                TeachingBanner(store: store, pageId: pageId)
                AICard {
                    VStack(spacing: 0) {
                        ForEach(Array(page.entries.enumerated()), id: \.element.id) { i, e in
                            if i > 0 { Divider().overlay(WhooshPalette.separator) }
                            DecisionEntryRow(store: store, pageId: pageId, entry: e, n: i + 1, last: i == page.entries.count - 1)
                                .opacity(dragging == e.id ? 0.4 : 1)
                                .onDrag { dragging = e.id; return NSItemProvider(object: e.id.uuidString as NSString) }
                                .onDrop(of: [.text], delegate: EntryDrop(store: store, pageId: pageId, target: i, dragging: $dragging))
                        }
                    }
                }
                .animation(WhooshMotion.unfold, value: page.entries)
                .accessibilityIdentifier("mac-settings-ai.decisions.page.\(pageId).entries")
                HStack(spacing: WhooshSpacing.s8) {
                    AIButton(title: AIWords.addRule, id: "mac-settings-ai.decisions.page.\(pageId).add-rule") { store.addRule(page: pageId) }
                    if !page.entries.contains(where: { $0.kind == .model }) {
                        AIButton(title: AIWords.addModel, id: "mac-settings-ai.decisions.page.\(pageId).add-model") { store.addModel(page: pageId) }
                    }
                    Spacer()
                    AIButton(title: AIWords.preview, primary: true, id: "mac-settings-ai.decisions.page.\(pageId).preview") { store.runPreview(page: pageId) }
                }
                PreviewList(store: store, pageId: pageId)
            }
        }
    }
}

private struct EntryDrop: DropDelegate {
    let store: AISettingsStore
    let pageId: String
    let target: Int
    @Binding var dragging: UUID?
    func dropEntered(info: DropInfo) {
        guard let id = dragging, let p = store.pageIndex(pageId),
              let from = store.pages[p].entries.firstIndex(where: { $0.id == id }), from != target else { return }
        store.move(page: pageId, from: from, to: target)
    }
    func dropUpdated(info: DropInfo) -> DropProposal? { DropProposal(operation: .move) }
    func performDrop(info: DropInfo) -> Bool { dragging = nil; return true }
}

/// Teaching on the page (DECISION-MODEL.md v2 §3): the corrections since a day, then the two fixes in order, "Make a rule from these" when they share a
/// sender, domain or list, "Refit the model" for Laya. The proposal banner's own look (one tinted card, its buttons); nothing shows while there is nothing to teach.
private struct TeachingBanner: View {
    let store: AISettingsStore
    let pageId: String
    var body: some View {
        let t = store.teaching(pageId)
        if t.corrections > 0 || !t.offers.isEmpty {
          VStack(alignment: .leading, spacing: WhooshSpacing.s4) {
            HStack(spacing: WhooshSpacing.s12) {
                Image(systemName: "sparkles").foregroundStyle(WhooshPalette.accent)
                if t.corrections > 0 { Text(AIWords.corrections(t.corrections)).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label) }
                Spacer()
                if t.offers.contains("make-rule") {
                    AIButton(title: AIWords.makeRuleFromThese, primary: true, id: "mac-settings-ai.decisions.page.\(pageId).teaching.make-rule") { _ = store.ruleFromCorrections(page: pageId) }
                }
                if t.offers.contains("refit") {
                    AIButton(title: AIWords.refitTheModel, id: "mac-settings-ai.decisions.page.\(pageId).teaching.refit") { _ = store.refit(page: pageId) }
                }
            }
            // the engine's answer to "Refit the model", in the proposal banner's caption style (no refit is claimed: the engine refuses until one exists)
            if let note = store.refitNotes[pageId] {
                Text(note).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                    .accessibilityIdentifier("mac-settings-ai.decisions.page.\(pageId).teaching.note")
            }
          }
            .padding(WhooshSpacing.s12)
            .background(RoundedRectangle(cornerRadius: SettingsMetrics.cardRadius, style: .continuous).fill(WhooshPalette.mine))
            .accessibilityIdentifier("mac-settings-ai.decisions.page.\(pageId).teaching")
        }
    }
}

private struct ProposalBanner: View {
    let store: AISettingsStore
    let pageId: String
    let path: String
    var body: some View {
        HStack(spacing: WhooshSpacing.s12) {
            Image(systemName: "sparkles").foregroundStyle(WhooshPalette.accent)
            VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                Text(AIWords.proposedChild).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                Text(path).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label)
            }
            Spacer()
            AIButton(title: AIWords.dismiss, id: "mac-settings-ai.decisions.page.\(pageId).proposal.dismiss") { store.proposals[pageId] = nil }
            AIButton(title: AIWords.accept, primary: true, id: "mac-settings-ai.decisions.page.\(pageId).proposal.accept") { store.acceptProposal(page: pageId) }
        }
        .padding(WhooshSpacing.s12)
        .background(RoundedRectangle(cornerRadius: SettingsMetrics.cardRadius, style: .continuous).fill(WhooshPalette.mine))
        .accessibilityIdentifier("mac-settings-ai.decisions.page.\(pageId).proposal")
        .transition(.opacity.combined(with: .scale(scale: WhooshConstants.POPOVER_SCALE_FROM, anchor: .top)))
    }
}
