import Foundation

#if os(macOS)
import AppKit

/// The `key` verb's key-code variant: `key code:19 mods:cmd` presses the physical key 19 as an AZERTY keyboard types it
/// (SPEC A99: ⌘2 must work where the "2" key types "é"). The event is a real NSEvent, so ChordKit.chord reads it exactly as
/// the shell's key monitor reads a hardware key. Arguments, in any order after the verb:
///   code:<n>               the key code (digit row: 18 19 20 21 23 22 26 28 25 29)
///   mods:cmd+shift+ctrl+opt  zero or more modifiers, joined by "+"
///   layout:azerty|qwerty   which characters the digit row types (default azerty)
///   char:<c>               the typed character, required for a key outside the digit row
/// The reply carries `chord`, the text ChordKit resolved, so a check reads the resolution as well as the outcome.
@MainActor
public enum KeyCodeVerb {
    /// What each digit-row key code types, unshifted, on a French AZERTY and on a US QWERTY keyboard.
    static let azertyRow: [UInt16: String] = [18: "&", 19: "é", 20: "\"", 21: "'", 23: "(", 22: "§", 26: "è", 28: "!", 25: "ç", 29: "à"]
    static let qwertyRow: [UInt16: String] = [18: "1", 19: "2", 20: "3", 21: "4", 23: "5", 22: "6", 26: "7", 28: "8", 25: "9", 29: "0"]
    /// With Shift held the digit row types the digit on both layouts.
    static let shiftedRow: [UInt16: String] = qwertyRow

    static func isKeyCodeCall(_ args: [String]) -> Bool { args.first?.hasPrefix("code:") == true }

    /// Builds the key-down event the arguments describe, or throws the words that name what is wrong.
    public static func event(_ args: [String]) throws -> NSEvent {
        var code: UInt16?
        var flags: NSEvent.ModifierFlags = []
        var layout = "azerty"
        var char: String?
        for a in args {
            guard let colon = a.firstIndex(of: ":") else { throw ControlFailure("key code: \"\(a)\" is not name:value") }
            let name = String(a[..<colon]), value = String(a[a.index(after: colon)...])
            switch name {
            case "code":
                guard let n = UInt16(value) else { throw ControlFailure("key code: \"\(value)\" is not a key code") }
                code = n
            case "mods":
                for w in value.split(separator: "+").map(String.init) where !w.isEmpty {
                    switch w {
                    case "cmd": flags.insert(.command)
                    case "shift": flags.insert(.shift)
                    case "ctrl": flags.insert(.control)
                    case "opt": flags.insert(.option)
                    default: throw ControlFailure("key code: the modifier \"\(w)\" is not cmd, shift, ctrl or opt")
                    }
                }
            case "layout":
                guard value == "azerty" || value == "qwerty" else { throw ControlFailure("key code: the layout \"\(value)\" is not azerty or qwerty") }
                layout = value
            case "char": char = value
            default: throw ControlFailure("key code: \"\(name)\" is not code, mods, layout or char")
            }
        }
        guard let code else { throw ControlFailure("key code needs code:<n>") }
        let row = layout == "azerty" ? azertyRow : qwertyRow
        let typed: String
        if let char { typed = char }
        else if flags.contains(.shift), let c = shiftedRow[code] { typed = c }
        else if let c = row[code] { typed = c }
        else { throw ControlFailure("key code \(code) is not on the digit row: pass char:<c>") }
        // characters = what the key types with the modifiers held; charactersIgnoringModifiers = what it types without (Shift still applies).
        guard let e = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: 0, context: nil,
                                       characters: typed, charactersIgnoringModifiers: typed, isARepeat: false, keyCode: code) else {
            throw ControlFailure("key code: the system would not build the event")
        }
        return e
    }

    /// Presses it the way the shell's monitor does (chord text, then the engine) and says which chord it resolved.
    public static func press(_ m: AppModel, _ args: [String]) throws -> [String: Any] {
        let e = try event(args)
        let chord = ChordKit.chord(e)
        try InteractionProbe.run {
            guard m.press(chord) else { throw ControlFailure("the key \(chord) (code \(e.keyCode)) does nothing on \(m.surface)") }
        }
        return ["chord": chord]
    }
}
#endif
