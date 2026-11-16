import Foundation

/// The String Catalog read as plain JSON: key, then language, then text. `swift build` copies
/// Localizable.xcstrings as it is, while Xcode compiles it into per-language tables; reading the file itself
/// answers the same in both, in the app, in a test and under `-AppleLanguages`.
public struct WordsCatalogue: Sendable {
    private let table: [String: [String: String]]
    public let languages: [String]

    public init?(data: Data) {
        guard let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let strings = root["strings"] as? [String: [String: Any]] else { return nil }
        var t: [String: [String: String]] = [:]
        var langs = Set<String>()
        for (key, entry) in strings {
            guard let locs = entry["localizations"] as? [String: [String: Any]] else { continue }
            var row: [String: String] = [:]
            for (lang, body) in locs {
                if let unit = body["stringUnit"] as? [String: Any], let v = unit["value"] as? String { row[lang] = v; langs.insert(lang) }
            }
            t[key] = row
        }
        table = t
        languages = langs.sorted()
    }

    /// The catalogue language the preferences pick (system order, or the given list), English when none matches.
    public func language(preferring preferences: [String]? = nil) -> String {
        Bundle.preferredLocalizations(from: languages, forPreferences: preferences).first ?? "en"
    }

    public func value(_ key: String, language: String) -> String? { table[key]?[language] ?? table[key]?["en"] }
    public var keys: [String] { table.keys.sorted() }
}
