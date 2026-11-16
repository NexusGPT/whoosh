import Foundation
import Testing

extension Contract {
    /// `uistate`: the screen state in twelve fields (docs/CONTROL-SOCKET.md), and the deltas 1 and 2.
    @Suite("uistate") struct UistateVerb {
        static let fields: Set<String> = ["surface", "tab", "counts", "waiting", "focus_message", "focus_index", "overlay", "theme",
                                          "account_filter", "empty_state", "core", "last_failure"]

        @Test func replyIsOkAndStateAloneWithExactlyTheTwelveFields() throws {
            let r = try Ctl.ok("uistate")
            #expect(r.keys == ["ok", "state"])
            let s = try #require(r.fields["state"] as? [String: Any])
            #expect(Set(s.keys) == Self.fields, "got \(Set(s.keys).symmetricDifference(Self.fields).sorted()) off")
        }

        @Test func atLaunchTheStateIsThePrimaryTabOnTheFixtureCore() throws {
            let s = try Ctl.state()
            #expect(s["surface"] as? String == "mac-main")
            #expect(s["tab"] as? String == "primary")
            #expect(s["focus_index"] as? Int == 0)
            #expect(s["focus_message"] as? String == Ctl.primaryIds[0])
            #expect(s["overlay"] is NSNull)
            #expect(s["theme"] as? String == "light")
            #expect(s["account_filter"] is NSNull)
            #expect(s["empty_state"] is NSNull, "delta 2: null until the InboxList lane serves it")
            #expect(s["core"] as? String == "fixture", "--fixture-core was set")
            #expect(s["last_failure"] is NSNull)
        }

        @Test func countsHoldEveryTabUnreadAndEveryMailboxLowercased() throws {
            let c = try #require(try Ctl.state()["counts"] as? [String: Any])
            for t in Ctl.tabs {
                let id = t["id"] as! String
                #expect(c[id] as? Int == Ctl.tabUnread(id), "tab \(id) unread")
            }
            for b in Ctl.mailboxes {
                let name = (b["name"] as! String).lowercased()
                guard let n = b["count"] as? Int else {
                    #expect(c[name] == nil, "\(name) has no count in the fixture, so no key")
                    continue
                }
                if Ctl.tabs.contains(where: { $0["id"] as? String == name }) {
                    // delta 1: a tab and a mailbox with one lowercase name report the tab's unread
                    #expect(c[name] as? Int == Ctl.tabUnread(name), "\(name) is the tab's unread")
                } else {
                    #expect(c[name] as? Int == n, "mailbox \(name)")
                }
            }
            #expect(c["snoozed"] as? Int == 7, "delta 1: snoozed is lowercase and counted")
            #expect(c["all inboxes"] as? Int == 198)
            #expect(c["sent"] == nil, "positive control above: drafts is present while sent (no count) is absent")
            #expect(c["drafts"] as? Int == 3)
        }

        @Test func waitingMapsEveryTabToItsWaitingFigureAndNothingElse() throws {
            let w = try #require(try Ctl.state()["waiting"] as? [String: Any])
            #expect(Set(w.keys) == Set(Ctl.tabs.map { $0["id"] as! String }))
            for t in Ctl.tabs { #expect(w[t["id"] as! String] as? Int == t["waiting"] as? Int, "waiting \(t["id"]!)") }
            #expect(w["primary"] as? Int == 4)
        }

        @Test func countsAndFocusFollowAnArchiveAndItsUndo() throws {
            let base = try Ctl.unread("primary")
            try Ctl.ok("key", ["E"])
            let s = try Ctl.state()
            #expect((s["counts"] as? [String: Any])?["primary"] as? Int == base - 1)
            #expect(s["focus_message"] as? String == Ctl.primaryIds[1], "focus stays on the index, so the next row is focused")
            try Ctl.ok("key", ["Z"])
            #expect(try Ctl.unread("primary") == base)
        }

        @Test func tabFollowsTabKey() throws {
            try Ctl.ok("key", ["Tab"])
            defer { _ = try? Ctl.ask("key", ["Shift+Tab"]) }
            #expect(try Ctl.state()["tab"] as? String == Ctl.tabs[1]["id"] as? String)
            // no fixture message sits in the second tab: its list is empty, so nothing is focused
            #expect(try Ctl.state()["focus_message"] is NSNull)
            #expect(try Ctl.state()["focus_index"] as? Int == 0)
        }
    }
}
