import Foundation

/// A chord with two modifiers can be written two ways: ChordKit writes ⌃ ⌥ ⇧ ⌘ ("⇧⌘C"), and a screen that
/// bound its key by hand may have written "⌘⇧C". The engine registers the ChordKit spelling only; when no
/// command of the engine takes such a chord it offers the key to the other spelling, so the real keyboard
/// reaches a binding written the other way round (the composer's ⌘⇧C, ⌘⇧Enter, ⌘⇧Z, ⌘⇧B, ⌘⇧L).
public enum ChordSpelling {
    private static let modifiers: Set<Character> = ["⌃", "⌥", "⇧", "⌘"]

    /// The chord's modifiers and its key: "⇧⌘Enter" -> (["⇧", "⌘"], "Enter").
    public static func split(_ chord: String) -> (mods: [Character], key: String) {
        let mods = Array(chord.prefix { modifiers.contains($0) })
        return (mods, String(chord.dropFirst(mods.count)))
    }

    /// Other orders of the same modifiers, or [] for a chord with fewer than two.
    public static func alternates(of chord: String) -> [String] {
        let (mods, key) = split(chord)
        guard mods.count == 2, !key.isEmpty else { return [] }
        return [String(mods.reversed()) + key]
    }
}
