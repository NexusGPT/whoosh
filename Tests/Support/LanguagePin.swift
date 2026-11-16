import Foundation
@testable import WhooshUI

/// Pins the app's language for a block of a test and puts the owner's choice back, whatever the block does.
///
/// Why a test needs it: `Words.text` and every number the app writes follow the stored choice (`Words.select`), else the Mac's language list.
/// A test process on a French Mac therefore answers in French, and a test that compares with an English sentence fails there. A test whose
/// point is the sentence or the figure pins the language it expects; a test of the language path pins each language it names.
///
/// Process-wide state: the choice lives in `UserDefaults.standard`, ONE value for the whole test process, and Swift Testing runs tests in parallel.
/// So two pinned blocks never overlap (one process-wide lock, recursive so a block may pin again inside itself), and a block is kept short and
/// synchronous (nothing awaited while pinned). A test that reads `Words` WITHOUT a pin can still run while another test holds a pin; such a test
/// must not compare with a sentence or a figure, or must pin its own language itself.
enum LanguagePin {
    private static let lock = NSRecursiveLock()

    /// Runs `body` with the app's language set to `code` (a catalogue code such as "en" or "fr"), then restores what was selected before.
    static func with<T>(_ code: String, _ body: () throws -> T) rethrows -> T {
        lock.lock()
        let before = Words.selected
        Words.select(code)
        defer { Words.select(before); lock.unlock() }
        return try body()
    }

    /// A count written the way the language `code` writes it ("1,240" in English, "1 240" in French, with that language's own space).
    static func number(_ n: Int, _ code: String) -> String { n.formatted(.number.locale(Locale(identifier: code))) }
}
