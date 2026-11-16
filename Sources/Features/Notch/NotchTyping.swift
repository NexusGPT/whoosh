#if os(macOS)
import AppKit

/// Letters that reach the notch's panel while no text field holds the cursor there (a row or a mail has the keyboard). The shell's key monitor
/// asks this hook before it reads such a key as a command (ChordKit.pickerTyping): the notch's own keys go on to its key table; on the list
/// any other letter continues the search (the field takes the cursor back) and Backspace takes its last letter; elsewhere the letter is the
/// notch's and does nothing. Without it a letter pressed on the notch ran as a command of the main window behind it.
@MainActor
enum NotchTyping {
    private static var installed = false

    static func installOnce(own: @escaping @MainActor () -> Set<String>) {
        guard !installed else { return }
        installed = true
        ChordKit.pickerTyping.append { e in
            guard e.window === NotchPanel.shared.window, NotchFeature.holdsKeyboard(.shared) else { ChordKit.note("notch-not-keyed"); return 0 }
            let chord = AppModel.normalise(ChordKit.chord(e))
            if own().contains(chord) { ChordKit.note("notch-own-key"); return 0 }
            let model = NotchModel.shared
            if chord == "Backspace" {
                _ = model.eraseSearch(.shared)
                ChordKit.note("notch-erase")
                return 2
            }
            if model.stage == .list, let typed = e.characters, !typed.isEmpty, typed.unicodeScalars.allSatisfy(isLetter) {
                _ = model.typeSearch(typed, .shared)
                ChordKit.note("notch-search-typed")
                return 2
            }
            ChordKit.note("notch-kept")
            return 2
        }
    }

    /// A character a person types into a field: no control character, no function-key code (the arrows and F keys sit in U+F700 to U+F8FF).
    nonisolated private static func isLetter(_ s: Unicode.Scalar) -> Bool {
        s.value >= 0x20 && s.value != 0x7F && !(0xF700...0xF8FF).contains(s.value)
    }
}
#endif
