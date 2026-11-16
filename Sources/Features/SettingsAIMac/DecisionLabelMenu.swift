import SwiftUI

/// Pick ANY node of the label tree; the choice reads as its path ("client › Provanta").
/// The tree is the core's (`core.labels()`, AILabelTree).
struct DecisionLabelMenu: View {
    let path: String
    let pick: (String) -> Void
    var body: some View {
        Menu {
            ForEach(AILabelTree.nodes(), id: \.self) { p in
                Button(String(repeating: "    ", count: AILabelTree.depth(p) - 1) + AILabelTree.leaf(p)) { pick(p) }
                    .accessibilityIdentifier("mac-settings-ai.label-menu.node")
            }
        } label: {
            HStack(spacing: WhooshSpacing.s4) {
                Image(systemName: "tag").font(.system(size: WhooshTypography.caption.size))
                Text(path).fontWeight(.semibold).lineLimit(1).truncationMode(.head)
            }
            .foregroundStyle(WhooshPalette.accent)
        }
        .menuStyle(.borderlessButton).fixedSize()
        .help(AIWords.pickLabel)
    }
}

/// What the page reads to draw the working word: only while the preview runs (a pure function so a test can pin it).
enum PreviewLine {
    static func shows(_ state: AISettingsStore.PreviewState) -> Bool { state == .running }
}

/// The preview on the last DECISION_PREVIEW_N messages: per message, the outcome and the entry that decided it.
struct PreviewList: View {
    let store: AISettingsStore
    let pageId: String
    var body: some View {
        if PreviewLine.shows(store.previewState(pageId)) {
            Text(AIWords.checking).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)      // the pane's own working word, only while the run lasts
        } else if let rows = store.previews[pageId] {
            VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
                HStack {
                    Text([AIWords.previewLead, store.previewNotes[pageId].map { Words.text($0) }].compactMap { $0 }.joined(separator: " ")).whooshText(WhooshTypography.subhead)
                        .foregroundStyle(WhooshPalette.secondaryLabel)
                    Spacer()
                    AIButton(title: store.applied.contains(pageId) ? AIWords.applied : AIWords.apply, primary: !store.applied.contains(pageId),
                             id: "mac-settings-ai.decisions.page.\(pageId).apply") { store.apply(page: pageId) }
                }
                AICard {
                    VStack(spacing: 0) {
                        ForEach(Array(rows.enumerated()), id: \.element.id) { i, r in
                            if i > 0 { Divider().overlay(WhooshPalette.separator) }
                            HStack(spacing: WhooshSpacing.s12) {
                                Text(r.sender).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.label)
                                    .frame(width: WhooshMetrics.blockWidth / 4, alignment: .leading).lineLimit(1)
                                Text(r.subject).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                                Spacer(minLength: WhooshSpacing.s8)
                                Text(r.decidedBy).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.tertiaryLabel).lineLimit(1)
                                AIChip(text: r.outcome, tone: r.differs ? .warn : .neutral)
                            }
                            .padding(.horizontal, WhooshSpacing.s16).padding(.vertical, WhooshSpacing.s6)
                            .help(r.differs ? AIWords.differs : "")
                        }
                    }
                }
                .accessibilityIdentifier("mac-settings-ai.decisions.page.\(pageId).preview")
            }
            .transition(.opacity)
        }
    }
}
