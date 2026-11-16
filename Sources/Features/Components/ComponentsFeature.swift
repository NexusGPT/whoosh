import Foundation

/// The components lane's registration: the rows of the two pickers (socket verb `rows`), and the gallery the
/// design build shoots (`click components.gallery.*`). Nothing here draws; the controls live in this folder's views.
public enum ComponentsFeature: FeatureModule {
    public static let id = "components"

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "mac-main.snooze.options": { m in
                let now = Date(timeIntervalSince1970: Double(m.host.clock.nowMs()) / 1000)
                let iso = ISO8601DateFormatter()
                return TimeInWords.options(now: now).enumerated().map { i, o in
                    var row: ControlRow = ["ax": "mac-main.snooze.options.\(i)", "id": o.id, "title": o.title,
                                           "refused": o.refusal != nil, "resolves_to": o.when.map { iso.string(from: $0) } ?? NSNull()]
                    if let r = o.refusal { row["reason"] = r }
                    if let w = o.when { row["detail"] = TimeInWords.describe(w, now: now) }
                    return row
                }
            },
            "mac-main.undo.suggestion": { m in
                (SuggestionActs.current(m).map { [$0] } ?? []).enumerated().map { i, o in
                    ["ax": "mac-main.undo.suggestion.\(i)", "id": o.id, "kind": "\(o.kind)", "subject": o.subject, "key": o.key,
                     "title": o.title, "detail": o.detail, "label_id": o.labelId ?? NSNull()]
                }
            },
            "whoosh.brand-mark": { _ in BrandMarkRegistry.rows() },
            "mac-main.label.picker": { _ in
                LabelPickerModel(nodes: LabelFixture.nodes()).allRows().enumerated().map { i, r in
                    ["ax": "mac-main.label.picker.\(i)", "id": r.id, "name": r.name, "path": r.path, "depth": r.depth, "has_children": r.hasChildren]
                }
            },
        ]
    }

    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        var k: [String: (AppModel) -> Bool] = [:]
        for kind in [SuggestionKind.blockSender, .alwaysJunkDomain, .unsubscribe, .muteThread, .alwaysLabel] {
            let letter = SuggestionOffer.key(for: kind)
            k[letter] = { SuggestionActs.key(letter, $0) }
        }
        return k
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [
            "mac-main.undo.suggestion.accept": { SuggestionActs.accept($0) },
            "mac-main.undo.suggestion.dismiss": { SuggestionActs.dismiss($0) },
        ]
        #if os(macOS)
        t["components.gallery.open"] = { _ in ComponentGalleryWindow.shared.show() }
        t["components.gallery.light"] = { _ in ComponentGalleryWindow.shared.show(dark: false) }
        t["components.gallery.dark"] = { _ in ComponentGalleryWindow.shared.show(dark: true) }
        for page in GalleryPage.allCases { t["components.gallery.show.\(page.rawValue)"] = { _ in ComponentGalleryWindow.shared.show(page: page) } }
        #endif
        return t
    }
}
