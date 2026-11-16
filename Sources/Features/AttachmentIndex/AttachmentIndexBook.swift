import Foundation

/// The attachment index as the fixture core runs it until the app mounts the Rust core (core/crates/attachment-index does the same
/// with the same readers): index the downloaded files, then answer a query with rows that name the attachment and its message.
/// Meaning hits come from the media and text vectors through the contract `Embedder`; word hits from the text; name hits say
/// "not indexed yet" or "name only" for a file that could not be read (J2.8).
public final class AttachmentIndexBook: @unchecked Sendable {
    public static let notDownloadedWords = "not indexed yet, download to search inside"
    public static let unreadableWords = "this kind of file cannot be read here, searched by name only"

    public struct Item: Sendable {
        public let messageId: String, attachmentId: String, fileName: String, mimeType: String
        /// nil until the file is downloaded
        public let localURL: URL?
        public init(messageId: String, attachmentId: String, fileName: String, mimeType: String, localURL: URL?) {
            self.messageId = messageId; self.attachmentId = attachmentId; self.fileName = fileName; self.mimeType = mimeType; self.localURL = localURL
        }
    }

    enum State: Equatable { case indexed, notDownloaded, unreadable, failed(String) }
    struct Entry { let item: Item; var state: State; var words = ""; var vectors: [[Float]] = [] }

    private let embedder: Embedder
    private let scanner: AttachmentScanner
    private let scratch: URL
    private let lock = NSLock()
    private var entries: [String: Entry] = [:]

    public init(embedder: Embedder, scanner: AttachmentScanner, scratch: URL) {
        self.embedder = embedder; self.scanner = scanner; self.scratch = scratch
    }

    public func index(_ items: [Item]) {
        for item in items {
            let entry = read(item)
            lock.lock(); entries[item.attachmentId] = entry; lock.unlock()
        }
    }

    private func read(_ item: Item) -> Entry {
        let kind = AttachmentFileKind.detect(mime: item.mimeType, fileName: item.fileName)
        if kind == .unreadable { return Entry(item: item, state: .unreadable) }
        guard let url = item.localURL, FileManager.default.fileExists(atPath: url.path) else { return Entry(item: item, state: .notDownloaded) }
        var scanned: ScannedAttachment
        switch kind {
        case .readable(let k): scanned = scanner.scan(url, kind: k, scratch: scratch)
        default:
            scanned = ScannedAttachment()
            scanned.text = (try? String(contentsOf: url, encoding: .utf8))?.trimmingCharacters(in: .whitespacesAndNewlines)
        }
        defer { if let folder = scanned.workFolder { try? FileManager.default.removeItem(at: folder) } }
        var entry = Entry(item: item, state: .indexed, words: scanned.words)
        for part in scanned.mediaParts {
            if let v = try? embedder.embedMedia(kind: part.kind, path: part.url.path), Self.usable(v, dimension: Int(embedder.dimension())) { entry.vectors.append(v) } else { scanned.notes.append("embedding \(part.url.lastPathComponent)") }
        }
        if !entry.words.isEmpty, let v = try? embedder.embed(texts: [entry.words]).first, Self.usable(v, dimension: Int(embedder.dimension())) { entry.vectors.append(v) }
        if entry.words.isEmpty && entry.vectors.isEmpty, let first = scanned.notes.first { entry.state = .failed(first) }
        return entry
    }

    private static func usable(_ v: [Float], dimension: Int) -> Bool { v.count == dimension && v.allSatisfy(\.isFinite) && v.contains { $0 != 0 } }

    /// One row per attachment. Order: content hits (meaning and words) before name hits; the search core fuses by rank.
    public func hits(for query: String) -> [SearchHit] {
        lock.lock(); let all = Array(entries.values); lock.unlock()
        let q = Self.queryWords(query)
        guard !q.isEmpty, let qv = try? embedder.embedQuery(text: query) else { return [] }
        var content: [(Entry, Float, String)] = []
        var names: [SearchHit] = []
        var byWordsIds = Set<String>()
        for e in all.sorted(by: { $0.item.attachmentId < $1.item.attachmentId }) {
            let have = Set(Self.words(e.words))
            let nameWords = Self.words(e.item.fileName)
            let byWords = e.state == .indexed && q.allSatisfy(have.contains)
            let cosine = e.vectors.map { zip(qv, $0).reduce(Float(0)) { $0 + $1.0 * $1.1 } }.max() ?? 0
            if byWords { content.append((e, 1, e.words.replacingOccurrences(of: "\n", with: " "))); byWordsIds.insert(e.item.attachmentId) }
            else if cosine > 0 { content.append((e, cosine, e.words.isEmpty ? e.item.fileName : e.words.replacingOccurrences(of: "\n", with: " "))) }
            if q.allSatisfy({ w in nameWords.contains { $0.hasPrefix(w) } }) {
                names.append(SearchHit(messageId: e.item.messageId, score: 1, snippet: Self.said(e), matchedIn: .attachmentName, attachmentName: e.item.fileName))
            }
        }
        // a file that holds every typed word is a match inside the attachment; a file that is only close in meaning (any cosine above 0, so every indexed file for
        // any query) holds none of the letters and is never a match (SPEC A149, A150; main88: "zzqxvk" listed the Linea mails with files as matches): `.related`
        let contentHits = content.sorted { $0.1 > $1.1 }.map { SearchHit(messageId: $0.0.item.messageId, score: Double($0.1), snippet: String($0.2.prefix(130)),
                                                                          matchedIn: byWordsIds.contains($0.0.item.attachmentId) ? .attachmentContent : .related,
                                                                          attachmentName: $0.0.item.fileName) }
        // a name hit gives way only to a word hit inside the same file, never to a file that is merely close in meaning (J2.8 "the warehouse photo" names it)
        let seen = Set(contentHits.filter { $0.matchedIn == .attachmentContent }.map { $0.messageId + "/" + ($0.attachmentName ?? "") })
        return contentHits + names.filter { !seen.contains($0.messageId + "/" + ($0.attachmentName ?? "")) }
    }

    private static func said(_ e: Entry) -> String {
        switch e.state {
        case .indexed: return e.item.fileName
        case .notDownloaded: return notDownloadedWords
        case .unreadable: return unreadableWords
        case .failed(let words): return words
        }
    }

    private static let filler: Set<String> = ["the", "and", "for", "with", "from", "that", "this", "what", "which", "any", "all", "about", "in", "of", "to", "on", "at", "a", "an", "is", "it"]
    static func words(_ s: String) -> [String] { s.lowercased().split(whereSeparator: { !$0.isLetter && !$0.isNumber }).map(String.init) }
    static func queryWords(_ s: String) -> [String] { words(s).filter { !filler.contains($0) } }
}
