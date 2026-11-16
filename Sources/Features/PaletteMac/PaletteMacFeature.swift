import SwiftUI

/// The palettemac lane's registration. ⌘K (the "palette" command) opens it; every feature's palette commands and
/// the places to jump to are its lines. It draws in the shell's overlay slot (`mac-main.overlay`) over the real
/// inbox, and it takes Enter, Up, Down and Escape only while it holds the focus (`hasFocus`).
@MainActor
public enum PaletteMacFeature: FeatureModule, CommandProviding {
    public static let id = "palettemac"
    public static let surfaces: [String] = ["mac-palette"]
    static var m: PaletteMacModel { .shared }

    /// Focus is the palette itself, from the frame it opens until the frame it closes.
    public static func hasFocus(_ app: AppModel) -> Bool { m.isOpen }

    public static func view(for slot: SurfaceSlot) -> AnyView? {
        slot == .macOverlay ? AnyView(PaletteMacOverlay()) : nil
    }

    public static func commands() -> [String: (AppModel) -> Bool] {
        [
            "palette": { _ in m.open() },
            // routed by focus: while the palette is closed these return false and the next provider answers
            "open": { _ in m.isOpen && m.run() },
            "next": { _ in m.isOpen && m.move(1) },
            "previous": { _ in m.isOpen && m.move(-1) },
        ]
    }

    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        [
            "Enter": { _ in m.isOpen && m.run() },
            "Down": { _ in m.isOpen && m.move(1) },
            "Up": { _ in m.isOpen && m.move(-1) },
            "Escape": { _ in m.isOpen && m.close() },
        ]
    }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "mac-palette.results": { _ in
                m.commands.enumerated().map { i, item in
                    ["ax": "mac-palette.results.\(i)", "id": item.id, "title": item.title, "key": item.key, "kind": item.kind.rawValue,
                     "selected": m.step == .commands && i == m.selected]
                }
            },
            "mac-palette.jumps": { _ in
                let first = m.commands.count
                return m.jumps.enumerated().map { j, item in
                    ["ax": "mac-palette.jumps.\(j)", "id": item.id, "title": item.title, "detail": item.detail, "kind": item.kind.rawValue,
                     "emblem": item.emblem != nil, "selected": m.step == .commands && first + j == m.selected]
                }
            },
            "mac-palette.accounts": { _ in
                m.accounts.enumerated().map { i, a in
                    ["ax": "mac-palette.accounts.\(i)", "id": a.id, "title": a.name, "address": a.address, "selected": i == m.selected]
                }
            },
            "mac-palette.state": { _ in
                [["ax": "mac-palette.state", "open": m.isOpen, "step": m.step == .commands ? "commands" : "accounts", "query": m.query,
                  "selected": m.selected, "count": m.count, "empty": m.count == 0, "words": m.count == 0 ? PaletteMacWords.empty(m.query) : ""]]
            },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [
            "mac-palette.run": { _ in m.isOpen && m.run() },
            "mac-palette.next": { _ in m.isOpen && m.move(1) },
            "mac-palette.previous": { _ in m.isOpen && m.move(-1) },
            "mac-palette.close": { _ in m.isOpen && m.close() },
            /// A click on the dimmed window behind the card (the view's scrim tap): closes the palette like Escape.
            "mac-palette.scrim": { _ in m.isOpen && m.close() },
        ]
        let first = m.commands.count
        for i in 0..<first { t["mac-palette.results.\(i)"] = { _ in m.isOpen && m.step == .commands && m.run(i) } }
        for j in 0..<m.jumps.count { t["mac-palette.jumps.\(j)"] = { _ in m.isOpen && m.step == .commands && m.run(first + j) } }
        for i in 0..<m.accounts.count { t["mac-palette.accounts.\(i)"] = { _ in m.isOpen && m.step == .accounts && m.pick(i) } }
        return t
    }

    public static func paletteCommands() -> [(title: String, key: String)] {
        // vip-mark: no key of its own (owner, 7 Nov); the title says what Enter does to the focused mail's sender
        [(title: PaletteMacWords.moveTitle, key: ""), (title: PaletteMacWords.generalSettings, key: "⌘,"),
         (title: m.focusedSenderIsVip ? PaletteMacWords.removeVip : PaletteMacWords.markVip, key: "")]
    }
}
