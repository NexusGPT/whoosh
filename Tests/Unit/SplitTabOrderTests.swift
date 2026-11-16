import Foundation
import Testing
@testable import WhooshUI

// fix-taborder: the test engine's split-tab reorder follows the same rule as the real core's door (contracts inbox_views.rs `check_tab_order`):
// the views first and in their order, every other order judged by the list ("the order must list every tab once"). Written uncompiled; never run.
// Broken copy per behaviour is named above each test.

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func fixtureCore() throws -> FixtureCore {
    try FixtureCore(url: repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
}
private func refusal(_ body: () throws -> Void) -> (field: String, words: String)? {
    do { try body(); return nil } catch CoreError.Invalid(let f, let w) { return (f, w) } catch { return ("\(error)", "\(error)") }
}

// Broken copy: FixtureCore.swift `case .reorderSplitTabs`: `splitTabRows = tabsOnly.compactMap ...` becomes `splitTabRows = splitTabRows` (the order is not written): fails at the first #expect.
// Not claimed: that the strip (`tabIds`) follows it for a pinned sub-label (the fixture keeps its tree place, RootDelta7Tests d7TabsReorderHideAndComeBackToTheDefaults).
@Test func theOrderIsWritten() throws {
    let core = try fixtureCore()
    let ids = core.splitTabs().map(\.id)
    #expect(ids.count >= 3)
    let order = Array(ids.dropLast().reversed()) + [ids[ids.count - 1]]   // a pinned sub-label keeps its tree place (delta 9): the categories are the ones that move
    _ = try core.run(command: .reorderSplitTabs(ids: order))
    #expect(core.splitTabs().map(\.id) == order)
    #expect(core.splitTabs().map(\.order) == Array(0..<UInt32(order.count)))
}

// Broken copy: FixtureCore.swift `case .reorderSplitTabs`: `let lead = ...` becomes `let lead = ids.prefix(0)` (a leading view is then a tab nobody listed): fails at the first #expect.
@Test func theViewsFirstAreLetByAsTheRealCoresOrderListsPrimary() throws {
    let core = try fixtureCore()
    let ids = core.splitTabs().map(\.id)
    let moved = Array(ids.dropLast().reversed()) + [ids[ids.count - 1]]
    _ = try core.run(command: .reorderSplitTabs(ids: ["primary"] + moved))
    #expect(core.splitTabs().map(\.id) == moved)
    _ = try core.run(command: .reorderSplitTabs(ids: ["primary", "all"] + ids))
    #expect(core.splitTabs().map(\.id) == ids)
}

// Broken copy: contracts/src/inbox_views.rs `check_tab_order`: `if !leading || p < last` becomes `if p < last`: the first two cases are let by and fail at their #expect.
@Test func aViewMovedIsRefusedInTheViewsWordsAndNothingChanges() throws {
    let core = try fixtureCore()
    let ids = core.splitTabs().map(\.id)
    let before = core.splitTabs()
    let late = refusal { _ = try core.run(command: .reorderSplitTabs(ids: ids + ["primary"])) }
    #expect(late?.field == "view_not_a_label")
    #expect(late?.words == "Primary is a view of the inbox, not a label, so it cannot be reordered: the views stay first.")
    let after = refusal { _ = try core.run(command: .reorderSplitTabs(ids: [ids[0], "all"] + ids.dropFirst())) }
    #expect(after?.words == "All is a view of the inbox, not a label, so it cannot be reordered: the views stay first.")
    let swapped = refusal { _ = try core.run(command: .reorderSplitTabs(ids: ["all", "primary"] + ids)) }
    #expect(swapped?.field == "view_not_a_label")
    #expect(core.splitTabs() == before)
}

// Broken copy: FixtureCore.swift `case .reorderSplitTabs`: `tabsOnly.count == splitTabRows.count` is removed from the guard: the short case is let by and fails at its #expect.
@Test func aTabTwiceOrMissingIsRefusedInTheStoresWordsAndNothingChanges() throws {
    let core = try fixtureCore()
    let ids = core.splitTabs().map(\.id)
    let before = core.splitTabs()
    let words = "the order must list every tab once"
    let short = refusal { _ = try core.run(command: .reorderSplitTabs(ids: Array(ids.dropLast()))) }
    #expect(short?.field == "ids")
    #expect(short?.words == words)
    var twice = ids
    twice[1] = twice[0]
    let dup = refusal { _ = try core.run(command: .reorderSplitTabs(ids: twice)) }
    #expect(dup?.words == words)
    let viewTwice = refusal { _ = try core.run(command: .reorderSplitTabs(ids: ["primary", "primary"] + ids)) }
    #expect(viewTwice?.words == words)
    let unknown = refusal { _ = try core.run(command: .reorderSplitTabs(ids: Array(ids.dropLast()) + ["nope"])) }
    #expect(unknown?.words == words)
    #expect(core.splitTabs() == before)
}
