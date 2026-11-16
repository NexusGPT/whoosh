import Foundation
import Testing

extension Contract {
    /// `winframes`: the visible windows (docs/CONTROL-SOCKET.md).
    @Suite("winframes") struct WinframesVerb {
        @Test func theMainWindowIsListedWithTheFiveFieldsAndAUsableSize() throws {
            let r = try Ctl.ok("winframes")
            #expect(r.keys == ["ok", "frames"])
            let frames = try #require(r.fields["frames"] as? [[String: Any]])
            #expect(frames.count >= 1, "the app shows at least its main window")
            let win = try #require(Ctl.fixture["window"] as? [String: Any])
            let minW = try #require(win["min_w"] as? Double), minH = try #require(win["min_h"] as? Double)
            for (i, f) in frames.enumerated() {
                #expect(Set(f.keys) == ["title", "x", "y", "w", "h"], "frame \(i) keys \(f.keys.sorted())")
                #expect(f["title"] is String, "frame \(i) title is text")
                for k in ["x", "y", "w", "h"] { #expect(f[k] is NSNumber && !isJSONBool(f[k]), "frame \(i) \(k) is a number") }
            }
            // at least one window is the main window: at least the fixture's minimum size (docs/design fixture window)
            #expect(frames.contains { ($0["w"] as? Double ?? 0) >= minW && ($0["h"] as? Double ?? 0) >= minH },
                    "no window of at least \(minW)x\(minH) in \(frames)")
        }

        @Test func theFramesDoNotChangeWhileNothingMoves() throws {
            let a = try Ctl.ok("winframes").fields["frames"] as? [[String: Any]] ?? []
            let b = try Ctl.ok("winframes").fields["frames"] as? [[String: Any]] ?? []
            #expect(NSArray(array: a).isEqual(to: b))
        }

        @Test func extraArgumentsAreIgnoredNotCoerced() throws {
            let r = try Ctl.ask("winframes", ["junk", 7])
            #expect(r.ok)
            #expect(r.keys == ["ok", "frames"])
        }
    }
}
