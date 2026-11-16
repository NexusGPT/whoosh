import Foundation
import CoreGraphics

/// An inline completion the owner has started in the text: `;name` for a snippet, `:smi` for an emoji. The editor finds
/// it from the text before the caret; the composer supplies the list and the editor replaces the typed trigger.
public struct RichTrigger: Equatable, Sendable {
    public enum Kind: String, Sendable { case snippet, emoji }
    public let kind: Kind
    /// What follows the trigger character, as typed.
    public let query: String
    /// How many UTF-16 units before the caret the trigger and its query take: what a pick removes.
    public var length: Int { 1 + query.utf16.count }

    public init(kind: Kind, query: String) { self.kind = kind; self.query = query }

    /// An emoji list opens from this many letters on, so "10:30" and ":)" never open it.
    public static let emojiMinimum = 2

    /// `before` is the text of the caret's line up to the caret. The trigger character must start the line or follow a
    /// space, so "a;b", "10:30" and "https://" are plain text.
    public static func detect(before: String) -> RichTrigger? {
        let chars = Array(before)
        var i = chars.count
        while i > 0, isQueryChar(chars[i - 1]) { i -= 1 }
        guard i > 0, chars[i - 1] == ";" || chars[i - 1] == ":" else { return nil }
        let mark = chars[i - 1]
        if i - 1 > 0, !chars[i - 2].isWhitespace { return nil }
        let query = String(chars[i...])
        if mark == ";" { return RichTrigger(kind: .snippet, query: query) }
        return query.count >= emojiMinimum ? RichTrigger(kind: .emoji, query: query) : nil
    }

    private static func isQueryChar(_ c: Character) -> Bool { c.isLetter || c.isNumber || c == "_" || c == "+" || c == "-" }
}

/// What the editor tells the composer about the trigger under the caret: the trigger, and where it is.
public struct RichTriggerState: Equatable, Sendable {
    public let trigger: RichTrigger
    /// The caret's rectangle in the editor's own coordinates (top-left origin).
    public let caret: CGRect
    public init(trigger: RichTrigger, caret: CGRect) { self.trigger = trigger; self.caret = caret }
}

/// Keys the editor hands to the composer while a trigger list is open.
public enum RichListKey: Sendable { case up, down, accept, cancel }
