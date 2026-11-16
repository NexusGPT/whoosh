import Foundation

/// The emoji behind `:name`. A short list of the names people type (smile, tada, +1), then every emoji the system knows by
/// its Unicode name ("smiling face" is `:smiling_face`). An inline list under the caret, not the system's Character
/// picker: the picker is a window, and a key typed in the text must not leave the text.
enum ComposerEmoji {
    struct Item: Equatable, Identifiable { let name: String; let glyph: String; var id: String { name } }

    static let limit = 6

    private static let common: [(String, String)] = [
        ("smile", "😄"), ("smiley", "😃"), ("grin", "😁"), ("joy", "😂"), ("laughing", "😆"), ("wink", "😉"), ("blush", "😊"),
        ("heart_eyes", "😍"), ("kissing_heart", "😘"), ("thinking", "🤔"), ("sunglasses", "😎"), ("cry", "😢"), ("sob", "😭"),
        ("angry", "😠"), ("scream", "😱"), ("thumbsup", "👍"), ("+1", "👍"), ("thumbsdown", "👎"), ("-1", "👎"), ("clap", "👏"),
        ("pray", "🙏"), ("wave", "👋"), ("ok_hand", "👌"), ("muscle", "💪"), ("tada", "🎉"), ("fire", "🔥"), ("sparkles", "✨"),
        ("star", "⭐"), ("heart", "❤️"), ("broken_heart", "💔"), ("rocket", "🚀"), ("eyes", "👀"), ("white_check_mark", "✅"),
        ("x", "❌"), ("warning", "⚠️"), ("bulb", "💡"), ("coffee", "☕"), ("sun", "☀️"), ("partying", "🥳"), ("hugs", "🤗"),
        ("sweat_smile", "😅"), ("slightly_smiling", "🙂"), ("upside_down", "🙃"), ("neutral", "😐"), ("sleeping", "😴"),
        ("facepalm", "🤦"), ("shrug", "🤷"), ("100", "💯"), ("handshake", "🤝"), ("memo", "📝"), ("calendar", "📅"),
        ("email", "📧"), ("phone", "📞"), ("clock", "🕐"), ("lock", "🔒"), ("key", "🔑"), ("moneybag", "💰"), ("gift", "🎁"),
        ("trophy", "🏆"), ("balloon", "🎈"), ("cake", "🍰"), ("pizza", "🍕"), ("beer", "🍺"), ("wine", "🍷"), ("dog", "🐶"),
        ("cat", "🐱"), ("flower", "🌸"), ("rainbow", "🌈"),
    ]

    /// Every emoji by Unicode name, built once on first use (about two thousand scalars, a few milliseconds).
    private static let named: [(String, String)] = {
        var out: [(String, String)] = []
        for range in [0x1F300...0x1FAFF, 0x2600...0x27BF] {
            for v in range {
                guard let u = Unicode.Scalar(v), u.properties.isEmojiPresentation, let n = u.properties.name else { continue }
                out.append((n.lowercased().replacingOccurrences(of: " ", with: "_"), String(u)))
            }
        }
        return out
    }()

    /// The matches for what was typed after the colon: exact name first, then names that start with it, then names that
    /// have a word starting with it. At most `limit`, no glyph twice.
    static func matches(_ query: String) -> [Item] {
        let q = query.lowercased()
        guard !q.isEmpty else { return [] }
        var out: [Item] = []
        func add(_ pairs: [(String, String)], _ test: (String) -> Bool) {
            for (n, g) in pairs where out.count < limit && test(n) && !out.contains(where: { $0.glyph == g }) { out.append(Item(name: n, glyph: g)) }
        }
        add(common) { $0 == q }
        add(common) { $0.hasPrefix(q) }
        add(common) { $0.split(separator: "_").contains { $0.hasPrefix(q) } }
        if out.count < limit {
            add(named) { $0.hasPrefix(q) }
            add(named) { $0.split(separator: "_").contains { $0.hasPrefix(q) } }
        }
        return out
    }
}

/// The snippet list under `;`: names with a word that starts with what was typed. Empty query: all, in order.
enum ComposerSnippetMatch {
    static func indices(_ query: String, names: [String]) -> [Int] {
        let q = query.lowercased()
        guard !q.isEmpty else { return Array(names.indices) }
        func words(_ s: String) -> [Substring] { s.lowercased().split { !$0.isLetter && !$0.isNumber } }
        let starts = names.indices.filter { names[$0].lowercased().hasPrefix(q) }
        let inside = names.indices.filter { !starts.contains($0) && words(names[$0]).contains { $0.hasPrefix(q) } }
        return starts + inside
    }
}
