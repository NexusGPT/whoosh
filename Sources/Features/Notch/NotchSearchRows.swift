import Foundation

/// What the notch's search reports to a check (socket verb `rows`): the field, the answer's rows, the hints under the list.
@MainActor
extension NotchRows {
    static var searchRows: [String: (AppModel) throws -> [ControlRow]] {
        [
            // the field and where the keyboard is: `focused` the cursor is in the field, `row_focus` the mail of the row under the keyboard,
            // `keyboard` the person took the notch with the keyboard; `ms` from the last typed letter to its answer in the model
            NotchIds.search: { _ in
                guard model.stage == .list else { return [] }
                let s = model.search
                return [["ax": NotchIds.search, "text": s.text, "placeholder": NotchCopy.searchPlaceholder, "key": NotchFeature.searchKey,
                         "focused": model.searchFocused, "keyboard": model.keyboardWanted, "row_focus": model.rowFocusId ?? NSNull(),
                         "active": s.active, "waiting": s.waiting, "results": s.results.count, "matches": s.matches, "related": s.results.count - s.matches,
                         "refusal": s.refusal ?? NSNull(), "ms": s.lastMs.map { $0 as Any } ?? NSNull()]]
            },
            NotchIds.searchResults: { _ in
                guard model.stage == .list, model.search.active else { return [] }
                let s = model.search
                return s.results.enumerated().map { i, it -> ControlRow in
                    ["ax": NotchIds.searchRow(i + 1), "id": it.id, "sender": it.sender, "subject": it.subject, "read": it.read,
                     "part": i < s.matches ? "match" : "related", "focused": model.rowFocusId == it.id, "detail": s.gists[it.id] ?? it.preview]
                }
            },
            // which keys the foot names: by the pointer alone the close key; with the keyboard, by its place (the field, a row, an answer)
            NotchIds.hints: { _ in
                guard model.stage == .list else { return [] }
                let place = !model.keyboardWanted ? "pointer" : model.rowFocusId != nil ? "row" : model.search.active ? "answer" : "field"
                return [["ax": NotchIds.hints, "place": place]]
            },
        ]
    }
}
