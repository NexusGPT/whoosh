import SwiftUI

/// An outgoing reply in the thread (board cards-s3 `.mine`): full thread width, the head line (the sending account's
/// emblem, "Vous", the address; at the right the state: the ring and "Envoi dans N s" while the window is open, then
/// "Envoyé 23:19"; a warning dot and "Non envoyé" when the server refused it), the text, and the action row: "Annuler Z"
/// while it waits; the failure's reason block then "Réessayer ⌘↵" · "Supprimer ⌫". LIFTED as glass while it is yours
/// (waiting, sending, failed); it LANDS flat on the frame the window closes, the Aurora light leaving the ring across
/// the card (AuroraLanding) while the glass clears in its wake (ThreadLift). Nothing waits on the light: "Envoyé" and the
/// landed state are on the landing frame; "Annuler" fades and its row collapses (the one collapsing row, MOTION rule 5).
struct ThreadOutgoingCard: View {
    let card: ThreadCards.Outgoing
    let index: Int
    let account: AccountSummary?
    var onUndo: () -> Void
    var onRetry: () -> Void
    var onDiscard: () -> Void
    var onEdit: () -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var seenAt = Date()
    @State private var lastOffset = 0.0   // the reader's scroll offset at this card's last move
    @State private var shown = false
    @State private var landing: AuroraLanding?
    @State private var ringCentre = CGPoint.zero
    @State private var actsShown: Bool
    private typealias C = WhooshConstants

    init(card: ThreadCards.Outgoing, index: Int, account: AccountSummary?, onUndo: @escaping () -> Void,
         onRetry: @escaping () -> Void = {}, onDiscard: @escaping () -> Void = {}, onEdit: @escaping () -> Void = {}) {
        self.card = card; self.index = index; self.account = account
        self.onUndo = onUndo; self.onRetry = onRetry; self.onDiscard = onDiscard; self.onEdit = onEdit
        _actsShown = State(initialValue: Self.hasActs(card.state))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            TimelineView(.animation(minimumInterval: card.retryAtMs != nil ? 1 : 1.0 / 30, paused: !(isWaiting || card.retryAtMs != nil))) { ctx in
                ThreadCardHead(account: account) { state(now: ctx.date) }
            }
            Group {   // the text column starts under "Vous", after the emblem (board cards-s3: the body is set off from the mark)
                Text(card.text).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label)
                    .frame(maxWidth: WhooshMetrics.readerMeasure, alignment: .leading).textSelection(.enabled)
                if case .failed(let words, let code, let accountName) = card.state { reason(words, code, accountName) }
                if actsShown { acts.transition(.opacity) }
            }
            .padding(.leading, ThreadCardInset.text(account))
        }
        .coordinateSpace(.named(Self.space))
        .frame(maxWidth: .infinity, alignment: .leading)
        .threadLift(isLifted, landing: landing)
        .threadHeadMarks()   // the pills' frames, marked outside the lift's layer effect (round 18)
        .threadFootEdges(.outgoing)
        // In at once (the stack reflows on the input frame, so no card is ever drawn over its neighbour), then a fade:
        // opacity only, never a moving frame on the thread's stack (docs/MOTION.md rule 5).
        .opacity(shown || reduceMotion ? 1 : 0)
        .onAppear {
            withAnimation(WhooshMotion.newRow) { shown = true }
            // Laid out now: the reveal can find it (an onChange alone fired before the stack had the new height).
            if isLifted { QuickReplyThreadCards.reveal(card.id) }
        }
        .onChange(of: card.state) { old, new in stateChanged(from: old, to: new) }
        // The thread above can still grow after the send (the open message's body lays out late: +216 pt on main44,
        // card pushed from y 385 to 601, under the slab at 574). While this card waits, a move DOWN that the person
        // did not scroll reveals it again; a scroll by the person (offset changed) is left alone.
        .onGeometryChange(for: CGFloat.self) { $0.frame(in: .global).minY } action: { old, new in
            let offset = ReaderScroll.shared.metrics.offset
            defer { lastOffset = offset }
            guard isLifted, new > old + 0.5, abs(offset - lastOffset) < 0.5 else { return }
            QuickReplyThreadCards.reveal(card.id)
        }
        .whooshAx(ThreadCardIds.outgoing(index))
    }

    private static let space = "thread-card"
    private var isWaiting: Bool { if case .waiting = card.state { return true }; return false }
    /// Yours to change: waiting in its window, on its way, or refused. Sent is on the record.
    private var isLifted: Bool { if case .sent = card.state { return false }; return true }
    private static func hasActs(_ s: ThreadCards.OutState) -> Bool {
        switch s { case .waiting, .failed: return true; default: return false }
    }
    private static func hadRing(_ s: ThreadCards.OutState) -> Bool {
        switch s { case .waiting, .sending: return true; default: return false }
    }

    /// The window closed: the landing light starts at the ring, "Annuler" fades and its row collapses after the light.
    private func stateChanged(from old: ThreadCards.OutState, to new: ThreadCards.OutState) {
        if case .sent = new, Self.hadRing(old) {
            // The ring's centre in the lifted card's own points (ThreadLift pads the content by s16 × s14).
            landing = AuroraLanding(id: card.id, origin: CGPoint(x: ringCentre.x + WhooshSpacing.s16, y: ringCentre.y + WhooshSpacing.s14))
        }
        let acts = Self.hasActs(new)
        guard acts != actsShown else { return }
        if acts || reduceMotion {
            withAnimation(.easeOut(duration: Double(C.LAND_STATE_MS) / 1000)) { actsShown = acts }
        } else {
            withAnimation(.easeOut(duration: Double(C.LAND_ACTS_MS) / 1000).delay(Double(C.LAND_ACTS_AT_MS) / 1000)) { actsShown = false }
        }
    }

    @ViewBuilder private func state(now: Date) -> some View {
        switch card.state {
        case .waiting(let until):
            let c = countdown(until)
            HStack(spacing: WhooshSpacing.s6) {
                LiftRing(fraction: c.fraction(at: now))
                    .onGeometryChange(for: CGRect.self) { $0.frame(in: .named(Self.space)) } action: { ringCentre = CGPoint(x: $0.midX, y: $0.midY) }
                // Round 20 (owner 07:28, "after send, I still see cancel with z"; the card read "Sending in 0 s"): once the window is over the
                // words say the send is on its way until the engine's next state is read (sending, sent or not sent).
                Text(c.isExpired(at: now) ? ThreadCardCopy.sendingNow : ThreadCardCopy.sendingIn(c.secondsLeft(at: now)))
                    .whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                    .contentTransition(reduceMotion ? .opacity : .numericText(countsDown: true))
            }
        case .offline:
            Text(ThreadCardCopy.offline).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
        case .sending:
            HStack(spacing: WhooshSpacing.s6) {
                LiftRing(fraction: 0)
                    .onGeometryChange(for: CGRect.self) { $0.frame(in: .named(Self.space)) } action: { ringCentre = CGPoint(x: $0.midX, y: $0.midY) }
                Text(ThreadCardCopy.sendingNow).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
            }
        case .sent(let at):
            Text(ThreadCardCopy.sent(at)).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                .transition(.opacity)
        case .failed:
            if let next = card.retryAtMs {
                // the engine tries again by itself: the head counts down to its next try, a second at a time
                let left = Int((Double(next) / 1000 - now.timeIntervalSince1970).rounded(.up))
                HStack(spacing: WhooshSpacing.s6) {
                    LiftRing(fraction: 0)
                    Text(left > 0 ? ThreadCardCopy.tryingAgainIn(left) : ThreadCardCopy.sendingNow)
                        .whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                        .contentTransition(reduceMotion ? .opacity : .numericText(countsDown: true))
                }
            } else {
                HStack(spacing: WhooshSpacing.s6) {
                    ThreadCardDot()
                    Text(ThreadCardCopy.notSent).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.label)
                }
            }
        }
    }

    /// `.reason`: the dot, the sentence naming the account, and the server's words with its code (delta 12).
    private func reason(_ words: String, _ code: String?, _ accountName: String) -> some View {
        HStack(alignment: .top, spacing: WhooshSpacing.s8) {
            ThreadCardDot().padding(.top, WhooshSpacing.s6)
            VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                Text(sentence(code, accountName.isEmpty ? (account?.name ?? "") : accountName))
                    .whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.label)
                HStack(spacing: WhooshSpacing.s6) {
                    Text(words).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(2)
                    if let code, !code.isEmpty, !code.hasPrefix("network_") {   // a server's own code is shown; the engine's step word is already in the words
                        Text(code).font(.system(size: WhooshTypography.caption.size, design: .monospaced))
                            .foregroundStyle(WhooshPalette.secondaryLabel)
                            .padding(.horizontal, WhooshSpacing.s4).background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: WhooshRadius.item, style: .continuous))
                    }
                }
            }
        }
        .frame(maxWidth: WhooshMetrics.readerMeasure, alignment: .leading)
    }

    /// The card's sentence: a broken connection is not a refusal (the engine's code is `network_<step>`); while the engine tries again it says so.
    private func sentence(_ code: String?, _ account: String) -> String {
        guard code?.hasPrefix("network_") == true else { return card.retryAtMs != nil ? ThreadCardCopy.laterRetrying(account) : ThreadCardCopy.refused(account) }
        return card.retryAtMs != nil ? ThreadCardCopy.brokeRetrying(account) : ThreadCardCopy.broke(account)
    }

    @ViewBuilder private var acts: some View {
        HStack(spacing: WhooshSpacing.s8) {
            switch card.state {
            case .waiting(let until):
                // Cancel only while the window is open: it leaves on the frame the window ends (round 20), not when the engine next says so.
                TimelineView(.animation(minimumInterval: 1.0 / 30, paused: !isWaiting)) { ctx in
                    if !countdown(until).isExpired(at: ctx.date) {
                        ThreadCardPill(word: ThreadCardCopy.undo, cap: ThreadCardCopy.undoCap, ax: ThreadCardIds.outgoingUndo(index), action: onUndo)
                    }
                }
            case .failed:
                // ⌘↵ and ⌘⌫ act on the conversation's draft card when one is drawn (QuickReplyFeature.sendKey, deleteShownCard): this card then
                // draws its pills without those two keys, so a key is drawn once on the screen (owner, 13 Nov, picture 206).
                let keysHere = !ThreadCardsRegistry.hasShownDraft(AppModel.shared)
                ThreadCardPill(word: card.retryAtMs != nil ? ThreadCardCopy.retryNow : ThreadCardCopy.retry, cap: keysHere ? ThreadCardCopy.sendCap : "", accent: true, ax: ThreadCardIds.outgoingRetry(index), action: onRetry)
                ThreadCardPill(word: ThreadCardCopy.edit, cap: ThreadCardCopy.editCap, ax: ThreadCardIds.outgoingEdit(index), action: onEdit)
                ThreadCardPill(word: ThreadCardCopy.delete, cap: keysHere ? ThreadCardCopy.deleteCap : "", ax: ThreadCardIds.outgoingDiscard(index), action: onDiscard)
            default:
                EmptyView()
            }
            Spacer(minLength: 0)
        }
    }

    /// The window the outbox granted (until minus created) counted from the frame the card appeared.
    private func countdown(_ untilMs: Int64) -> UndoCountdown {
        if QuickReplyModel.shared.outboxId == card.id, case .counting(_, let c) = QuickReplyModel.shared.phase { return c }
        return UndoCountdown(startedAt: seenAt)
    }
}
