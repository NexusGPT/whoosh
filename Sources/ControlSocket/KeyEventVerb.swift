import Foundation

#if os(macOS)
import AppKit

/// `keyevent [arm:a|b|c|d] <chord|text>` (probes-r1, round 5, plan R6): a REAL key press. The socket's `key` verb calls
/// `AppModel.press`, which never meets the keyboard monitor or the text field (the owner's A113: "when pressing cmd+k enter is
/// to validate the search"). This verb builds NSEvents (keyDown then keyUp: characters, key code, modifiers, the window's number)
/// for the window that holds the focused text view (else the window of the current surface) and hands them on. The arm picks the route, so one hidden launch can run experiment E1:
///   a  NSApp.sendEvent(event)                         the local monitors see it, then the window (default)
///   b  window.sendEvent(event)                        bypasses the monitors: the counter must stay 0 (proves it can fail)
///   c  NSApp.sendEvent(event) with the window not key  `resignKey()` first (the hidden rig never activates the app)
///   d  NSApp.sendEvent(event) after `makeKey()`        the fallback if a passes only on a key window
/// `<chord>`: modifiers as ⌃ ⌥ ⇧ ⌘ or ctrl+ alt+ shift+ cmd+, then Return/Enter, Escape, Tab, Space, Delete, Up, Down, Left, Right
/// or one character. Any other text is typed one character at a time. `keyevent reset` zeroes the counters.
/// The counter row is `rows mac-keyevent.monitor`: `seen` = keyDown events a local monitor installed by this verb saw, `sent` = keyDown
/// events handed on. The monitor is installed on the first use, after the shell's own (apps/mac KeyMonitor), so it is called first when
/// AppKit calls the newest monitor first; arm b reading 0 and arm a reading `sent` is the proof that holds either way.
@MainActor
public enum KeyEventVerb {
    public static let rowId = "mac-keyevent.monitor"
    private static var seen = 0, sent = 0
    private static var lastArm = "-"
    private static var monitor: Any?

    static let codes: [Character: UInt16] = [
        "a": 0, "s": 1, "d": 2, "f": 3, "h": 4, "g": 5, "z": 6, "x": 7, "c": 8, "v": 9, "b": 11, "q": 12, "w": 13, "e": 14, "r": 15, "y": 16, "t": 17,
        "1": 18, "2": 19, "3": 20, "4": 21, "6": 22, "5": 23, "=": 24, "9": 25, "7": 26, "-": 27, "8": 28, "0": 29, "]": 30, "o": 31, "u": 32, "[": 33,
        "i": 34, "p": 35, "l": 37, "j": 38, "'": 39, "k": 40, ";": 41, "\\": 42, ",": 43, "/": 44, "n": 45, "m": 46, ".": 47, "`": 50]
    static let named: [String: (code: UInt16, chars: String)] = [
        "return": (36, "\r"), "enter": (36, "\r"), "escape": (53, "\u{1b}"), "esc": (53, "\u{1b}"), "tab": (48, "\t"), "space": (49, " "),
        "delete": (51, "\u{7f}"), "backspace": (51, "\u{7f}"), "up": (126, "\u{F700}"), "down": (125, "\u{F701}"), "left": (123, "\u{F702}"), "right": (124, "\u{F703}")]

    /// The last reply, kept for `rows mac-keyevent.monitor` (field `last`): a driver that does not log the verb's reply can read it after a red.
    private static var last: [String: Any] = [:]
    public static func row() -> ControlRow { ["ax": rowId, "seen": seen, "sent": sent, "arm": lastArm, "last": last, "taker": Registrations.lastKeyTaker, "trace": ChordKit.trace, "key_returned": WindowKeyReturn.returned] }

    /// Every visible window with what its focus is (round 9, main77: a letter did not reach two editors; the reply must say which windows
    /// held a text focus and whether that focus is really in the window's views).
    private static func candidates() -> [[String: Any]] {
        NSApp.windows.filter(\.isVisible).map { w in
            let r = w.firstResponder
            return ["id": WindowBridge.id(of: w), "title": w.title, "key": w.isKeyWindow, "kind": WindowBridge.kind(w),
                    "focus": r.map { String(describing: type(of: $0)) } ?? "none", "text": r is NSText,
                    "in_window": ((r as? NSView)?.window === w), "number": w.windowNumber]
        }
    }

    private static func focusText(_ w: NSWindow) -> String? { (w.firstResponder as? NSText)?.string }

    struct Key { let code: UInt16; let chars: String; let plain: String; let flags: NSEvent.ModifierFlags }

    /// One chord or one character -> the key it presses, or nil when the text is not a chord (then it is typed).
    static func parse(_ text: String) -> Key? {
        var rest = Substring(text), flags: NSEvent.ModifierFlags = []
        let words: [(String, NSEvent.ModifierFlags)] = [("ctrl+", .control), ("alt+", .option), ("opt+", .option), ("shift+", .shift), ("cmd+", .command)]
        var again = true
        while again {
            again = false
            if let f = rest.first, let m = ["⌃": NSEvent.ModifierFlags.control, "⌥": .option, "⇧": .shift, "⌘": .command][f] { flags.insert(m); rest = rest.dropFirst(); again = true; continue }
            for (w, m) in words where rest.lowercased().hasPrefix(w) { flags.insert(m); rest = rest.dropFirst(w.count); again = true; break }
        }
        let name = rest.lowercased()
        if let n = named[name] { return Key(code: n.code, chars: n.chars, plain: n.chars, flags: flags) }
        guard rest.count == 1, let ch = name.first, let code = codes[ch] else { return nil }
        let shifted = flags.contains(.shift) ? String(rest).uppercased() : name
        return Key(code: code, chars: flags.contains(.command) || flags.contains(.control) ? name : shifted, plain: name, flags: flags)
    }

    static func keys(_ text: String) throws -> [Key] {
        if let k = parse(text) { return [k] }
        let out = text.compactMap { c -> Key? in
            if c == " " { return named["space"].map { Key(code: $0.code, chars: $0.chars, plain: $0.chars, flags: []) } }
            guard let code = codes[Character(c.lowercased())] else { return nil }
            return Key(code: code, chars: String(c), plain: c.lowercased(), flags: c.isUppercase ? .shift : [])
        }
        guard out.count == text.count, !text.isEmpty else { throw ControlFailure("keyevent: \"\(text)\" is neither a chord nor text of letters, digits and basic punctuation") }
        return out
    }

    static func install() {
        guard monitor == nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { e in
            // the local monitor runs on the main thread; the counter takes no event, it only counts
            MainActor.assumeIsolated { seen += 1 }
            return e
        }
    }

    /// A window as the routing sees it: plain data, so the choice is one function (`pick`) that a selftest mutant can be named after.
    struct Seat { let index: Int; let isKey: Bool; let isSurface: Bool; let isSettings: Bool; let holdsText: Bool }

    /// The window an event is built for (round 8, lead's main76 probe: every key was built for the window of the current surface, the
    /// MAIN window, while the editor lives in the Settings window): the window that holds the focused text view, the key window first,
    /// then the window of the current surface (so a check that worked on it keeps its window when another window also holds a text focus),
    /// then the Settings window, then any other. nil when no window has a text focus (the caller falls back to the surface's window).
    static func pick(_ seats: [Seat]) -> Int? {
        if let k = seats.first(where: { $0.isKey && $0.holdsText }) { return k.index }
        if let f = seats.first(where: { $0.isSurface && $0.holdsText }) { return f.index }
        if let s = seats.first(where: { $0.isSettings && $0.holdsText }) { return s.index }
        return seats.first(where: \.holdsText)?.index
    }

    /// The visible window that holds a text focus (a text view, or a field's editor), or nil.
    private static func focusWindow(surface: NSWindow?) -> NSWindow? {
        let ws = NSApp.windows.filter { $0.isVisible && WindowBridge.kind($0) == "window" }
        let seats = ws.enumerated().map { i, w in
            Seat(index: i, isKey: w.isKeyWindow, isSurface: w === surface, isSettings: (w.identifier?.rawValue ?? "").hasPrefix(WindowBridge.settingsWindowId), holdsText: w.firstResponder is NSText)
        }
        return pick(seats).map { ws[$0] }
    }

    /// The main menu's item for a chord (an Edit item such as Undo or Select All), searched by key equivalent and modifiers.
    private static func menuItem(_ menu: NSMenu, _ k: Key) -> NSMenuItem? {
        let mask = NSEvent.ModifierFlags.deviceIndependentFlagsMask.intersection([.command, .shift, .option, .control])
        for i in menu.items {
            if let sub = i.submenu, let found = menuItem(sub, k) { return found }
            if i.action != nil, !i.keyEquivalent.isEmpty, i.keyEquivalent.lowercased() == k.plain,
               i.keyEquivalentModifierMask.intersection(mask) == k.flags.intersection(mask) { return i }
        }
        return nil
    }

    /// The menu's own step of AppKit's order, for a window that is not key: the item's action goes up the responder chain of `w`, starting at its
    /// first responder (`NSApp.sendAction` would start at the key window, which the hidden rig does not have).
    private static func menuAction(_ e: NSEvent, _ k: Key, on w: NSWindow) -> Bool {
        guard let menu = NSApp.mainMenu, let item = menuItem(menu, k), let action = item.action else { return false }
        if let t = item.target { return NSApp.sendAction(action, to: t, from: item) }
        var r: NSResponder? = w.firstResponder
        while let x = r {
            if x.tryToPerform(action, with: item) { return true }
            r = x.nextResponder
        }
        return w.tryToPerform(action, with: item)
    }

    /// ⌘Z and ⇧⌘Z on a window that is not key (main79: the menu step said handled and the text stayed). A menu item whose action no responder
    /// of the chain answers, or one that answers without undoing, leaves the text as it was. When the key did not change the focused text, the undo
    /// manager the text view uses (its own, else the window's: what `undo:` ends in on a key window) is asked directly. Only the test door does
    /// this; a real key press never meets this verb. Returns what happened, in a word, for the reply.
    private static func undoStep(_ k: Key, on w: NSWindow, textBefore: String?, menuHandled: Bool, route: inout String, handled: inout Bool) -> String {
        guard k.plain == "z", k.flags.contains(.command), !k.flags.contains(.option), !k.flags.contains(.control) else { return "not an undo key" }
        let redo = k.flags.contains(.shift)
        if focusText(w) != textBefore { return menuHandled ? "menu" : "window" }
        guard let um = w.firstResponder?.undoManager ?? w.undoManager else { return "no undo manager" }
        guard redo ? um.canRedo : um.canUndo else { return menuHandled ? "menu said handled, text unchanged, nothing to \(redo ? "redo" : "undo")" : "nothing to \(redo ? "redo" : "undo")" }
        guard um.groupingLevel <= 1 else { return "an undo group is open (level \(um.groupingLevel))" }
        if redo { um.redo() } else { um.undo() }
        // The door runs the undo manager itself, so nothing told the text view's delegate: tell it, as a real ⌘Z's text operation does (main92: text_after 0 while the composer's body row still read the typed letter).
        (w.firstResponder as? NSTextView)?.didChangeText()
        route = "keyEquivalent+undoManager"; handled = true
        return "undo manager"
    }

    /// True when the focused text view takes this chord before the engine does: the rule the shell's monitor applies (`ChordKit.handle`).
    private static func textTakes(_ e: NSEvent) -> Bool {
        guard let tv = ChordKit.typingView(e) else { return false }
        let c = ChordKit.chord(e)
        return TextKeys.owns(c) || (tv as? ChordOwningTextView)?.ownsChord(c) == true
    }

    public static func run(_ m: AppModel, _ args: [String]) throws -> [String: Any] {
        if args == ["reset"] { seen = 0; sent = 0; return ["seen": 0, "sent": 0] }
        var arm = "a", rest = args
        if let f = rest.first, f.hasPrefix("arm:") { arm = String(f.dropFirst(4)); rest.removeFirst() }
        guard ["a", "b", "c", "d"].contains(arm) else { throw ControlFailure("keyevent: the arm is a, b, c or d") }
        guard !rest.isEmpty else { throw ControlFailure("usage: keyevent [arm:a|b|c|d] <chord|text> | keyevent reset") }
        let list = try keys(rest.joined(separator: " "))
        let surfaceWindow = WindowBridge.window(for: SurfaceSlot(m.surface))
        let byFocus = focusWindow(surface: surfaceWindow)
        guard let w = byFocus ?? surfaceWindow ?? WindowBridge.mainWindow() else { throw ControlFailure("keyevent: no window shows \(m.surface)") }
        install()
        lastArm = arm
        let keyBefore = w.isKeyWindow
        if arm == "c" { w.resignKey() }
        if arm == "d" { w.makeKey() }
        let from = seen
        var route = "sendEvent", handled = true, undoNote = "not tried"
        let cands = candidates()
        let textBefore = focusText(w)
        for k in list {
            for type in [NSEvent.EventType.keyDown, .keyUp] {
                guard let e = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: k.flags, timestamp: ProcessInfo.processInfo.systemUptime,
                                               windowNumber: w.windowNumber, context: nil, characters: k.chars, charactersIgnoringModifiers: k.plain,
                                               isARepeat: false, keyCode: k.code) else { throw ControlFailure("keyevent: the system would not build the event") }
                if type == .keyDown { sent += 1 }
                // A command key a focused text view takes, sent to a window that is not the key window (the hidden rig never makes one key):
                // AppKit's own order is the key window's `performKeyEquivalent`, then the menu, and `NSApp.sendEvent` would offer it to the
                // key window (or to no window) instead of this one. So the order is run here, on the window that holds the focus, and no
                // window is made key, shown or activated.
                if arm != "b", k.flags.contains(.command), !w.isKeyWindow, byFocus != nil, textTakes(e) {
                    route = "keyEquivalent"
                    if type == .keyDown {
                        let before = focusText(w)
                        handled = w.performKeyEquivalent(with: e) || menuAction(e, k, on: w)
                        undoNote = undoStep(k, on: w, textBefore: before, menuHandled: handled, route: &route, handled: &handled)
                    }
                    continue
                }
                // hid-ai round 8: a command key for the Settings window with no text focus. The hidden rig never makes that window key, and
                // NSApp.sendEvent then gave the chord to the main menu before any local monitor saw it (main95 sweep-settingsreplies
                // sources-add-command-n: route sendEvent, seen 0, ⌘N on Réponses › Sources asked nothing). A key window's order is run here:
                // the shell's monitor (ChordKit.handle: the panes' bindings), then the window's own key equivalents, then the menu.
                if arm == "a", k.flags.contains(.command), !w.isKeyWindow, WindowBridge.id(of: w) == WindowBridge.settingsWindowId {
                    route = "monitor+keyEquivalent"
                    if type == .keyDown {
                        seen += 1   // the key met the monitor's chain, as a key window's event does
                        handled = ChordKit.handle(e, model: m) == nil || w.performKeyEquivalent(with: e) || menuAction(e, k, on: w)
                    }
                    continue
                }
                if arm == "b" { w.sendEvent(e) } else { NSApp.sendEvent(e) }
            }
            RunLoop.current.run(until: Date().addingTimeInterval(0.03))   // the field editor and the views settle before the next key
        }
        let reply: [String: Any] = ["arm": arm, "sent": list.count, "seen": seen - from, "monitor_seen": seen, "window": w.title, "window_id": WindowBridge.id(of: w),
                "focus": byFocus?.firstResponder.map { String(describing: type(of: $0)) } ?? "none", "route": route, "handled": handled,
                "key_before": keyBefore, "key_after": w.isKeyWindow, "text_before": textBefore.map { $0.count } ?? -1,
                "text_after": focusText(w).map { $0.count } ?? -1, "undo": undoNote,
                "can_undo": (w.firstResponder?.undoManager ?? w.undoManager)?.canUndo ?? false, "windows": cands,
                "app_key_window": NSApp.keyWindow.map { WindowBridge.id(of: $0) } ?? "none"]
        last = reply
        return reply
    }
}
#endif
