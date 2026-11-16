import Foundation
import Observation

/// A saved message: its subject, its text and the files it carries (paths on this Mac).
struct ComposerTemplate: Codable, Equatable, Identifiable {
    var id: String
    var name: String
    var subject: String
    var body: String
    var files: [String]
}

/// A short saved piece of text that goes in at the caret (⌘;).
struct ComposerSnippet: Codable, Equatable, Identifiable {
    var id: String
    var name: String
    var text: String
}

/// Templates, snippets and the "Sent with Whoosh" footer switch. Kept in this Mac's defaults because the core has no
/// record for them yet (reported in docs/lanes/composermac.md "## Round 2"); one file to swap when it does.
@MainActor @Observable
final class ComposerLibrary {
    static let shared = ComposerLibrary()
    private static let key = "whoosh.composer.library"
    private static let footerKey = "whoosh.composer.footer"

    private struct Stored: Codable { var templates: [ComposerTemplate]; var snippets: [ComposerSnippet] }

    private(set) var templates: [ComposerTemplate]
    private(set) var snippets: [ComposerSnippet]
    /// Files attached lately, newest first, for the attach panel. The count is a copy of a constant the lead owns (docs/lanes/composermac.md asks for COMPOSER_RECENT_FILES).
    private(set) var recentFiles: [String] = UserDefaults.standard.stringArray(forKey: "whoosh.composer.recent") ?? []
    static let recentMax = 4
    var footerOn: Bool { didSet { UserDefaults.standard.set(footerOn, forKey: Self.footerKey) } }

    static var footerLine: String { Words.text("Sent with Whoosh") }

    init() {
        footerOn = UserDefaults.standard.bool(forKey: Self.footerKey)
        if let data = UserDefaults.standard.data(forKey: Self.key), let s = try? JSONDecoder().decode(Stored.self, from: data) {
            templates = s.templates; snippets = s.snippets
        } else {
            templates = [ComposerTemplate(id: "t-confirm", name: Words.text("Confirm delivery"), subject: Words.text("Delivery confirmation"),
                                          body: Words.text("Hi,\n\nPlease confirm the delivery date and send me the signed quote.\n\nThank you"), files: [])]
            snippets = [ComposerSnippet(id: "s-thanks", name: Words.text("Thanks, received"), text: Words.text("Thanks, received. I will come back to you shortly.")),
                        ComposerSnippet(id: "s-move", name: Words.text("Move our call"), text: Words.text("Could we move our call to another time? Tell me what suits you."))]
        }
    }

    @discardableResult
    func addTemplate(subject: String, body: String, files: [String]) -> ComposerTemplate {
        let name = Self.name(from: subject.isEmpty ? body : subject)
        let t = ComposerTemplate(id: "t-\(UUID().uuidString)", name: name, subject: subject, body: body, files: files)
        templates.append(t); persist()
        return t
    }

    @discardableResult
    func addSnippet(text: String) -> ComposerSnippet {
        let s = ComposerSnippet(id: "s-\(UUID().uuidString)", name: Self.name(from: text), text: text)
        snippets.append(s); persist()
        return s
    }

    /// The attach panel lists a file again after it was attached once; a file that no longer exists is dropped.
    func noteRecent(_ path: String) {
        recentFiles = ([path] + recentFiles.filter { $0 != path }).filter { FileManager.default.fileExists(atPath: $0) }.prefix(Self.recentMax).map { $0 }
        UserDefaults.standard.set(recentFiles, forKey: "whoosh.composer.recent")
    }

    /// A title from the first line of a text; the row clips it to one line.
    static func name(from text: String) -> String {
        text.split(whereSeparator: \.isNewline).first.map { $0.trimmingCharacters(in: .whitespaces) } ?? text
    }

    private func persist() {
        if let data = try? JSONEncoder().encode(Stored(templates: templates, snippets: snippets)) { UserDefaults.standard.set(data, forKey: Self.key) }
    }
}
