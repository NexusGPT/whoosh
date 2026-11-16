import Foundation
import Testing

extension Contract {
    /// `fixture <name> ...`: reserved for the fixture events the journeys name; `ok:false` at this commit (delta 7).
    @Suite("fixture") struct FixtureVerb {
        @Test func everyReservedEventAnswersNotServedAndNamesItself() throws {
            let before = try Ctl.state()
            for name in ["deliver", "speak", "paste", "load"]   /* `load` = the capability contract's planned `fixture load <name>` */ {
                let r = try Ctl.ask("fixture", [name, "x"])
                #expect(!r.ok, "fixture \(name) is reserved: ok:false")
                #expect(r.error == "fixture \(name) is not served at this commit", "\(name): \(r.error ?? "")")
                #expect(r.keys == ["ok", "error"])
            }
            // reserved means NO effect: the screen state is the one it was
            #expect(NSDictionary(dictionary: try Ctl.state()).isEqual(to: before))
        }

        @Test func theVerbIsRegisteredNotUnknown() throws {
            // a renamed verb would answer "unknown verb fixture"; a reserved one answers in its own words
            let r = try Ctl.ask("fixture", ["deliver"])
            #expect(r.error?.hasPrefix("unknown verb") == false)
            #expect(try Ctl.ask("verbs").ok, "positive control")
        }

        @Test func noNameStillAnswersReservedNotACrash() throws {
            let r = try Ctl.ask("fixture")
            #expect(!r.ok)
            #expect(r.error?.contains("is not served at this commit") == true)
            #expect(try Ctl.ask("verbs").ok, "the app still answers")
        }
    }
}
