import SwiftUI

/// The iPhone palette's registrations: rows, clicks and the door register here, never in a hub.
/// ax ids follow `<surface>.<region>[.<element>...][.<index>]` (checks/palettephone-check.py).
@MainActor
public enum PalettePhoneFeature: FeatureModule {
    public static let id = "palettephone"
    public static let surfaces: [String] = ["iphone-palette"]
    static var m: PalettePhoneModel { .shared }

    /// Focus is the palette itself, from the frame it opens until the frame it closes.
    public static func hasFocus(_ app: AppModel) -> Bool { m.isOpen }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "iphone-palette.results": { _ in
                m.flat.enumerated().map { i, item in
                    ["ax": "iphone-palette.results.\(i)", "id": item.id, "title": item.title, "detail": item.detail,
                     "group": item.headerKey, "symbol": item.symbol, "account_mark": item.accountId != nil]
                }
            },
            "iphone-palette.state": { _ in
                let n = m.flat.count
                return [["ax": "iphone-palette.state", "open": m.isOpen, "query": m.query, "count": n, "empty": n == 0,
                         "words": n == 0 ? PalettePhoneWords.noMatch(m.query) : "", "line": m.line ?? ""]]
            },
            "iphone-palette.last": { _ in
                guard let r = m.last else { return [] }
                return [["ax": "iphone-palette.last", "id": r.id, "title": r.title, "outcome": r.outcome, "words": r.words]]
            },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [
            "iphone-palette.open": { _ in m.open() },
            "iphone-palette.close": { _ in m.isOpen && m.close() },
            "iphone-palette.clear": { _ in m.isOpen && m.clear() },
            /// The keyboard's Go key: runs the first line.
            "iphone-palette.submit": { _ in m.isOpen && m.runFirst() },
        ]
        for i in 0..<m.flat.count { t["iphone-palette.results.\(i)"] = { _ in m.isOpen && m.run(i) } }
        return t
    }
}

/// The door other screens call: `PalettePhone.open()` from a button, plus `.palettePhoneSheet()` once on the screen that hosts the sheet.
public enum PalettePhone {
    @MainActor @discardableResult
    public static func open() -> Bool { PalettePhoneModel.shared.open() }
}
