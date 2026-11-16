import Foundation
import Observation

/// The iPhone search screen's state. The field's text is `AppModel.typed` (the one home the real keyboard and the socket's `type`
/// both write, as on the Mac); everything else is what this screen alone remembers. Four phases, one screen: idle (recent searches),
/// searching (the answer is on its way, the rows already shown stay), results, none found.
/// What a search asks of the core is the Mac's: `Command.search` for plain words, answered on the Search stream, the local engine
/// for filters alone and for a quoted figure (`SearchEngine`, `SearchQuery`, `SearchHitSink` are the Mac's own, read here, not copied).
@MainActor @Observable
public final class SearchPhoneModel {
    public static let shared = SearchPhoneModel()

    public enum Phase: String, Sendable { case idle, searching, results, none }

    /// The screen is up: the surface word `iphone-search` is current, or a thread opened from it is.
    public private(set) var active = false
    public internal(set) var phase = Phase.idle
    /// the rows the list draws: one per thread, best first, every filter applied. The matches, the related mail, then (only while the closed line
    /// "All other mail" is open) the rest, as the Mac's list (SPEC A150).
    public internal(set) var results: [SearchResult] = []
    /// every row of the answer, the rest included: what the counts and the closed line read
    public internal(set) var everyResult: [SearchResult] = []
    /// the closed line "All other mail" is open (closed at every new search)
    public internal(set) var restOpen = false
    /// how the engine made the answer (`search/last/stage`; "words": the other mails are not ranked yet and wait), read once per answer as the Mac does
    public internal(set) var stage = ""
    /// the first answer lists the other mails by date and waits for their ranking (stage "words" with mail that is not a match): the Mac's `tailPending`
    public internal(set) var tailPending = false
    /// "From domain <domain>" rows the typed last word offers (SPEC A153), the Mac's own suggestion rows (`SearchSuggest.build`)
    public internal(set) var domainOffers: [SearchSuggestion] = []
    /// the screen's own hold of the skeleton: the first answer stays unlisted until released (a socket click, as the Mac's `mac-search.skeleton.hold`)
    public internal(set) var holdSkeleton = false
    /// the same rows before the sender filter: the sender picker lists these people
    public internal(set) var candidates: [SearchResult] = []
    var filters = SearchPhoneFilters()
    var menu: SearchPhoneMenu?
    public internal(set) var recents: [String] = []
    public internal(set) var openedMessageId: String?
    /// the mail that holds the passage of the result that was opened: the engine's `hitMessageId`, which is another mail than the opened row's when the passage is
    /// earlier in the thread (SPEC A152: "opened at the mail and the passage that matched"); equal to `openedMessageId` otherwise
    public internal(set) var openedHitMessageId: String?
    /// how many times the meaning model's door was pressed (the screen's own count; the download itself is the AI setup's)
    public internal(set) var doorTaps = 0

    // saving as a smart folder
    public internal(set) var naming = false
    public var name = ""
    public internal(set) var fieldSays = ""
    public internal(set) var savedAs: String?

    var app: AppModel { AppModel.shared }
    var corpus: [InboxRow] = []
    @ObservationIgnored var hits: [SearchHit] = []
    @ObservationIgnored var hitsFor = ""
    @ObservationIgnored var awaiting = ""
    @ObservationIgnored var searchSub: UInt64?
    /// `fixture reset` closes the search through its own close (round 14, phone round 6): the router's reset leaves the stack on the inbox, but the
    /// search model stayed open under it and drew itself back (palettephone `close` read iphone-search after a reset).
    @ObservationIgnored var resetToken: NSObjectProtocol?
    @ObservationIgnored var hitCache: [String: [SearchHit]] = [:]
    /// the rows of each cached answer's hits, one core read per answer (`SearchEngine.rowsOfHits`)
    @ObservationIgnored var hitRowsCache: [String: [InboxRow]] = [:]
    @ObservationIgnored var hitRowsRead: (([String]) -> [InboxRow])?
    @ObservationIgnored var nameCache: [String: [String]] = [:]
    @ObservationIgnored var seenText = ""
    @ObservationIgnored var dirty = false
    @ObservationIgnored var scheduled = false
    @ObservationIgnored var reaskPending = false
    @ObservationIgnored var meaningListening = false
    /// what the last answer said about the typed words (a filter word not understood, no exact match): the line under the header
    public internal(set) var notes: [String] = []
    @ObservationIgnored var exactMissed = false

    private init() {}

    /// The field's text, the one buffer.
    public var text: String { app.typed }

    /// The words the engine reads: the filters, then what is typed. `includingSender: false` is the candidates' reading.
    func engineQuery(includingSender: Bool) -> SearchQuery {
        SearchQuery([filters.words(includingSender: includingSender), app.typed].filter { !$0.isEmpty }.joined(separator: " "))
    }

    /// What a saved folder's rule says: every filter but the account (the rule language has no account), then the typed words.
    var ruleWords: String { engineQuery(includingSender: true).isEmpty ? "" : [filters.words(includingSender: true), app.typed.trimmingCharacters(in: .whitespaces)].filter { !$0.isEmpty }.joined(separator: " ") }

    var isIdleInput: Bool { filters.isEmpty && app.typed.trimmingCharacters(in: .whitespaces).isEmpty }

    // MARK: open and close

    /// Opens the screen. Coming back from a thread it opened keeps the field, the filters and the rows.
    @discardableResult
    public func open() -> Bool {
        if !active {
            corpus = SearchMacModel.everyRow(app)
            hitCache = [:]; hitRowsCache = [:]; nameCache = [:]; hits = []; hitsFor = ""; awaiting = ""
            filters = SearchPhoneFilters(); menu = nil
            naming = false; name = ""; fieldSays = ""; savedAs = nil
            openedMessageId = nil; openedHitMessageId = nil
            app.typed = ""; seenText = ""
            results = []; everyResult = []; restOpen = false; stage = ""; candidates = []; notes = []; phase = .idle
            tailPending = false; domainOffers = []; holdSkeleton = false
            reloadRecents()
            subscribe()
            active = true
        }
        app.overlay = nil
        app.surface = "iphone-search"
        return true
    }

    /// Back to the inbox: the field empties and the rows go. The inbox's pill returns to rest.
    @discardableResult
    public func close() -> Bool {
        app.typed = ""; seenText = ""
        filters = SearchPhoneFilters(); menu = nil
        naming = false; name = ""; fieldSays = ""; savedAs = nil
        results = []; everyResult = []; restOpen = false; stage = ""; candidates = []; notes = []; phase = .idle
        tailPending = false; domainOffers = []; holdSkeleton = false
        awaiting = ""
        active = false
        InboxPhoneState.shared.searching = false
        app.overlay = nil
        app.surface = "iphone-inbox"
        // the router learns it now, not on a view's later change (PhoneRootStack's onChange): until then its stack still says "searching", and its next
        // settle wrote the surface back to iphone-search (phone round 5: close-returns-to-inbox and palettephone's close landed on iphone-search)
        PhoneRouter.shared.searchChanged(active: false)
        return true
    }
}
