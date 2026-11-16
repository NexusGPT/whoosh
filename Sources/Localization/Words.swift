import Foundation
import Observation

/// The one entry to every user-facing string. The key is the English text, with `%@` for a word and `%lld` for a
/// number, exactly as `Sources/Resources/Localizable.xcstrings` holds it in 14 languages. A key the catalogue lacks
/// answers `default`, else the key itself, so a missing translation shows English and never an empty label.
///
///     Text(Words.text("Smart folders"))
///     Text(Words.text("%lld selected", n))
///     Text(Words.text("%@, %lld unread", name, unread))
///
/// ONE language at a time, for every string: the app's own choice (`Words.select`, stored under `Words.languageKey`)
/// when there is one, else the system's list. Both ways answer from the same catalogue, so a window never holds two
/// languages. A key missing in the chosen language answers English, never another language.
public enum Words {
    /// The stored choice: a catalogue language code, or `system` (the default) to follow the Mac's language list.
    public static let languageKey = "whoosh.language"
    public static let system = "system"
    /// Posted once the choice is stored (by `select`, or by the Settings pane that wrote the key itself); `WordsLanguage.shared.revision` then moves.
    public static let languageDidChange = Notification.Name("whoosh.languageChanged")
    /// The 14 catalogue languages with their own names, in the order a picker lists them after "System".
    public static let languages: [(code: String, name: String)] = [
        ("en", "English"), ("fr", "Français"), ("es", "Español"), ("pt", "Português"), ("de", "Deutsch"), ("it", "Italiano"),
        ("nl", "Nederlands"), ("pl", "Polski"), ("sv", "Svenska"), ("da", "Dansk"), ("nb", "Norsk bokmål"), ("fi", "Suomi"),
        ("ja", "日本語"), ("zh-Hans", "简体中文")]

    private static let catalogue: WordsCatalogue? = {
        guard let url = Bundle.module.url(forResource: "Localizable", withExtension: "xcstrings"),
              let data = try? Data(contentsOf: url) else { return nil }
        return WordsCatalogue(data: data)
    }()
    private static let codes = languages.map(\.code)
    private static let systemLanguage: String = catalogue?.language() ?? Bundle.preferredLocalizations(from: codes).first ?? "en"
    private static let tables = LanguageTables()
    private static let missing = "\u{1}missing\u{1}"

    /// The language every string answers in now: the stored choice, else the system's.
    public static var current: String {
        let pick = UserDefaults.standard.string(forKey: languageKey) ?? system
        return codes.contains(pick) ? pick : systemLanguage
    }
    /// The stored choice (`system` or a code).
    public static var selected: String { UserDefaults.standard.string(forKey: languageKey) ?? system }

    /// Stores the choice (`system` or a code; anything else is `system`) and tells the windows.
    public static func select(_ code: String) {
        UserDefaults.standard.set(codes.contains(code) ? code : system, forKey: languageKey)
        NotificationCenter.default.post(name: languageDidChange, object: nil)
    }

    public static func text(_ key: String, default fallback: String? = nil, _ args: any CustomStringConvertible...) -> String {
        _ = WordsLanguage.shared.revision   // a view body that reads a word depends on the language and re-renders when it changes
        let found = lookup(key) ?? fallback ?? key
        return args.isEmpty ? found : WordsFormat.fill(found, args.map { String(describing: $0) })
    }

    /// Every catalogue language's text of a key (the English source included): a stored default is recognised in whichever language it was seeded.
    public static func allTexts(_ key: String) -> Set<String> {
        var out: Set<String> = [key]
        for code in codes {
            if let table = tables.bundle(code) {
                let v = table.localizedString(forKey: key, value: missing, table: nil)
                if v != missing { out.insert(v); continue }
            }
            if let v = catalogue?.value(key, language: code) { out.insert(v) }
        }
        return out
    }

    private static func lookup(_ key: String) -> String? {
        // ONE path for the stored choice and for the system's: the explicit `<code>.lproj` of the resource bundle, then the catalogue file.
        // The system language used to ask `Bundle.module.localizedString` instead, which picks its table by CFBundle's own language
        // negotiation and, for a key that table lacks, answers the English table's text (never the `missing` sentinel), so an English
        // word reached the screen under a French system while every explicit choice was French.
        let pick = UserDefaults.standard.string(forKey: languageKey) ?? system
        let code = codes.contains(pick) ? pick : systemLanguage
        if let table = tables.bundle(code) {
            let compiled = table.localizedString(forKey: key, value: missing, table: nil)
            if compiled != missing { return compiled }
        }
        return catalogue?.value(key, language: code)
    }
}

/// The compiled `<code>.lproj` of the resource bundle, found once per language (nil in a `swift build`, which keeps the catalogue file).
private final class LanguageTables: @unchecked Sendable {
    private var cache: [String: Bundle?] = [:]
    private let lock = NSLock()

    func bundle(_ code: String) -> Bundle? {
        lock.lock(); defer { lock.unlock() }
        if let hit = cache[code] { return hit }
        let found = Bundle.module.path(forResource: code, ofType: "lproj").flatMap { Bundle(path: $0) }
        cache[code] = found
        return found
    }
}

/// Moves when the language choice changes, so SwiftUI views that read `Words.text` (or the revision) re-render without a restart.
@Observable
public final class WordsLanguage: @unchecked Sendable {
    public static let shared = WordsLanguage()
    public private(set) var revision = 0
    private init() {
        NotificationCenter.default.addObserver(forName: Words.languageDidChange, object: nil, queue: .main) { [weak self] _ in self?.revision += 1 }
    }
}
