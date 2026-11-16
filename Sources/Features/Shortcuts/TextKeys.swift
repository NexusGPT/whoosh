import Foundation

/// Which keys a focused text view or text field takes FIRST, before the engine sees them (lead F18, main74: ⌘Z did nothing in the
/// prompt editor, Return replied to the selected mail from a Settings field). The table, for a chord pressed while a text view
/// holds the focus of the event's own window:
///
///   key                                              who gets it        why
///   a letter, digit, space, punctuation, ⇧ letter    the text           it is typing
///   Return, Tab, ⇧Tab, Backspace, ← → ↑ ↓ (plain/⇧)  the text           the field's own commands (a search field validates on Return,
///                                                                       the quick reply and the composer take Tab, the arrows move the caret)
///   Escape                                           the engine first   screens close on it (search, palette, composer, quick reply); the text
///                                                                       gets it when no screen takes it
///   ⌘Z ⇧⌘Z ⌘A ⌘X ⌘C ⌘V ⌥⌘V ⇧⌥⌘V                       the text           undo, redo, select all, cut, copy, paste live in the field, not the list
///   ⌘ ⌥ ← → ↑ ↓ (+⇧), ⌘ ⌥ Backspace                  the text           line, word and document moves and deletes
///   ⌃A ⌃E ⌃K ⌃F ⌃B ⌃N ⌃P ⌃D ⌃H ⌃T ⌃Y ⌃O               the text           the Cocoa text bindings; the preset binds none of them
///   a chord the focused view lists in `ownsChord`    the text           e.g. an editor that saves on ⌘S (ChordOwningTextView)
///   every other chord with ⌘ ⌃ ⌥ (⌘K, ⌘Enter, ⌘,     the engine         the preset says so and the field does not handle the key
///   ⌘N, ⌘1 to ⌘9, ⌃1 to ⌃9, ⌘B in the composer…)
///
/// Not a text view: nothing changes, the engine gets every key.
public enum TextKeys {
    private static let modifiers: Set<Character> = ["⌃", "⌥", "⇧", "⌘"]
    private static let editing: Set<String> = ["⌘Z", "⇧⌘Z", "⌘A", "⌘X", "⌘C", "⌘V", "⌥⌘V", "⇧⌥⌘V"]
    private static let cocoa: Set<String> = ["⌃A", "⌃E", "⌃K", "⌃F", "⌃B", "⌃N", "⌃P", "⌃D", "⌃H", "⌃T", "⌃Y", "⌃O"]
    private static let moves: Set<String> = ["Left", "Right", "Up", "Down", "\u{7F}", "Backspace", "Delete"]

    /// True when a text view holding the focus takes this chord before the engine does.
    public static func owns(_ chord: String) -> Bool {
        let mods = Set(chord.prefix { modifiers.contains($0) })
        let key = String(chord.dropFirst(mods.count))
        if chord == "Escape" { return false }
        if mods.isEmpty || mods == ["⇧"] { return true }
        if chord == "Shift+Tab" { return true }
        if editing.contains(chord) || cocoa.contains(chord) { return true }
        return moves.contains(key) && !mods.contains("⌃")
    }
}
