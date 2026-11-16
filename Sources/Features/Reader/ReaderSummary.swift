import SwiftUI

/// `.sm` the summary callout at the top of a thread: the summary the core keeps for the thread (`thread/<id>/summary`),
/// made by the owner's writing model in the background. The read is one indexed read; it never calls a model.
enum ReaderSummary {
    @MainActor static func text(_ m: AppModel, threadId: String) -> String? {
        guard let json = m.core.state(path: "thread/\(threadId)/summary"), let data = json.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(String.self, from: data)
    }
}

/// The summaries the reader shows, kept off the view's body. The core writes a summary in the background and sends no event,
/// so an opened thread is read at once, then again after each wait until a summary is kept (three more reads at most).
@MainActor @Observable
final class ReaderSummaryStore {
    static let shared = ReaderSummaryStore()
    private(set) var texts: [String: String] = [:]
    private static let waitsMs: [UInt64] = [0, 2_000, 5_000, 10_000]

    func load(_ m: AppModel, threadId: String) async {
        for ms in Self.waitsMs {
            if ms > 0 { try? await Task.sleep(nanoseconds: ms * 1_000_000) }
            if Task.isCancelled { return }
            let t0 = InteractionProbe.phaseStart()
            let read = ReaderSummary.text(m, threadId: threadId)
            InteractionProbe.phaseEnd(.readerSummary, t0)   // nonzero only when the read lands between a socket key and its frame
            if let t = read {
                if texts[threadId] != t { texts[threadId] = t }
                return
            }
        }
    }
}

struct ReaderSummaryCard: View {
    let text: String
    @State private var ai = AISetupStatus.shared
    var body: some View {
        let missing = !ai.status(.writing).usable
        VStack(alignment: .leading, spacing: WhooshSpacing.s4) {
            // ai-setup W1: with no writing model the card is the drawn line (dot, "Summary", "needs a writing model", Connect a model) and no text
            HStack(spacing: WhooshSpacing.s6) {
                if missing { MissingSetupDot(needs: [.writing], id: "mac-main.reader.summary.dot") }
                else { Image(systemName: "sparkles").foregroundStyle(WhooshPalette.accent) }
                Text(Words.text("Summary")).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.label)
                if missing {
                    Text(AISetupWords.needsWriting).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                    Spacer(minLength: WhooshSpacing.s8)
                    AISetupButton(title: AISetupWords.connectModel, amber: true) { AISetupDoors.fix(.writing) }.whooshAx("mac-main.reader.summary.connect")
                }
            }
            if !missing { Text(text).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: WhooshMetrics.readerMeasure, alignment: .leading) }
        }
        .padding(.vertical, WhooshSpacing.s8).padding(.horizontal, WhooshSpacing.s14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(WhooshPalette.mine, in: RoundedRectangle(cornerRadius: WhooshRadius.sheet, style: .continuous))
        .readerFrame("summary")
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityIdentifier("mac-main.reader.summary-callout")
    }
}
