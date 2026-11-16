import Foundation
import Testing
@testable import WhooshUI

/// Nested labels (owner, 2026-11-07): paths up to LABEL_DEPTH_MAX levels, missing levels created.
private func core() throws -> FixtureCore {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    return try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
}

@Test func theFixtureTreeComesFlatWithPathsAndDepths() throws {
    let c = try core()
    let rows = c.labels()
    let nordlicht = try #require(rows.first { $0.name == "Nordlicht" })
    #expect(nordlicht.path == "Work > Clients > Nordlicht")
    #expect(nordlicht.depth == 3)
    let parent = try #require(rows.first { $0.id == nordlicht.parentId })
    #expect(parent.path == "Work > Clients")
    for r in rows { if let p = r.parentId { #expect(rows.firstIndex { $0.id == p }! < rows.firstIndex { $0.id == r.id }!) } }
}

@Test func labellingByPathCreatesTheMissingLevels() throws {
    let c = try core()
    let before = c.labels().count
    _ = try c.run(command: .label(messageIds: ["msg-04"], labelName: "Clients>Provanta>RFP", createIfMissing: true))
    let rows = c.labels()
    #expect(rows.count == before + 3)
    let rfp = try #require(rows.first { $0.path == "Clients > Provanta > RFP" })
    #expect(rfp.depth == 3)
    #expect(c.state(path: "label_assignment/msg-04") == #"["RFP"]"#)
    // the new subtree sits together, parent first
    let i = rows.firstIndex { $0.path == "Clients" }!
    #expect(rows[i + 1].path == "Clients > Provanta" && rows[i + 2].path == "Clients > Provanta > RFP")
}

@Test func aSingleNameFindsItsOnlyMatchAnywhereInTheTree() throws {
    let c = try core()
    let before = c.labels().count
    _ = try c.run(command: .label(messageIds: ["msg-01"], labelName: "Suppliers", createIfMissing: true))
    #expect(c.labels().count == before)
    #expect(c.state(path: "label_assignment/msg-01") == #"["Suppliers"]"#)
}

@Test func aSixthLevelIsRefusedInWords() throws {
    let c = try core()
    let before = c.labels().count
    #expect(throws: CoreError.self) {
        try c.run(command: .label(messageIds: ["msg-04"], labelName: "A > B > C > D > E > F", createIfMissing: true))
    }
    #expect(c.labels().count == before)
    _ = try c.run(command: .label(messageIds: ["msg-04"], labelName: "A > B > C > D > E", createIfMissing: true))
    #expect(c.labels().first { $0.path == "A > B > C > D > E" }?.depth == UInt8(WhooshConstants.LABEL_DEPTH_MAX))
}
