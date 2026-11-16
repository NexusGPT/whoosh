import Foundation
import Testing

extension Contract {
    /// `measure <ax-id>`: `mac-main.launch` = process start to first paint, in ms (docs/CONTROL-SOCKET.md).
    /// The doc also promises a `frame` form; no ax-id serves it at this commit (finding in docs/lanes/contracttests.md).
    @Suite("measure") struct MeasureVerb {
        @Test func launchIsAPositiveMillisecondFigureAndNothingElse() throws {
            let r = try Ctl.ok("measure", ["mac-main.launch"])
            #expect(r.keys == ["ok", "ms"])
            let ms = try #require(r.fields["ms"] as? Double)
            #expect(isJSONBool(r.fields["ms"]) == false)
            #expect(ms > 0 && ms.isFinite, "launch-to-first-paint is a real positive figure, got \(ms)")
        }

        @Test func theLaunchFigureIsSetOnceNotRemeasuredPerCall() throws {
            let a = try Ctl.ok("measure", ["mac-main.launch"]).fields["ms"] as? Double
            Thread.sleep(forTimeInterval: 0.3)
            let b = try Ctl.ok("measure", ["mac-main.launch"]).fields["ms"] as? Double
            #expect(a != nil && a == b, "first paint happened once: \(String(describing: a)) vs \(String(describing: b))")
        }

        @Test func anIdWithNoMeasurementIsNotOkAndNamesIt() throws {
            let r = try Ctl.ask("measure", ["mac-main.no-such"])
            #expect(!r.ok)
            #expect(r.error == "no measurement at mac-main.no-such yet")
            #expect(r.fields.isEmpty, "a refused measure carries no ms and no frame")
            // positive control: the same verb answers for the launch id
            #expect(try Ctl.ask("measure", ["mac-main.launch"]).ok)
        }

        @Test func noAxIdIsRefusedInWords() throws {
            let r = try Ctl.ask("measure")
            #expect(!r.ok)
            #expect(r.error == "measure needs an ax-id")
        }
    }
}
