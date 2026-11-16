import Foundation
import Observation

/// The phone palette's state. The field's text is `AppModel.typed` (one home: the keyboard and the socket `type` verb both
/// write it). The pool is read when the palette opens; typing only ranks it, so a key never waits on the core.
@MainActor @Observable
final class PalettePhoneModel {
    static let shared = PalettePhoneModel()
    static let overlayName = "iphone-palette"

    struct Section: Identifiable {
        let group: PalettePhoneItem.Group
        let items: [PalettePhoneItem]
        var id: Int { group.rawValue }
        var header: String {
            switch group {
            case .suggested: PalettePhoneWords.suggested
            case .commands: PalettePhoneWords.commandsHeader
            case .goTo: PalettePhoneWords.goToHeader
            }
        }
    }
    /// What the last chosen line did: the socket's `iphone-palette.last` row and the refusal line read it.
    struct Run: Equatable { let id: String, title: String, outcome: String, words: String }

    var line: String?
    var last: Run?
    @ObservationIgnored private var pool: [PalettePhoneItem] = []
    @ObservationIgnored private var memo: (query: String, sections: [Section])?
    @ObservationIgnored private var backTo = "iphone-inbox"

    var app: AppModel { AppModel.shared }
    var isOpen: Bool { app.overlay == Self.overlayName }
    var query: String { app.typed }

    /// The lines in the order they are drawn: Suggested, Commands, Go to; a group with no match is not drawn.
    var sections: [Section] {
        if let m = memo, m.query == query { return m.sections }
        let ranked = PalettePhoneItem.Group.allCases.compactMap { g -> Section? in
            let items = PaletteMatch.rank(pool.filter { $0.group == g }, query) { $0.haystack }
            return items.isEmpty ? nil : Section(group: g, items: items)
        }
        memo = (query, ranked)
        return ranked
    }
    var flat: [PalettePhoneItem] { sections.flatMap(\.items) }

    @discardableResult
    func open() -> Bool {
        guard !isOpen else { return true }
        backTo = app.surface == Self.overlayName ? "iphone-inbox" : app.surface
        app.typed = ""; line = nil; memo = nil
        pool = PalettePhoneCatalog.pool(app)
        app.surface = Self.overlayName; app.overlay = Self.overlayName
        return true
    }

    @discardableResult
    func close() -> Bool {
        app.typed = ""; line = nil; memo = nil
        if app.overlay == Self.overlayName { app.overlay = nil }
        if app.surface == Self.overlayName { app.surface = backTo }
        return true
    }

    @discardableResult
    func clear() -> Bool { app.typed = ""; line = nil; return true }

    @discardableResult
    func runFirst() -> Bool { run(0) }

    @discardableResult
    func run(_ index: Int) -> Bool {
        let list = flat
        guard list.indices.contains(index) else { return false }
        return run(list[index])
    }
}
