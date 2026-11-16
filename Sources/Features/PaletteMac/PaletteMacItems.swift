import Foundation

/// The account facts a jump row draws its emblem from (AccountBook, one directory for the app).
struct PaletteEmblem: Equatable {
    let name: String, address: String, colour: String, iconId: String?
}

/// One line of the palette: a command by name with its key, or a place to jump to.
struct PaletteItem: Identifiable, Equatable {
    enum Kind: String { case command, account, tab, label, settings }
    enum Act: Equatable {
        case command(String), chord(String), resultTarget(String), moveAccount
        /// vip-mark: the focused mail's sender joins (true) or leaves (false) the VIP list
        case markVip(Bool)
        case jumpAccount(String), jumpTab(String), jumpLabel(String), openSettings(String)
        case none
    }
    let id: String
    let title: String
    var detail = ""
    let key: String
    let kind: Kind
    let act: Act
    var emblem: PaletteEmblem?
    /// Text the fuzzy match reads: the title, and for a jump what sits beside it.
    var haystack: String { detail.isEmpty ? title : title + " " + detail }
    var isJump: Bool { kind != .command }

    /// The key as separate caps: "⌘K" is two, "⇧⌘P" three, "Tab" one, "Shift+Tab" is ⇧ and Tab.
    var caps: [String] { PaletteItem.caps(key) }

    static func caps(_ chord: String) -> [String] {
        guard !chord.isEmpty else { return [] }
        let words = ["Enter": ["↵"], "Escape": [PaletteMacWords.escKey], "Down": ["↓"], "Up": ["↑"], "Shift+Tab": ["⇧", PaletteMacWords.tabKey]]
        if let w = words[chord] { return w }
        let mods: Set<Character> = ["⌃", "⌥", "⇧", "⌘"]
        let chars = Array(chord)
        if chars.count > 1, chars.dropLast().allSatisfy({ mods.contains($0) }) { return chars.map(String.init) }
        return [chord]
    }
}
