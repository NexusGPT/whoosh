import Foundation
import Testing

extension Contract {
    /// `clock advance <seconds>`: moves the fixture core's clock; `advance_to` is not served (delta 7).
    /// The clock has no read-back verb and no base key reads it: the effect is UNOBSERVABLE on the socket at this
    /// commit (finding). What is tested is the wire contract, the usage refusals and that the app stays up and unchanged.
    @Suite("clock") struct ClockVerb {
        @Test func advanceIsOkWithNoFieldsAndLeavesTheScreenAlone() throws {
            let before = try Ctl.state()
            let r = try Ctl.ask("clock", ["advance", 60])
            #expect(r.ok, "\(r.error ?? "")")
            #expect(r.keys == ["ok"])
            #expect(NSDictionary(dictionary: try Ctl.state()).isEqual(to: before), "a clock move changes no count and no row at this commit")
        }

        @Test func secondsMayBeTextOrANumberOrFractional() throws {
            #expect(try Ctl.ask("clock", ["advance", "1.5"]).ok)
            #expect(try Ctl.ask("clock", ["advance", 0]).ok)
            #expect(try Ctl.ask("clock", ["advance", 0.25]).ok)
        }

        @Test func advanceToIsNotServedAndSaysUsage() throws {
            let r = try Ctl.ask("clock", ["advance_to", "tomorrow", "8:00"])
            #expect(!r.ok)
            #expect(r.error == "usage: clock advance <seconds>")
        }

        /// docs/CAPABILITY-CONTRACT.md planned `clock set <iso>`; the live verb is `clock advance <seconds>` only (finding).
        @Test func theCapabilityContractsPlannedSetFormIsNotServed() throws {
            let r = try Ctl.ask("clock", ["set", "2026-11-08T08:00:00Z"])
            #expect(!r.ok)
            #expect(r.error == "usage: clock advance <seconds>")
        }

        @Test func malformedArgumentsAreUsageNeverACoercedNumber() throws {
            let cases: [[Any]] = [[], ["advance"], ["advance", "soon"], ["advance", ""], ["advance", "1", "2"], ["rewind", 5], ["ADVANCE", 5]]
            for args in cases {
                let r = try Ctl.ask("clock", args)
                #expect(!r.ok, "clock \(args) must be refused")
                #expect(r.error == "usage: clock advance <seconds>", "clock \(args): \(r.error ?? "")")
            }
            #expect(try Ctl.ask("clock", ["advance", 1]).ok, "positive control: the verb works")
        }
    }
}
