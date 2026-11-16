import Foundation

/// The flow of a label through the delta 17 commands, written AFTER the label's fields (`createLabelFull` / `updateLabel` carry chain 0).
/// Order matters to the contract: the fallback goes first (the exit filter cannot be switched on without one), then the three cards,
/// the AI's ticks, then each card's chains. Only what differs from `was` is sent (a new label differs from the defaults).
extension LabelInspectorModel {
    private func send(_ c: Command) throws -> CommandOutcome { try core.run(command: c) }

    func writeFlow(_ id: String, _ d: LabelDraft, was: FlowCards?) throws {
        let f = d.flow, s = was ?? FlowCards()
        if let fb = f.fallback, fb != s.fallback { _ = try send(.setLabelExitFallback(labelId: id, fallback: fb)) }
        for (card, on, before) in [(FlowCard.entry, f.entryOn, s.entryOn), (.ai, f.aiOn, s.aiOn), (.exit, f.exitOn, s.exitOn)] where on != before {
            _ = try send(.setLabelCard(labelId: id, card: card, enabled: on))
        }
        if f.reads != s.reads { _ = try send(.setLabelAiReads(labelId: id, reads: f.reads.core)) }
        try reconcile(id, .entry, desired: f.others, chainZero: !d.entry.isEmpty)
        try reconcile(id, .exit, desired: f.exits, chainZero: false)
    }

    /// Brings the chains the core holds for (label, placement) to the drawn ones: each drawn chain is updated when the core still has its
    /// number and added when it does not (a new chain, or one a core dropped with the label's fields), each held chain no longer drawn is
    /// removed. The first entry chain is the label's own (written with its fields) and is never touched here.
    private func reconcile(_ id: String, _ placement: ChainPlacement, desired: [ChainDraft], chainZero: Bool) throws {
        let word = placement == .entry ? "entry" : "exit"
        let have = StoredChains.decode(core.state(path: "label/\(id)/chains/\(word)"))
        let own: UInt32? = chainZero ? have.first?.id : nil
        var keep = Set<UInt32>()
        for c in desired.map({ $0.settled() }) where !c.isEmpty {
            if let n = c.chainId, n != own, have.contains(where: { $0.id == n }) {
                _ = try send(.updateLabelChain(labelId: id, placement: placement, chainId: n, chain: c.ruleSet))
                keep.insert(n)
            } else {
                let out = try send(.addLabelChain(labelId: id, placement: placement, chain: c.ruleSet))
                if let n = out.createdId.flatMap(UInt32.init) { keep.insert(n) }
            }
        }
        for h in have where h.id != own && !keep.contains(h.id) {
            _ = try send(.removeLabelChain(labelId: id, placement: placement, chainId: h.id))
        }
    }
}
