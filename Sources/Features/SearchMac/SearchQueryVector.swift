import Foundation

/// Per keystroke, the meaning search's query vector is made for the field's latest plain words only: the embedding lane's
/// QueryEncoder (newest query wins, cached) drops a query a newer keystroke replaced, so fast typing queues no stale work and
/// Enter finds the vector ready. The encoder is the embedding lane's one instance over its one model (`MeaningSearch.queryEncoder`);
/// this screen holds no model and no cache size of its own.
@MainActor
enum SearchQueryVector {
    private static var last = ""

    static func typed(_ q: SearchQuery) {
        guard let words = SearchEngine.coreWords(q), words != last, let encoder = MeaningSearch.queryEncoder else { return }
        last = words
        Task.detached(priority: .userInitiated) { _ = await encoder.vector(for: words) }   // never awaited by the field: typing never waits
    }
}
