import SwiftUI

/// The bottom of the conversation for my own mail: every outgoing send of this thread as a card, then the thread's draft
/// card, then (round 5) the ROOM the floating slab needs: a spacer as tall as the slab plus its bottom inset while the
/// reply is open, so the last card is never drawn under the slab and a new send is visible on the frame it lands.
/// The reader mounts it after the last message block (hook "quickreply-r4 hook: reader").
public struct QuickReplyThreadCards: View {
    let threadId: String
    @State private var app = AppModel.shared   // local-ui: observed for the field's text and the draft
    @State private var spine = Spine.shared    // local-ui: observed for the outbox stream's ticks
    @State private var quick = QuickReplyModel.shared   // local-ui: observed for the open draft, focus, last delete and the slab's height
    /// local-ui: moves every LIVE_LOOK_MS while a card of this thread waits or sends, so the engine's state is read again (round 20: the
    /// stream sent no tick when the engine marked a send failed, and the card kept its countdown). Still while no card is pending.
    @State private var look = 0
    public init(threadId: String) { self.threadId = threadId }

    public var body: some View {
        let _ = look
        let cards = ThreadCards.outgoing(threadId: threadId, core: app.core)
        let accounts = app.core.accounts().map(AccountNames.localised)
        // Explicit dependencies: a draft save, update or delete moves draftsRevision, and the deleted line reads
        // lastDeletedDraft; the rows alone did not redraw this stack after a delete (round 4 probe, frames).
        let _ = (quick.draftsRevision, quick.lastDeletedDraft, app.rows)
        let drafts = quick.drafts.drafts(threadId: threadId, core: app.core)
        VStack(spacing: WhooshMetrics.blockGap) {
            ForEach(Array(cards.enumerated()), id: \.element.id) { i, card in
                ThreadOutgoingCard(card: card, index: i, account: accounts.first { $0.id == card.accountId },
                                   onUndo: { _ = ThreadCards.undo(outboxId: card.id, app) },
                                   onRetry: { _ = quick.retryFailed(outboxId: card.id, app) },
                                   onDiscard: { _ = quick.discardFailed(outboxId: card.id, app) },
                                   onEdit: { _ = quick.editFailed(outboxId: card.id, app) })
                .id(Self.revealId(card.id))
            }
            // The newest saved draft; the one the field has open is in the field already, so it is not drawn twice.
            if let d = ThreadCardsRegistry.cardDraft(drafts, quick) {
                ThreadDraftCard(draft: d, account: accounts.first { $0.id == d.accountId }, collapsed: Self.collapses(quick),
                                onOpen: { _ = quick.openDraft(d.id, app) },
                                onSend: { _ = quick.sendDraft(d.id, app) },
                                onDelete: { _ = quick.deleteDraft(d.id, app) })
                .id(Self.revealId(d.id))
            } else if quick.showsDeletedLine(threadId: threadId) {
                ThreadDraftDeletedLine { _ = quick.undoDraftDelete(app) }
                    .id(Self.revealId(ThreadCardIds.draftDeleted))
            }
            if quick.showsBox, quick.threadId == threadId, quick.slabHeight > 0 {
                Color.clear.frame(height: Self.room(quick.slabHeight)).whooshAx(ThreadCardIds.room).id(Self.revealId(ThreadCardIds.room))
            }
        }
        // A new send or a newly kept draft scrolls into view at once: "it should directly show up" (owner 17:19).
        // The cards are the thread's last blocks, so "into view" is the end: scrolled once the new height is laid out.
        .task(id: Self.pendingKey(cards)) {
            guard !Self.pendingKey(cards).isEmpty else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .milliseconds(Self.liveLookMs))
                look &+= 1
            }
        }
        .onChange(of: cards.map(\.id)) { old, new in if let id = new.last, !old.contains(id) { Self.reveal(id) } }
        .onChange(of: drafts.first?.id) { _, id in if let id { Self.reveal(id) } }
        .onChange(of: quick.lastDeletedDraft) { _, id in if id != nil { Self.reveal(ThreadCardIds.draftDeleted) } }
        .onChange(of: quick.slabHeight) { old, new in if new > old { Self.reveal(Self.slabTarget(lastCardId: cards.last?.id)) } }
        // A send that reached `sent` belongs to the thread now: read the thread again, so the core's thread (which gains
        // the sent message once contracts delta 15 serves it) redraws above the cards.
        .onChange(of: cards.filter { if case .sent = $0.state { return true }; return false }.map(\.id)) { old, new in
            // Round 20 (main115 judge: the sent card "mostly hidden under the reply card" at 960 dark): the refresh adds the sent message to
            // the thread ABOVE this stack, which pushes the card down under the slab while this stack's own height does not change, so the
            // growth reveal never ran. The landed card is shown again at the end.
            if new.count > old.count { app.refresh(); if let id = new.last { Self.reveal(id) } }
        }
        // The end stays the end: when this stack grows (a card's row, the room) while the thread was already scrolled to
        // its end, the reveal runs again on the new height, so nothing of ours slides under the slab after the key frame.
        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { old, new in
            guard new > old else { return }
            let id = Self.growTarget(lastCardId: cards.last?.id, firstDraftId: drafts.first?.id)
            let m = ReaderScroll.shared
            if m.maxOffset - m.metrics.offset <= (new - old) + WhooshSpacing.s16 { Self.reveal(id) }
        }
    }

    /// The room under the last card while the slab floats: the slab's drawn height and its inset from the pane's foot
    /// (the reader's own bottom padding is the slab's bottom gap).
    static func room(_ slabHeight: CGFloat) -> CGFloat { slabHeight - WhooshSpacing.s16 + WhooshMetrics.blockGap }

    /// The draft card folds to one line while the field has the keys and the thread does not fit its pane.
    static func collapses(_ q: QuickReplyModel) -> Bool { q.fieldFocused && ReaderScroll.shared.maxOffset > 0 }

    /// One main-actor turn later the reader scrolls so the new card is in view. While the slab floats, the END is the
    /// only right place (it leaves the room clear under the last card, so every card sits above the slab; the id reveal
    /// centres a card in the whole viewport, half of it under the slab: round 5, run 4), and the end is reached through
    /// the ROOM's id: the ScrollViewReader lays it out fresh, where `scroll(to:)` clamps to a content height the
    /// geometry callback may not have delivered yet (run 6: the scroll stopped one card short). The room is the last
    /// block, so centring it is clamped to the end. With the slab closed the card's own id goes first, the end after.
    static func reveal(_ id: String) {
        Task { @MainActor in
            ReaderScroll.shared.show(revealId(QuickReplyModel.shared.isOpen ? ThreadCardIds.room : id))
            _ = ReaderScroll.shared.scroll(to: .greatestFiniteMagnitude)
        }
    }

    /// The id the reader scrolls to when the slab opens: the last card, or with no card (R on a thread with none) the room,
    /// which is the new last block; the slab is open then, so `reveal` goes to the room whatever id it is given.
    static func slabTarget(lastCardId: String?) -> String { lastCardId ?? ThreadCardIds.room }

    /// The id the reader scrolls to when this stack grows: the last card, else the newest draft, else the room.
    static func growTarget(lastCardId: String?, firstDraftId: String?) -> String { lastCardId ?? firstDraftId ?? ThreadCardIds.room }

    static func revealId(_ id: String) -> String { "quickreply.thread.card.\(id)" }

    /// How often a pending card reads the engine again: twice a second, under the one-second step of the countdown it shows.
    static let liveLookMs = 500
    /// The cards still waiting or sending, with their states: empty when none is (then nothing ticks).
    static func pendingKey(_ cards: [ThreadCards.Outgoing]) -> String {
        cards.compactMap { c -> String? in
            switch c.state { case .waiting, .sending, .offline: return "\(c.id):\(c.state)"; default: return nil }
        }.joined(separator: ",")
    }
}
