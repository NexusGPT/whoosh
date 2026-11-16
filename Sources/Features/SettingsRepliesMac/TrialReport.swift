import SwiftUI

/// « Essayer sur un message »: what the three steps said on one real message, what the search asked and kept, and the draft it would write.
/// Nothing is stored and nothing is sent (the contract: a trial writes no draft).
struct TrialReportView: View {
    let report: ReplyTryReport?
    private let P = RepliesAx.id("trial")

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: WhooshSpacing.s12) {
                if let r = report {
                    if r.kind == .auto {
                        section(RepliesWords.forWhich) {
                            step(RepliesWords.entryFilter, r.entry, "entry")
                            step(RepliesWords.aiDecision, r.ai, "ai")
                            step(RepliesWords.exitFilter, r.exit, "exit")
                        }
                    }
                    section(RepliesWords.questions) {
                        ForEach(Array(r.retrieval.questions.enumerated()), id: \.offset) { _, q in
                            Text("· " + q).whooshText(WhooshTypography.subhead).fixedSize(horizontal: false, vertical: true)
                        }
                        Text(RepliesWords.foundKept(r.retrieval.found, kept: r.retrieval.kept.count, dropped: r.retrieval.droppedByAiFilter))
                            .whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                    }
                    section(RepliesWords.draft) {
                        Text(r.draftText ?? RepliesWords.noDraft).whooshText(WhooshTypography.subhead)
                            .foregroundStyle(r.draftText == nil ? WhooshPalette.secondaryLabel : WhooshPalette.label)
                            .fixedSize(horizontal: false, vertical: true).textSelection(.enabled)
                            .whooshAx("\(P).draft")
                    }
                    Text(RepliesWords.nothingSent).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                } else {
                    Text(RepliesWords.notReady).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                }
            }
            .padding(WhooshSpacing.s14).frame(maxWidth: .infinity, alignment: .leading)
        }
        .frame(width: 380, height: 420)
        .whooshAx(P)
    }

    private func section<Content: View>(_ title: String, @ViewBuilder _ content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s4) {
            Text(title).whooshText(WhooshTypography.body, emphasized: true)
            content()
        }
    }

    private func step(_ title: String, _ s: ReplyStepReport, _ key: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s8) {
            Image(systemName: RepliesWords.stepSymbol(s.result)).foregroundStyle(s.result == .stopped ? WhooshPalette.warning : WhooshPalette.accent)
            VStack(alignment: .leading, spacing: 0) {
                Text("\(title) : \(RepliesWords.stepResult(s.result))").whooshText(WhooshTypography.subhead)
                if let note = s.modelNote { Text(note).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel) }
            }
        }
        .whooshAx("\(P).\(key)")
    }
}
