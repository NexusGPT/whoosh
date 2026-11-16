import Foundation
import Testing

extension Contract {
    /// `rows <ax-id>`: the rows a region shows (docs/CONTROL-SOCKET.md, field sets served at this commit).
    /// Expected values come from docs/design/fixtures/mac-main-sample.json, not from the app.
    @Suite("rows") struct RowsVerb {
        @Test func listRowsAreThePrimaryTabInFixtureOrderPinnedFirst() throws {
            let r = try Ctl.ok("rows", ["mac-main.list"])
            #expect(r.keys == ["ok", "rows"])
            let rows = try #require(r.fields["rows"] as? [[String: Any]])
            #expect(rows.count == Ctl.primaryIds.count)
            for (i, row) in rows.enumerated() {
                let id = Ctl.primaryIds[i]
                let m = Ctl.message(id)
                #expect(row["ax"] as? String == "mac-main.list.row.\(i)", "row \(i) ax")
                #expect(row["id"] as? String == id, "row \(i) id")
                #expect(row["sender"] as? String == m["sender"] as? String, "row \(i) sender")
                #expect(row["subject"] as? String == m["subject"] as? String, "row \(i) subject")
                #expect(row["account"] as? String == m["account"] as? String, "row \(i) account")
                #expect(isJSONBool(row["unread"]) && row["unread"] as? Bool == m["unread"] as? Bool, "row \(i) unread is a JSON bool equal to the fixture")
                #expect(isJSONBool(row["pinned"]) && row["pinned"] as? Bool == m["pinned"] as? Bool, "row \(i) pinned")
                #expect((row["chips"] as? [String]) != nil, "row \(i) chips is a list of words")
                // base registration; a feature lane may add fields, never drop these
                #expect(Set(["ax", "id", "sender", "subject", "account", "unread", "pinned", "chips"]).isSubset(of: Set(row.keys)), "row \(i) keys \(row.keys.sorted())")
            }
        }

        @Test func tabRowsCarryTitleUnreadAndWaitingFromTheFixture() throws {
            let r = try Ctl.ok("rows", ["mac-main.tabs"])
            #expect(r.keys == ["ok", "rows"])
            let rows = try #require(r.fields["rows"] as? [[String: Any]])
            #expect(rows.count == Ctl.tabs.count)
            for (i, row) in rows.enumerated() {
                let t = Ctl.tabs[i]
                #expect(row["ax"] as? String == "mac-main.tabs.\(i)")
                #expect(row["id"] as? String == t["id"] as? String)
                #expect(row["title"] as? String == t["name"] as? String)
                #expect(row["unread"] as? Int == t["unread"] as? Int, "tab \(i) unread")
                #expect(row["waiting"] as? Int == t["waiting"] as? Int, "tab \(i) waiting")
                #expect(Set(["ax", "id", "title", "unread", "waiting"]).isSubset(of: Set(row.keys)))
            }
        }

        @Test func aRegionNobodyRegistersIsNotOkNeverAnEmptyList() throws {
            let r = try Ctl.ask("rows", ["mac-main.no-such-region"])
            #expect(!r.ok, "delta 3: an unregistered region is ok:false")
            #expect(r.error == "no rows at mac-main.no-such-region")
            #expect(r.fields.isEmpty, "and carries no rows key")
            // positive control: a registered region reads a real, non-zero list
            #expect(try Ctl.rows("mac-main.list").count > 0)
        }

        @Test func noAxIdIsRefusedInWords() throws {
            let r = try Ctl.ask("rows")
            #expect(!r.ok)
            #expect(r.error == "rows needs an ax-id")
        }

        @Test func aRowLeavesTheListWhenItIsArchived() throws {
            let before = try Ctl.rows("mac-main.list")
            try Ctl.ok("key", ["E"])
            defer { _ = try? Ctl.ask("key", ["Z"]) }
            let after = try Ctl.rows("mac-main.list")
            #expect(after.count == before.count - 1)
            #expect(!after.contains { $0["id"] as? String == before[0]["id"] as? String })
            #expect(after.first?["ax"] as? String == "mac-main.list.row.0", "indices are renumbered from 0")
        }
    }
}
