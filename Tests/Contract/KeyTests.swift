import Foundation
import Testing

extension Contract {
    /// `key <chord>`: the same path as the real keyboard (docs/CONTROL-SOCKET.md). Every test leaves the app as it found it.
    @Suite("key") struct KeyVerb {
        @Test func jMovesFocusDownAndKBack() throws {
            let before = try Ctl.state()
            #expect(before["focus_index"] as? Int == 0)
            #expect(before["focus_message"] as? String == Ctl.primaryIds[0])
            let r = try Ctl.ok("key", ["J"])
            #expect(r.keys == ["ok"], "key has no reply fields")
            let down = try Ctl.state()
            #expect(down["focus_index"] as? Int == 1)
            #expect(down["focus_message"] as? String == Ctl.primaryIds[1])
            try Ctl.ok("key", ["K"])
            #expect(try Ctl.state()["focus_index"] as? Int == 0)
        }

        @Test func aSingleLetterIsCaseBlind() throws {
            try Ctl.ok("key", ["j"])
            #expect(try Ctl.state()["focus_index"] as? Int == 1, "lower-case j is J")
            try Ctl.ok("key", ["k"])
            #expect(try Ctl.state()["focus_index"] as? Int == 0)
        }

        @Test func focusStopsAtBothEnds() throws {
            let last = Ctl.primaryIds.count - 1
            for _ in 0..<(last + 10) { try Ctl.ok("key", ["J"]) }
            #expect(try Ctl.state()["focus_index"] as? Int == last)
            #expect(try Ctl.state()["focus_message"] as? String == Ctl.primaryIds[last])
            for _ in 0..<(last + 10) { try Ctl.ok("key", ["K"]) }
            #expect(try Ctl.state()["focus_index"] as? Int == 0)
        }

        @Test func eArchivesTheFocusedRowAndZBringsItBack() throws {
            let first = Ctl.primaryIds[0]
            let unreadBefore = Ctl.tabUnread("primary")
            #expect(try Ctl.unread("primary") == unreadBefore)
            try Ctl.ok("key", ["E"])
            // msg-01 is unread in the fixture: one row and one unread leave
            #expect(Ctl.message(first)["unread"] as? Bool == true, "fixture precondition")
            #expect(try Ctl.unread("primary") == unreadBefore - 1)
            let after = try Ctl.rows("mac-main.list").map { $0["id"] as? String }
            #expect(after == Array(Ctl.primaryIds.dropFirst()).map { Optional($0) })
            try Ctl.ok("key", ["Z"])
            #expect(try Ctl.unread("primary") == unreadBefore)
            #expect(try Ctl.rows("mac-main.list").map { $0["id"] as? String } == Ctl.primaryIds.map { Optional($0) })
        }

        @Test func tabAndShiftTabWalkTheSplitTabsAndWrap() throws {
            let ids = Ctl.tabs.map { $0["id"] as! String }
            for i in 1..<ids.count {
                try Ctl.ok("key", ["Tab"])
                #expect(try Ctl.state()["tab"] as? String == ids[i], "Tab \(i)")
            }
            try Ctl.ok("key", ["Tab"])
            #expect(try Ctl.state()["tab"] as? String == ids[0], "Tab wraps to the first tab")
            try Ctl.ok("key", ["Shift+Tab"])
            #expect(try Ctl.state()["tab"] as? String == ids[ids.count - 1], "Shift+Tab wraps to the last tab")
            try Ctl.ok("key", ["Shift+Tab"])
            #expect(try Ctl.state()["tab"] as? String == ids[ids.count - 2])
            // back to Primary
            for _ in 0..<(ids.count - 2 + ids.count) where (try Ctl.state()["tab"] as? String) != "primary" { try Ctl.ok("key", ["Tab"]) }
            #expect(try Ctl.state()["tab"] as? String == "primary")
            #expect(try Ctl.state()["focus_index"] as? Int == 0, "a tab switch focuses row 0")
        }

        @Test func aChordBoundToNothingIsNotOkAndSaysSo() throws {
            let before = try Ctl.state()
            let r = try Ctl.ask("key", ["Hyper+Q9"])
            #expect(!r.ok)
            #expect(r.error == "the key Hyper+Q9 does nothing on mac-main")
            #expect(r.keys == ["ok", "error"])
            // positive control: a bound chord on the same connection path is ok
            try Ctl.ok("key", ["J"]); try Ctl.ok("key", ["K"])
            #expect(NSDictionary(dictionary: try Ctl.state()).isEqual(to: before), "a refused key changes nothing")
        }

        @Test func noChordIsRefusedInWords() throws {
            let r = try Ctl.ask("key")
            #expect(!r.ok)
            #expect(r.error == "key needs a chord")
        }

        @Test func aKeyThatFailsAtTheCoreIsNotOkAndKeepsTheReasonInUiState() throws {
            // msg-03 sits on the failing account (acc-18): archive is refused in words.
            let idx = try #require(Ctl.primaryIds.firstIndex(of: "msg-03"))
            for _ in 0..<idx { try Ctl.ok("key", ["J"]) }
            defer { for _ in 0..<idx { _ = try? Ctl.ask("key", ["K"]) } }
            let r = try Ctl.ask("key", ["E"])
            #expect(!r.ok, "archive on the failing account is not ok")
            let f = try #require(try Ctl.state()["last_failure"] as? [String: Any])
            #expect(f["account_name"] as? String == Ctl.accountName("acc-18"))
            #expect(f["reason_words"] as? String == "needs you to sign in again")
            #expect(isJSONBool(f["retryable"]) && f["retryable"] as? Bool == true)
            #expect(Set(f.keys) == ["account_name", "reason_words", "retryable"])
            // a later success clears it: pin then unpin the same row
            try Ctl.ok("key", ["P"])
            #expect(try Ctl.state()["last_failure"] is NSNull, "a success clears last_failure")
            try Ctl.ok("key", ["P"])
        }

        @Test func escapeReturnsToTheMainSurfaceWithNoOverlay() throws {
            try Ctl.ok("key", ["Escape"])
            let s = try Ctl.state()
            #expect(s["surface"] as? String == "mac-main")
            #expect(s["overlay"] is NSNull)
        }
    }
}
