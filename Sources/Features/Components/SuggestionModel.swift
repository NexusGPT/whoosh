import Foundation

/// One follow-up the core offers after an action, as the undo bar's place shows it: one action word, one letter.
/// The core serves the rows (`WhooshCore.suggestions()`); this type only names them. A suggestion never acts alone:
/// only `acceptSuggestion` (which the core turns into a block, a rule, a mute or an unsubscribe) writes anything.
public struct SuggestionOffer: Equatable, Sendable, Identifiable {
    public let id: String
    public let kind: SuggestionKind
    public let subject: String
    public let labelId: String?

    public init(_ s: Suggestion) { id = s.id; kind = s.kind; subject = s.subject; labelId = s.labelId }

    /// One plain letter per kind, none already taken by a list key (J K E P Z L H S).
    public var key: String { Self.key(for: kind) }
    public static func key(for kind: SuggestionKind) -> String {
        switch kind {
        case .blockSender: return "B"
        case .alwaysJunkDomain: return "D"
        case .unsubscribe: return "U"
        case .muteThread: return "M"
        case .alwaysLabel: return "A"
        }
    }
    public static func kind(forKey chord: String) -> SuggestionKind? {
        [SuggestionKind.blockSender, .alwaysJunkDomain, .unsubscribe, .muteThread, .alwaysLabel].first { key(for: $0) == chord.uppercased() }
    }

    /// The action word, through the catalogue (French and the rest come from Localizable.xcstrings).
    public var title: String { Self.title(for: kind) }
    public static func title(for kind: SuggestionKind) -> String {
        switch kind {
        case .blockSender: return Words.text("Block sender")
        case .alwaysJunkDomain: return Words.text("Always junk this domain")
        case .unsubscribe: return Words.text("Unsubscribe")
        case .muteThread: return Words.text("Mute thread")
        case .alwaysLabel: return Words.text("Always label")
        }
    }
    public static var dismissWord: String { Words.text("Dismiss suggestion") }
    /// The core's own line for this suggestion (address, domain, list or thread): the tooltip.
    public var detail: String { suggestionWords(kind: kind, subject: subject) }
}

/// What the keys and clicks do. Every path ends in one core command; nothing is written here.
@MainActor
enum SuggestionActs {
    /// The one suggestion on offer: the core lists newest first and the bar shows only the first.
    static func current(_ m: AppModel) -> SuggestionOffer? { m.core.suggestions().first.map(SuggestionOffer.init) }

    @discardableResult
    static func accept(_ m: AppModel) -> Bool {
        guard let o = current(m) else { return false }
        return m.perform(.acceptSuggestion(suggestionId: o.id))
    }
    @discardableResult
    static func dismiss(_ m: AppModel) -> Bool {
        guard let o = current(m) else { return false }
        return m.perform(.dismissSuggestion(suggestionId: o.id))
    }
    /// A letter takes the suggestion only when it is the one on offer and the main list holds the keys.
    static func key(_ chord: String, _ m: AppModel) -> Bool {
        guard m.surface == "mac-main", m.overlay == nil, let o = current(m), o.key == chord.uppercased() else { return false }
        return accept(m)
    }
}
