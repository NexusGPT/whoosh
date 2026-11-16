import Foundation

#if os(macOS)
import AppKit

/// Turns a real key event into the chord text the registry uses, and says whether the owner is typing.
/// The app shell's key monitor should call `ChordKit.chord(_:)` and skip `ChordKit.isTyping` events, so a
/// letter typed in the composer or the search bar never fires a single-key command.
public enum ChordKit {
    public static func chord(_ e: NSEvent) -> String {
        let flags = e.modifierFlags.intersection([.control, .option, .shift, .command])
        var prefix = ""
        if flags.contains(.control) { prefix += "⌃" }
        if flags.contains(.option) { prefix += "⌥" }
        if flags.contains(.shift) { prefix += "⇧" }
        if flags.contains(.command) { prefix += "⌘" }
        // Named keys carry their modifiers too: ⌘Enter, ⇧⌘Enter, ⌘Up, ⇧Space (v8 binds them).
        // 51 is the delete key above Return (⌫). It had no name here, so its chord was the character it carries ("\u{7F}") and no binding
        // written "Backspace" ever met a real key press (lead, 13 Nov, walk on main117: the draft card shows "Delete ⌫", a real ⌫ deleted the
        // open MAIL through the reader's "\u{7F}" binding, and the draft stayed). One name for the key, the one the socket and the presets use.
        let named: [UInt16: String] = [36: "Enter", 76: "Enter", 53: "Escape", 125: "Down", 126: "Up", 123: "Left", 124: "Right", 49: "Space", 51: "Backspace"]
        if let name = named[e.keyCode] { return prefix + name }
        if e.keyCode == 48 {
            if flags == [.shift] { return "Shift+Tab" }
            return flags == [.control] ? "⌃Tab" : prefix + "Tab"
        }
        // The digit row is read by its physical key, not by the character the layout gives it: on a French AZERTY keyboard
        // the "2" key types "é" and ⌘2 never matched (owner, 2026-11-09 23:25: "cmd+2, cmd+3, etc. don't work on my azerty
        // keyboard"). Key codes 18 to 29 are the digit row on every layout; a Shift-only chord keeps the typed character.
        let digitRow: [UInt16: String] = [18: "1", 19: "2", 20: "3", 21: "4", 23: "5", 22: "6", 26: "7", 28: "8", 25: "9", 29: "0"]
        if flags != [.shift], !flags.isEmpty, let digit = digitRow[e.keyCode] { return prefix + digit }
        let ch = flags == [.shift] ? (e.characters ?? "") : unshifted(e, flags)
        return prefix + ch.uppercased()
    }

    /// The key's own character for a chord that holds Shift WITH another modifier. `charactersIgnoringModifiers` ignores every modifier
    /// except Shift, so a real ⇧⌘ and the comma key arrives as "<" (as "?" on an AZERTY keyboard) and the preset's "⇧⌘," (Discard draft)
    /// never matched a real key press; only the test key verb, which writes the unshifted character itself, ever fired it (lead, 13 Nov,
    /// owner A162). A letter is the same key either way; only a punctuation key is read again, and only when the event's character IS what
    /// Shift gives on that key in the current layout (so an event that already carries the unshifted character is left as it is).
    private static func unshifted(_ e: NSEvent, _ flags: NSEvent.ModifierFlags) -> String {
        let plain = e.charactersIgnoringModifiers ?? ""
        guard flags.contains(.shift), flags != [.shift], plain.count == 1, let c = plain.first, !c.isLetter, !c.isNumber,
              e.characters(byApplyingModifiers: [.shift]) == plain,
              let base = e.characters(byApplyingModifiers: []), base.count == 1, base != plain else { return plain }
        return base
    }

    /// The text view or text field that holds the focus of the EVENT'S window (not the key window: a Settings field while the main
    /// window is key, a sheet, the hidden rig's window), nil when the focus is on anything else.
    @MainActor public static func typingView(_ e: NSEvent) -> NSText? {
        (e.window ?? (NSApp as NSApplication?)?.keyWindow)?.firstResponder as? NSText
    }

    /// True while a text field or text view owns the keyboard of the event's window.
    @MainActor public static func isTyping(_ e: NSEvent) -> Bool { typingView(e) != nil }

    /// The shell's monitor calls this: nil when the engine took the key, the event otherwise. A text view in focus is offered the
    /// key first when `TextKeys.owns` says so (the table is there) or when it lists the chord itself (`ChordOwningTextView`).
    /// A text that was given the keyboard and lost it to no act of the owner's takes it back before a typed key is read as a command
    /// (the quick reply: `ReplyKeyboard.reclaim`). 0: not its key; 1: deliver this event (its text is the first responder again); 2: it took the key.
    @MainActor public static var reclaimTyping: (@MainActor (NSEvent) -> Int)?

    /// A picker over the list (snooze, label, move) is for typing: while one is up, a typed key that finds no text holding the keyboard is given to
    /// the picker, never read as a list command. Each picker's owner adds its hook (TriagePick.installOnce, LabelMoveState.installOnce). A hook
    /// answers 0: its picker is not up; 1: deliver this event (the picker's field is the first responder now); 2: the picker took the key itself (its
    /// field is not in the window yet).
    @MainActor public static var pickerTyping: [@MainActor (NSEvent) -> Int] = []

    /// The overlays that are for typing (the snooze, label and move pickers; each owner adds its own beside its hook). The net under the hooks:
    /// while one is up, a typed key that no text and no hook took ends here. It is never read as a command on the mail under the picker.
    @MainActor public static var typingOverlays: Set<String> = []

    /// The last keys `handle` saw, newest last: the chord, whether a text held the keyboard, the branch that took the key and what each picker
    /// hook said. `rows mac-keyevent.monitor` shows it (field `trace`), so a key that ran the wrong thing names the branch that let it through.
    @MainActor public private(set) static var trace: [String] = []
    @MainActor private static var hookNotes: [String] = []
    /// A picker hook's word on the key in hand ("triage-field-ok", "move-not-up"): one token, no spaces.
    @MainActor public static func note(_ token: String) { hookNotes.append(token) }
    @MainActor private static func leave(_ c: String, _ text: Bool, _ branch: String) {
        trace.append("\(c)|\(text ? "text" : "no-text")|\(branch)|\(hookNotes.joined(separator: "+"))")
        if trace.count > 16 { trace.removeFirst(trace.count - 16) }
    }

    /// The composer's own word on which header field holds the cursor (its ring: To on a new message or a forward): that field as AppKit has it,
    /// nil when the ring is on the message. The composer sets it (ComposerKeyboard).
    @MainActor public static var composerField: (@MainActor (NSWindow) -> NSView?)?

    /// The message text of a composer window (its rich text view); nil for every other window.
    @MainActor static func writingText(in w: NSWindow) -> NSText? {
        guard WindowBridge.id(of: w) == "mac-composer" else { return nil }
        func find(_ v: NSView) -> NSText? {
            if let t = v as? WhooshTextView { return t }
            for s in v.subviews { if let t = find(s) { return t } }
            return nil
        }
        return w.contentView.flatMap(find)
    }

    /// A control that records a key (the notch's key in Settings) takes the next key press before anything reads it as a command: true when
    /// it took the key. Set while it listens, nil otherwise.
    @MainActor public static var capture: (@MainActor (NSEvent) -> Bool)?

    @MainActor public static func handle(_ e: NSEvent, model: AppModel = .shared) -> NSEvent? {
        Registrations.primeKeyTables()
        if let capture, capture(e) { leave(chord(e), false, "capture"); return nil }
        let c = chord(e)
        let text = typingView(e) != nil
        hookNotes = []
        if typingView(e) == nil, TextKeys.owns(c), let took = reclaimTyping?(e), took != 0 { leave(c, text, "reclaim-\(took)"); return took == 1 ? e : nil }
        if typingView(e) == nil, TextKeys.owns(c) {
            for hook in pickerTyping { let took = hook(e); if took != 0 { leave(c, text, "picker-\(took)"); return took == 1 ? e : nil } }
        }
        // The composer window is for writing: a typed key that finds no text holding the keyboard there is given to the message text and typed in it. It is never
        // read as a mail command of the window behind (lead, 13 Nov 2026, walk compret-142: the letters of "Line one." ran as shortcuts on the open mail and
        // Return closed the composer).
        // The field is the one the composer's ring is on: a letter typed right after F (forward) is the recipient's, it never joins the end of the forwarded
        // text (lead, 13 Nov 2026, probe on main155: "an" typed after F ended the quoted mail while the ring was on To).
        if typingView(e) == nil, TextKeys.owns(c), let w = e.window ?? (NSApp as NSApplication?)?.keyWindow, WindowBridge.id(of: w) == "mac-composer",
           let field = composerField?(w), w.makeFirstResponder(field) {
            if let editor = w.firstResponder as? NSText { editor.selectedRange = NSRange(location: (editor.string as NSString).length, length: 0) }
            leave(c, text, "composer-field"); return e
        }
        if typingView(e) == nil, TextKeys.owns(c), let w = e.window ?? (NSApp as NSApplication?)?.keyWindow, let body = writingText(in: w), w.makeFirstResponder(body) { leave(c, text, "composer"); return e }
        if let tv = typingView(e), TextKeys.owns(c) || (tv as? ChordOwningTextView)?.ownsChord(c) == true { leave(c, text, "text"); return e }
        if TextKeys.owns(c), let up = model.overlay, typingOverlays.contains(up) { leave(c, text, "typing-overlay-net"); return nil }
        let pressed = model.press(c)
        leave(c, text, pressed ? "press" : "unbound")
        return pressed ? nil : e
    }
}

/// A text view that handles chords the engine also binds (an editor that saves on ⌘S, one that takes ⌘Enter) lists them here:
/// the shell's monitor then leaves those keys to the view while it holds the focus.
/// Main-actor bound: its only conformers are AppKit views, and its only caller is `handle` above.
@MainActor public protocol ChordOwningTextView: AnyObject { func ownsChord(_ chord: String) -> Bool }
#endif
