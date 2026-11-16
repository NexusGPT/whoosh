import Foundation
import Testing

extension Contract {
    /// `core state <path>`: the core's stored value at a path (docs/CONTROL-SOCKET.md; delta 5: `/` and `.` both separate).
    @Suite("core") struct CoreVerb {
        @Test func storedValuesMatchTheFixture() throws {
            let r = try Ctl.ok("core", ["state", "message/msg-01/archived"])
            #expect(r.keys == ["ok", "value"])
            #expect(isJSONBool(r.fields["value"]) && r.fields["value"] as? Bool == false)
            #expect(try Ctl.ok("core", ["state", "message/msg-03/account"]).fields["value"] as? String == "acc-18")
            #expect(try Ctl.ok("core", ["state", "counts/primary"]).fields["value"] as? Int == Ctl.tabUnread("primary"))
            #expect(try Ctl.ok("core", ["state", "pin/msg-01"]).fields["value"] as? Bool == true)
            #expect(try Ctl.ok("core", ["state", "pin/msg-03"]).fields["value"] as? Bool == false)
            #expect(try Ctl.ok("core", ["state", "message/msg-01/read"]).fields["value"] as? Bool == false, "msg-01 is unread in the fixture")
            #expect(try Ctl.ok("core", ["state", "message/msg-02/read"]).fields["value"] as? Bool == true, "msg-02 is read in the fixture")
        }

        @Test func dotAndSlashSeparateAlike() throws {
            let a = try Ctl.ok("core", ["state", "message/msg-03/account"]).fields["value"] as? String
            let b = try Ctl.ok("core", ["state", "message.msg-03.account"]).fields["value"] as? String
            #expect(a == "acc-18" && b == a)
        }

        @Test func anArchiveIsVisibleInTheStoreAndUndoRevertsIt() throws {
            try Ctl.ok("key", ["E"])
            #expect(try Ctl.ok("core", ["state", "message/msg-01/archived"]).fields["value"] as? Bool == true)
            try Ctl.ok("key", ["Z"])
            #expect(try Ctl.ok("core", ["state", "message/msg-01/archived"]).fields["value"] as? Bool == false)
            #expect(try Ctl.ok("core", ["state", "undo_entry/consumed_last"]).fields["value"] as? Bool == true)
        }

        @Test func anUnknownPathIsNotOkAndNamesThePath() throws {
            let r = try Ctl.ask("core", ["state", "message/msg-99/archived-ish"])
            #expect(!r.ok)
            #expect(r.error == "no core value at message/msg-99/archived-ish")
            let empty = try Ctl.ask("core", ["state", ""])
            #expect(!empty.ok)
            #expect(try Ctl.ask("core", ["state", "pin/msg-01"]).ok, "positive control")
        }

        @Test func wrongShapesAreUsageNeverGuessed() throws {
            let cases: [[Any]] = [[], ["state"], ["value", "pin/msg-01"], ["state", "a", "b"]]
            for args in cases {
                let r = try Ctl.ask("core", args)
                #expect(!r.ok, "core \(args)")
                #expect(r.error == "usage: core state <path>", "core \(args): \(r.error ?? "")")
            }
        }
    }
}
