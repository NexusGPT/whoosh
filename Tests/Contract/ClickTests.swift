import Foundation
import Testing

extension Contract {
    /// `click <ax-id>`: ok:false when nothing is registered at the id, or it did nothing (docs/CONTROL-SOCKET.md).
    @Suite("click") struct ClickVerb {
        @Test func theDockRegistrationIsOkWithNoFields() throws {
            let r = try Ctl.ask("click", ["dock.whoosh"])
            #expect(r.ok, "click dock.whoosh: \(r.error ?? "")")
            #expect(r.keys == ["ok"])
            #expect(try Ctl.state()["surface"] as? String == "mac-main")
        }

        @Test func allAccountsClearsTheAccountFilter() throws {
            let r = try Ctl.ask("click", ["mac-main.accounts.all"])
            #expect(r.ok, "\(r.error ?? "")")
            #expect(r.keys == ["ok"])
            #expect(try Ctl.state()["account_filter"] is NSNull)
            // the list is the whole Primary tab: the click did not narrow it
            #expect(try Ctl.rows("mac-main.list").count == Ctl.primaryIds.count)
        }

        @Test func anIdNobodyRegistersIsNotOkAndNamesTheId() throws {
            let r = try Ctl.ask("click", ["mac-main.no-such.thing.3"])
            #expect(!r.ok)
            #expect(r.error == "nothing to click at mac-main.no-such.thing.3")
            #expect(r.keys == ["ok", "error"])
            // positive control: a registered id is ok on the same build
            #expect(try Ctl.ask("click", ["dock.whoosh"]).ok)
        }

        @Test func noAxIdIsRefusedInWords() throws {
            let r = try Ctl.ask("click")
            #expect(!r.ok)
            #expect(r.error == "click needs an ax-id")
        }

        @Test func garbageIdsAreRefusedNotCoerced() throws {
            for bad in ["", "   ", "dock.whoosh ", "DOCK.WHOOSH", String(repeating: "a", count: 10_000)] {
                let r = try Ctl.ask("click", [bad])
                #expect(!r.ok, "click on \(bad.prefix(20))' must not be ok")
            }
            #expect(try Ctl.ask("click", ["dock.whoosh"]).ok, "positive control")
        }
    }
}
