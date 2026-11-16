import SwiftUI

/// The draft card (board cards-s3 `.mine.clamp`): full thread width, LIFTED (a draft is still yours), the head line
/// (emblem, "Vous", the address; the "Brouillon" mark at the right), the first lines of the text, then "Envoyer ⌘↵" ·
/// "Supprimer ⌫". A click on the card opens it in the field. While the field holds the keyboard and the thread is
/// taller than the pane, the card collapses to one line, so the field keeps its room. Its height changes on the
/// frame the focus does: no animated height.
struct ThreadDraftCard: View {
    let draft: DraftSummary
    let account: AccountSummary?
    let collapsed: Bool
    var onOpen: () -> Void
    var onSend: () -> Void
    var onDelete: () -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var shown = false

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            ThreadCardHead(account: account, addressAx: ThreadCardIds.draft + ".address") {
                HStack(spacing: WhooshSpacing.s6) {
                    if collapsed {
                        Text(draft.firstLines.replacingOccurrences(of: "\n", with: " ")).whooshText(WhooshTypography.caption)
                            .foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1).truncationMode(.tail)
                            .layoutPriority(-1)   // the preview gives way: the mark keeps its word at 960
                    }
                    Text(ThreadCardCopy.draft).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
                        .padding(.horizontal, WhooshSpacing.s6).frame(height: WhooshSpacing.s16)
                        .background(WhooshPalette.fill, in: Capsule())
                    if !collapsed {
                        Text(RowFacts.time(draft.savedAt)).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                    } else {
                        pills   // one line: the actions ride the head (round 4), so the field keeps its room
                    }
                }
            }
            if !collapsed {
                Group {   // the text column under "Vous", as on the outgoing card (round 18)
                    Text(draft.firstLines).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label)
                        .lineLimit(WhooshConstants.DRAFT_SUMMARY_LINES).truncationMode(.tail)
                        .frame(maxWidth: WhooshMetrics.readerMeasure, alignment: .leading)
                    HStack(spacing: WhooshSpacing.s8) { pills; Spacer(minLength: 0) }
                }
                .padding(.leading, ThreadCardInset.text(account))
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .threadLift(ThreadFootLayout.fill(.draft) == .lifted)
        .threadHeadMarks()                // the head's address and You, marked outside the lift's layer effect
        .whooshAx(ThreadCardIds.draft)   // the drawn card's frame, inside the author inset (A147 edge checks read cards, not the column)
        .threadFootEdges(.draft)
        .contentShape(RoundedRectangle(cornerRadius: WhooshGlass.Lift.cardRadius, style: .continuous))
        .onTapGesture(perform: onOpen)
        .modifier(HoverAck(radius: WhooshGlass.Lift.cardRadius))
        .opacity(shown || reduceMotion ? 1 : 0)
        .onAppear { withAnimation(WhooshMotion.newRow) { shown = true }; QuickReplyThreadCards.reveal(draft.id) }
    }

    private var pills: some View {
        HStack(spacing: WhooshSpacing.s8) {
            ThreadCardPill(word: ThreadCardCopy.send, cap: ThreadCardCopy.sendCap, accent: true, ax: ThreadCardIds.draftSend, action: onSend)
            ThreadCardPill(word: ThreadCardCopy.delete, cap: ThreadCardCopy.deleteCap, ax: ThreadCardIds.draftDelete, action: onDelete)
        }
    }
}

/// After Delete on a draft with words (board `.gone`): one quiet line where the card was, the trash mark,
/// "Brouillon supprimé", and Undo Z (the core's undo entry) at its right.
struct ThreadDraftDeletedLine: View {
    var onUndo: () -> Void
    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Image(systemName: "trash").font(.system(size: WhooshTypography.caption.size)).foregroundStyle(WhooshPalette.secondaryLabel).accessibilityHidden(true)
            Text(ThreadCardCopy.draftDeleted).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
            Spacer(minLength: WhooshSpacing.s8)
            Button(action: onUndo) {
                HStack(spacing: WhooshSpacing.s4) {
                    Text(Words.text("Undo")).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.accent)   // a delete is undone ("Cancel" is the word of a send in its window)
                    KeyCap(ThreadCardCopy.undoCap)
                }
            }
            .buttonStyle(.plain).help(ThreadCardCopy.undoHelp).whooshAx(ThreadCardIds.draftUndo)
        }
        .padding(.horizontal, WhooshSpacing.s16).frame(height: WhooshMetrics.segmentHeight + WhooshSpacing.s8)
        .overlay(alignment: .top) { Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator) }
        .transition(.opacity)
        .onAppear { QuickReplyThreadCards.reveal(ThreadCardIds.draftDeleted) }
        .whooshAx(ThreadCardIds.draftDeleted)
    }
}
