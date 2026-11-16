import Foundation

/// `Command.saveDecisionList` in the test engine (gap-decisions): the rules the owner files under a node of the label tree on the Labels decision page.
/// The same contract as the real core (core/crates/engine/src/decision_list.rs): only the "labels" page files under nodes; the lists are the same length; an unknown node is
/// NotFound and a rule with no condition is refused, both before anything is written; a rule the node already holds as an entry chain is not written twice; each node
/// that gained a chain is re-run at once, so what its rules catch is filed (`label/<id>/rerun_count` and `label_assignment` move). A rule of the owner may file under any node,
/// also one whose AI card is off; the pipeline of a node whose AI card is on is unchanged (step 1 picks, the AI rules).
extension FixtureCore {
    func saveDecisionListLocked(page: String, nodeIds: [String], rules: [RuleSet]) throws -> CommandOutcome {
        guard page == "labels" else {
            throw CoreError.Invalid(field: "page_id", reasonWords: "only the Labels page files mail under a node of the label tree, not “\(page)”")
        }
        guard nodeIds.count == rules.count else { throw CoreError.Invalid(field: "rules", reasonWords: "every rule needs the node it files under") }
        for (id, rule) in zip(nodeIds, rules) {
            _ = try labelRow(id)
            try validateChain(rules: rule)
        }
        var written: [String] = []
        for (id, rule) in zip(nodeIds, rules) {
            var chains = chainsLocked(id, .entry)
            if !chains.contains(where: { $0.rules == rule }) {
                chains.append(LabelChain(chainId: nextChainId(existing: chains.map(\.chainId)), rules: rule))
                writeChainsLocked(id, .entry, chains)
            }
            if !written.contains(id) { written.append(id) }
        }
        for id in written { _ = try rerunLabelLocked(id) }
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }
}
