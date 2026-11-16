import SwiftUI

/// What a model's line says under its name (owner's pick M1, board decision-models-r1): the download's size, the licence, and the index score when the
/// index table gives one. The size is the strong part, as drawn.
struct DecisionModelFacts: Equatable {
    let size: String
    let rest: String
    /// The whole line as one string (what a check and VoiceOver read).
    var line: String { rest.isEmpty ? size : size + " · " + rest }

    @MainActor init(_ entry: DecisionModelEntry) {
        size = AttachmentSize.words(Int64(entry.downloadMB) * 1_000_000)
        var parts = [entry.licence]
        if let score = entry.score {
            parts.append(AIWords.indexScore(score.formatted(.number.precision(.fractionLength(1)).locale(AttachmentSize.locale))))
        }
        rest = parts.joined(separator: " · ")
    }
}

/// The mark at the left of a model's line: filled for the model in use (board `.rdo`, 15 pt; the filled ring is 5 pt of accent).
struct DecisionModelMark: View {
    let inUse: Bool
    var body: some View {
        Circle().strokeBorder(inUse ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(WhooshPalette.tertiaryLabel), lineWidth: inUse ? 5 : 1.5)
            .frame(width: 15, height: 15)
            .accessibilityHidden(true)
    }
}
