import SwiftUI

/// The follow-up capsule, in the undo bar's place: the action word and its one key, a quiet close. Same glass, height,
/// rise and acknowledge as UndoBar; no colour edge, the accent only on the word. Pattern: UndoBar's own capsule.
public struct SuggestionBar: View {
    let offer: SuggestionOffer
    /// What just happened ("Moved to junk"), first in the same pill: the action words, then the follow-up.
    var lead: String?
    /// The undo control stays in the pill beside the follow-up, key cap shown; nil when there is nothing to undo.
    var onUndo: (() -> Void)?
    var onAccept: () -> Void
    var onDismiss: () -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var risen = false

    public init(_ offer: SuggestionOffer, lead: String? = nil, onUndo: (() -> Void)? = nil, onAccept: @escaping () -> Void, onDismiss: @escaping () -> Void) {
        self.offer = offer; self.lead = lead; self.onUndo = onUndo; self.onAccept = onAccept; self.onDismiss = onDismiss
    }

    public var body: some View {
        HStack(spacing: WhooshSpacing.s10) {
            if let lead {
                // One line: the action words, then "·", then the follow-up; at a narrow list it wraps to two lines inside the pill.
                Text(lead).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label)
                    .fixedSize(horizontal: false, vertical: true)
                Text("·").whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).accessibilityHidden(true)
            }
            if let onUndo {
                Button(action: onUndo) {
                    HStack(spacing: WhooshSpacing.s6) {
                        Text(Words.text("Undo")).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.accent)
                            .fixedSize(horizontal: false, vertical: true)
                        KeyCap(UndoKey.letter)
                    }
                }
                .buttonStyle(.plain)
                .accessibilityLabel("\(Words.text("Undo")), key \(UndoKey.letter)")
                .accessibilityIdentifier("mac-main.undo.suggestion.undo")
                Text("·").whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).accessibilityHidden(true)
            }
            Button(action: onAccept) {
                HStack(spacing: WhooshSpacing.s6) {
                    Text(offer.title).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.accent)
                        .fixedSize(horizontal: false, vertical: true)
                    KeyCap(offer.key)
                }
            }
            .buttonStyle(.plain)
            .help(offer.detail)
            .accessibilityLabel("\(offer.title), key \(offer.key)")
            .accessibilityHint(offer.detail)
            Button(action: onDismiss) {
                Image(systemName: "xmark").font(.system(size: 9, weight: .semibold)).foregroundStyle(WhooshPalette.secondaryLabel)
                    .frame(width: WhooshSpacing.s16, height: WhooshSpacing.s16)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(SuggestionOffer.dismissWord)
        }
        .padding(.horizontal, WhooshSpacing.s16)
        .padding(.vertical, WhooshSpacing.s4)
        .frame(minHeight: WhooshMetrics.segmentHeight)
        .floatingPill()
        .offset(y: risen || reduceMotion ? 0 : WhooshMotion.undoRise)
        .opacity(risen ? 1 : 0)
        .onAppear { withAnimation(reduceMotion ? WhooshMotion.readerSwap : WhooshMotion.newRow) { risen = true } }
        .accessibilityIdentifier("mac-main.undo.suggestion")
    }
}

/// The host-ready foot: shows the core's one suggestion, if any. A list or reader lane drops it in the foot strip.
public struct SuggestionFoot: View {
    @State private var model = AppModel.shared
    var lead: String?
    var undoable: Bool
    public init(lead: String? = nil, undoable: Bool = true) { self.lead = lead; self.undoable = undoable }
    public var body: some View {
        _ = model.rows                       // re-reads after every command (a command refreshes the model)
        return Group {
            if let offer = SuggestionActs.current(model) {
                SuggestionBar(offer, lead: lead, onUndo: undoable ? { model.perform(.undo) } : nil, onAccept: { SuggestionActs.accept(model) }, onDismiss: { SuggestionActs.dismiss(model) })
                    .id(offer.id).transition(.opacity)
            }
        }
        .animation(WhooshMotion.readerSwap, value: SuggestionActs.current(model)?.id)
    }
}
