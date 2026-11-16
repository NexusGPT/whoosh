import Foundation
#if os(macOS)
import AppKit
#endif

/// Every socket verb, in one registry written complete at the root commit (docs/CONTROL-SOCKET.md).
/// Features never add a verb: they register rows, keys and clicks (FeatureModule), which these verbs reach.
@MainActor
public enum ControlVerbs {
    typealias Handler = (AppModel, [String]) throws -> [String: Any]

    static let all: [String: Handler] = WindowVerbs.all.merging(base) { $1 }

    private static let base: [String: Handler] = [
        "type": { m, a in
            guard let text = a.first else { throw ControlFailure("type needs the text") }
            InteractionProbe.run { Registrations.type(text, m) }   // a typed letter is measured like a key (`measure mac-main.interaction.commit`)
            return [:]
        },
        "key": { m, a in
            #if os(macOS)
            if KeyCodeVerb.isKeyCodeCall(a) { return try KeyCodeVerb.press(m, a) }   // `key code:19 mods:cmd` (AZERTY digit row)
            #endif
            guard let chord = a.first else { throw ControlFailure("key needs a chord") }
            try InteractionProbe.run {
                guard m.press(chord) else { throw ControlFailure("the key \(chord) does nothing on \(m.surface)") }
            }
            return [:]
        },
        "keyevent": { m, a in
            #if os(macOS)
            // probes-r1: a real NSEvent through the monitors and the field (KeyEventVerb.swift).
            // Lead, 13 Nov (owner A162): a real key press is timed like the socket's `key`, so `measure mac-main.interaction.*` answers for the
            // path the owner's keyboard takes (monitor, field editor, text view), not only for `AppModel.press`. `reset` is not an event.
            if a == ["reset"] { return try KeyEventVerb.run(m, a) }
            return try InteractionProbe.run { try KeyEventVerb.run(m, a) }
            #else
            throw ControlFailure("keyevent needs the Mac app")
            #endif
        },
        "click": { m, a in
            guard let ax = a.first else { throw ControlFailure("click needs an ax-id") }
            guard let c = Registrations.click(ax) else { throw ControlFailure("nothing to click at \(ax)") }
            try InteractionProbe.run {
                guard c(m) else { throw ControlFailure("\(ax) did nothing") }
            }
            return [:]
        },
        "rows": { m, a in
            guard let ax = a.first else { throw ControlFailure("rows needs an ax-id") }
            #if os(macOS)
            if ax == KeyEventVerb.rowId { return ["rows": [KeyEventVerb.row()]] }
            #endif
            guard let source = Registrations.rowSource(ax) else { throw ControlFailure("no rows at \(ax)") }
            return ["rows": try source(m)]
        },
        "uistate": { m, _ in
            var counts: [String: Any] = [:]
            for t in m.counts.tabs { counts[t.tabId] = Int(t.unread) }
            for (k, v) in m.counts.mailboxes where counts[k] == nil { counts[k] = Int(v) }
            var waiting: [String: Any] = [:]
            for t in m.counts.tabs { waiting[t.tabId] = Int(t.waiting) }
            let state: [String: Any] = [
                "surface": m.surface, "tab": m.tab, "counts": counts, "waiting": waiting,
                "focus_message": m.focusedMessageId ?? NSNull(), "focus_index": m.focusIndex,
                "overlay": m.overlay ?? NSNull(), "first_responder": FirstResponder.id(), "settings_page": m.settingsPage.id, "show_emblems": AppModel.showEmblems, "theme": WindowControl.theme, "account_filter": m.accountFilter ?? NSNull(),
                "empty_state": Registrations.emptyState(m) ?? NSNull(), "core": m.host.kind.rawValue, "hidden": RigHidden.state,
                "last_failure": m.lastFailure.map { ["account_name": $0.accountName, "reason_words": $0.reasonWords, "retryable": $0.retryable] } ?? NSNull(),
            ]
            return ["state": state]
        },
        "core": { m, a in
            if a.first == "run" {   // `core run search <words...>`: Command::Search as given (also no words); the engine's refusal comes back in its words
                guard a.count >= 2, a[1] == "search" else { throw ControlFailure("usage: core run search <words...>") }
                let words = a.dropFirst(2).joined(separator: " ")
                do { _ = try m.core.run(command: .search(query: words)) }
                catch CoreError.Invalid(_, let w) { throw ControlFailure(w) }
                catch CoreError.Unexpected(let w) { throw ControlFailure(w) }
                catch CoreError.Action(let f) { throw ControlFailure(f.reasonWords) }
                catch let c as CoreError { throw ControlFailure("\(c)") }
                return ["sent": words]
            }
            guard a.first == "state", a.count == 2 else { throw ControlFailure("usage: core state <path> or core run search <words...>") }
            if a[1].hasPrefix("system-ui/") { return try FixtureSystemUI.state(a[1]) }
            #if os(macOS)
            if a[1].hasPrefix("desktop/") {   // hid-compose: what `drag <attachment> desktop` wrote (a rig copy only)
                guard let v = AttachmentDesk.value(a[1]) else { throw ControlFailure("no core value at \(a[1])") }
                return ["value": v]
            }
            #endif
            guard let json = m.core.state(path: a[1]) else { throw ControlFailure("no core value at \(a[1])") }
            guard let v = try? JSONSerialization.jsonObject(with: Data(json.utf8), options: [.fragmentsAllowed]) else {
                throw ControlFailure("the core value at \(a[1]) is not JSON")
            }
            return ["value": v]
        },
        "capture": { m, a in
            guard a.first == "shot", a.count >= 2 else { throw ControlFailure("usage: capture shot <name> [ax-id ...]") }
            return try shot(a[1], m: m, frames: Array(a.dropFirst(2)))
        },
        "shot": { m, a in
            guard let name = a.first else { throw ControlFailure("shot needs a name or a surface id") }
            return try shot(name, m: m, frames: Array(a.dropFirst()))
        },
        "clock": { m, a in
            guard a.first == "advance", a.count == 2, let s = Double(a[1]) else { throw ControlFailure("usage: clock advance <seconds>") }
            guard m.host.kind == .fixture else { throw ControlFailure("the clock moves only under --fixture-core") }
            guard m.host.clock.advance(seconds: s) else {
                throw ControlFailure("clock advance takes a finite number of seconds up to \(Int(FixtureClock.maxAdvanceS))")
            }
            m.refresh()
            return [:]
        },
        "fixture": { m, a in try FixtureVerbs.run(m, a) },
        "verbs": { _, _ in ["verbs": all.keys.sorted()] },
    ]

    public static func handle(_ line: String) -> String {
        switch ControlRequest.parse(line) {
        case .failure(let f): return ControlReply.fail(f.words)
        case .success(let req):
            guard let h = all[req.verb] else { return ControlReply.fail("unknown verb \(req.verb)") }
            #if os(macOS)
            defer { if RigUndoBoundary.inserts(req.verb) { RigUndoBoundary.close() } }   // hid-compose round 14: only after a verb that inserted text
            #endif
            do { return ControlReply.ok(try h(AppModel.shared, req.args)) }
            catch let f as ControlFailure { return ControlReply.fail(f.words) }
            catch { return ControlReply.fail("\(error)") }
        }
    }
}

/// Starts the socket once, at the first window, when the rig set $WHOOSH_SOCKET_DIR.
@MainActor
public enum ControlBootstrap {
    private static var server: ControlServer?
    public static func startOnce() {
        guard server == nil else { return }
        let s = ControlServer { line in
            DispatchQueue.main.sync { MainActor.assumeIsolated { ControlVerbs.handle(line) } }
        }
        do { try s?.start(); server = s } catch { FileHandle.standardError.write(Data("control socket failed: \(error)\n".utf8)) }
    }
}

#if os(macOS)
/// hid-compose rounds 11 and 14 (rig only: the socket exists only on a test copy). A socket verb is not an NSEvent. The window undo manager
/// groups by event, and on the hidden copy no event ever ends its group (the keys of `keyevent` are handed to `sendEvent`, never fetched
/// from the event queue), so what the socket's `type` inserted and the keys typed after it stayed one group: one Undo took both (main95 to
/// main103). After a verb that INSERTS text from outside an event (`type`, a `fixture` paste or dictation), the open text view's typing run
/// is broken (lead, main111: only that; the group itself is never ended by hand, see `closeNow`).
/// Never after a key, a click or the Undo itself (round 11 did that and began an empty group each time: main105, the Undo took back that
/// empty group and removed nothing).
@MainActor
enum RigUndoBoundary {
    /// The verbs that put text in from outside an event.
    static func inserts(_ verb: String) -> Bool { verb == "type" || verb == "fixture" }

    /// hid-compose round 16: the boundary runs on the NEXT run-loop turn, as its own pass, never at the end of the verb's own pass. In the
    /// verb's pass SwiftUI may still write into an NSTextView that is not a field editor and so registers undo on the window's manager: the
    /// quick reply's TextEditor (the socket's `type` sets QuickReplyModel.draft and SwiftUI writes it into the editor later in that pass). An
    /// event group ended by hand there and registered on again in the same pass is what raised `must begin a group before registering undo`
    /// (main107 to main109, on a field editor). In a pass of its own the manager opens its next group by itself.
    static func close() {
        DispatchQueue.main.async { MainActor.assumeIsolated { closeNow() } }
    }

    private static func closeNow() {
        for w in NSApp.windows where w.isVisible {
            // Only a text view of the app's own (the composer, the quick reply). Never a field's editor and never the window's manager: a field
            // editor's NSCellUndoManager whose event group was ended here was left "in invalid state", and SwiftUI's next write into that
            // field ended the test copy (main107 to main109: "_registerUndoObject:: NSCellUndoManager … must begin a group before registering
            // undo", raised under PlatformTextFieldCoordinator.update after `fixture reset` with the search field focused).
            guard let text = w.firstResponder as? NSTextView, !text.isFieldEditor else { continue }
            // Only the typing run is broken. No group is ever ended by hand: the manager keeps its own mark that this pass's event group is open, and
            // with that group ended here the text view's next insert raised in `-[NSUndoManager _prepareEventGrouping]` and ended the test copy
            // (main111: par-composermac under ComposerModel.insertSnippet, par-keys under ShortcutEngine.press). The run loop ends the event group
            // itself; `breakUndoCoalescing` is what stops the text view from joining the next typed keys to what the socket inserted.
            text.breakUndoCoalescing()
        }
    }
}
#endif
