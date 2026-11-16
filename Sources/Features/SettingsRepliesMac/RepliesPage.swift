import SwiftUI

/// The parts of Settings › Réponses (layout L2, owner 2026-11-10, docs/design/boards/ai-replies-r7): six panes listed under « Réponses » in
/// the Settings sidebar, and the page « Quels messages » behind the three lines of Brouillon automatique.
enum RepliesPage: String, CaseIterable, Identifiable, Hashable, Sendable {
    case auto, filter, onDemand = "on-demand", sources, retrieval, voice, prompt
    var id: String { rawValue }

    /// The six panes the sidebar lists under « Réponses », in the board's order. The filter page is not listed: it opens from its pane.
    static let panes: [RepliesPage] = [.auto, .onDemand, .sources, .retrieval, .voice, .prompt]

    /// The pane a page lives under: the sidebar keeps that pane selected while the page shows.
    var parent: RepliesPage? { self == .filter ? .auto : nil }
    var title: String {
        switch self {
        case .auto: return Words.text("Auto draft")
        case .filter: return Words.text("Which messages")
        case .onDemand: return Words.text("On demand")
        case .sources: return Words.text("Sources")
        case .retrieval: return Words.text("Retrieval")
        case .voice: return Words.text("Voice")
        case .prompt: return Words.text("The prompt")
        }
    }
    /// The one line under the part's name in its header (the board's words).
    var line: String {
        switch self {
        case .auto: return Words.text("Whoosh writes a draft as soon as a message calls for one. Nothing is sent without you.")
        case .filter: return ""
        case .onDemand: return Words.text("You ask for a draft in a thread, whenever you want.")
        case .sources: return Words.text("What Whoosh may read again to write. Everything stays on this Mac.")
        case .retrieval: return Words.text("How Whoosh finds in your sources what it must know to reply.")
        case .voice: return Words.text("Your way of writing, described once, used in every draft.")
        case .prompt: return Words.text("The text the model reads to write. Type @ to place a variable.")
        }
    }
    /// The toolbar's words: « Brouillon automatique › Quels messages » on the filter page.
    var path: String { parent.map { "\($0.title) › \(title)" } ?? title }
    /// The pane's id on the control socket and in the window's place.
    var ax: String { "\(RepliesAx.P).page.\(rawValue)" }
}

/// The socket names of this feature: one prefix, so the check and the rows agree on every id.
enum RepliesAx {
    nonisolated static let P = "mac-settings-replies"
    nonisolated static func id(_ parts: String...) -> String { ([P] + parts).joined(separator: ".") }
}

/// Where a link goes. The Settings window owns the place (back, forward, the sidebar selection): the integration points `go` and `current`
/// at the window model (docs/lanes/settingsreplies-mac.md "Integration"). Until then the store keeps the page, so a link still lands.
@MainActor
enum RepliesRouter {
    static var go: (RepliesPage) -> Void = { RepliesStore.shared.page = $0 }
    static var current: () -> RepliesPage = { RepliesStore.shared.page }
}

/// The page a place names. The window's pane switch calls this for « Réponses »; each pane reads the store and nothing else.
struct RepliesPaneSwitch: View {
    let page: RepliesPage
    var body: some View {
        Group {
            switch page {
            case .auto: AutoDraftPane()
            case .filter: FilterPage()
            case .onDemand: OnDemandPane()
            case .sources: SourcesPane()
            case .retrieval: RetrievalPane()
            case .voice: VoicePane()
            case .prompt: PromptPane()
            }
        }
        .task(id: page) { RepliesStore.shared.load() }
    }
}
