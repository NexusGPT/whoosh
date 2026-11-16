import SwiftUI
#if os(macOS)
import AppKit
#endif

/// A sync status the core published, read by its code: the code (`sync.paused`), its parameters, and the engine's own English words.
/// The app shows ONE catalogue sentence per code in the app's language; the engine's words appear only under Details (a tooltip on
/// the status line, a Details toggle under the step of an added account), never as the sentence itself.
///
/// The code reaches the app as text, because the contract has no field for it: a status is `<English words>\u{1F}<code>` in
/// `SyncState.providerStatusWords`, a failure is the code in `ActionFailed.code` with the words in `reasonWords`. A code is
/// `name?key=value&key=value`, values percent-encoded (core/crates/sync-engine/src/words.rs).
struct SyncStatusWords: Equatable {
    let code: String
    let params: [String: String]
    /// The engine's own words, in English, never translated.
    let engine: String

    static let connecting = "sync.connecting", fetching = "sync.fetching", messages = "sync.messages"
    static let paused = "sync.paused", failed = "sync.failed", slowed = "sync.slowed", lost = "sync.lost"

    /// A status read from `providerStatusWords`; nil when it carries no sync code (an older row, the fixture core's plain words).
    static func status(_ text: String?) -> SyncStatusWords? {
        guard let text, let cut = text.firstIndex(of: "\u{1F}") else { return nil }
        return parse(String(text[text.index(after: cut)...]), engine: String(text[..<cut]))
    }

    /// A failure read from its `ActionFailed.code` and `reasonWords`; nil when the code is not a sync code (an engine or sign-in code, an SMTP code).
    static func failure(code: String?, words: String) -> SyncStatusWords? { code.flatMap { parse($0, engine: words) } }

    private static func parse(_ code: String, engine: String) -> SyncStatusWords? {
        let parts = code.split(separator: "?", maxSplits: 1, omittingEmptySubsequences: false)
        guard let name = parts.first, name.hasPrefix("sync.") else { return nil }
        var params: [String: String] = [:]
        if parts.count == 2 {
            for pair in parts[1].split(separator: "&") {
                let kv = pair.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
                guard kv.count == 2 else { continue }
                params[String(kv[0])] = String(kv[1]).removingPercentEncoding ?? String(kv[1])
            }
        }
        return SyncStatusWords(code: String(name), params: params, engine: engine)
    }

    /// Progress (connecting, fetching, the stored count) is not a problem: the status line shows no warning for it.
    var isProgress: Bool { [Self.connecting, Self.fetching, Self.messages].contains(code) }

    private var count: Int { Int(params["count"] ?? "") ?? 0 }
    private var folder: String { params["folder"] ?? "" }
    private var isInbox: Bool { folder.caseInsensitiveCompare("INBOX") == .orderedSame }

    /// "N messages", in the app's language.
    static func messageCount(_ n: Int) -> String { n == 1 ? Words.text("1 message") : Words.text("%@ messages", n) }

    /// The one catalogue sentence for this code, in the app's language. A code with no sentence of its own is the generic stop.
    var sentence: String {
        switch code {
        case Self.connecting: return Words.text("Connecting…")
        case Self.fetching: return isInbox || folder.isEmpty ? Words.text("Fetching the inbox…") : Words.text("Fetching %@…", folder)
        case Self.messages: return Self.messageCount(count)
        case Self.paused:
            let s = max(1, Int(params["retry_s"] ?? "") ?? 1)
            return s < 120 ? Words.text("Paused. Trying again in %@ s.", s) : Words.text("Paused. Trying again in %@ min.", (s + 59) / 60)
        case Self.failed:
            switch params["step"] ?? "" {
            case "list": return Words.text("Could not list the folders.")
            case "fetch": return isInbox || folder.isEmpty ? Words.text("Could not fetch the inbox.") : Words.text("Could not fetch %@.", folder)
            case "save": return Words.text("Could not save mail on this Mac.")
            default: return Words.text("The sync stopped.")
            }
        case Self.slowed: return Words.text("Slowed: %@", params["provider"] ?? "")
        case Self.lost: return Words.text("New mail is no longer pushed to this Mac.")
        default: return Words.text("The sync stopped.")
        }
    }

    /// "Details: <the engine's words>", the tooltip under a status line; empty when the engine said nothing.
    var details: String { engine.isEmpty ? "" : Words.text("Details: %@", engine) }
}

/// The status line's words, read from the core's sync states at draw time (never a stored copy).
struct MacSyncLine: Equatable {
    var summary: String
    var failing: String?
    var failingAccountId: String?
    /// The engine's own words behind the failing line (its tooltip); nil when there is none.
    var details: String? = nil
    /// "indexing 10 %" or "indexing paused" while the meaning index builds, else nil. It follows the one index model (a change of it
    /// redraws the line); the line is exactly as before when it is nil.
    var indexing: String? = nil

    @MainActor static func read(_ m: AppModel) -> MacSyncLine {
        let indexing = IndexProgress.statusLinePart(IndexProgressModel.shared.status)
        let states = m.core.syncStates()
        guard !states.isEmpty else { return MacSyncLine(summary: "", failing: nil, failingAccountId: nil, indexing: indexing) }
        let healthy = states.filter { $0.lastError == nil }.count
        var summary = Words.text("%@ of %lld accounts synced", healthy, states.count)
        if let last = states.compactMap(\.lastSyncedMs).max() {
            let minutes = max(0, (m.host.clock.nowMs() - last) / 60_000)
            summary += minutes == 0 ? Words.text(" · just now") : Words.text(" · %lld min ago", minutes)
        }
        let bad = states.first { $0.lastError != nil }
        // The core's sentence comes by its code (sync.failed with its step and folder): the catalogue answers in the app's language,
        // the account name stays the owner's, the engine's words stay behind Details. A failure with no sync code (an older row, the
        // fixture core) keeps its words through the catalogue, as before. A failed account that will be tried again says when.
        var details: String?
        let words: String? = bad?.lastError.map { e in
            let coded = SyncStatusWords.failure(code: e.code, words: e.reasonWords)
            details = coded?.details
            let status = SyncStatusWords.status(bad?.providerStatusWords)
            let again = status?.code == SyncStatusWords.paused ? status?.sentence : nil
            return ([e.accountName, coded?.sentence ?? Words.text(e.reasonWords), again]).compactMap { $0 }.joined(separator: " ")
        }
        // No account failed outright, but one says why its sync is not whole ("New mail is paused", "Slowed by Microsoft"): the line
        // shows that sentence after the account's name. Progress (connecting, fetching, the count) is no notice.
        let noted = bad == nil ? states.first { s in
            guard let w = s.providerStatusWords, !w.isEmpty else { return false }
            return !(SyncStatusWords.status(w)?.isProgress ?? false)
        } : nil
        let name = noted.flatMap { n in m.core.accounts().first { $0.id == n.accountId }.map { a in let n = AccountNames.display(a); return n.isEmpty ? a.address : n } }
        let notice: String? = noted?.providerStatusWords.map { w in
            let coded = SyncStatusWords.status(w)
            details = coded?.details
            return [name, coded?.sentence ?? Words.text(w)].compactMap { $0 }.joined(separator: " · ")
        }
        return MacSyncLine(summary: summary, failing: words ?? notice, failingAccountId: bad?.accountId ?? noted?.accountId, details: details, indexing: indexing)
    }

    /// The index fields of the row `mac-main.sync.line`, added to the shell's own fields (MacShellFeature). They are what the one index model
    /// holds: a read never asks the builder and moves nothing on screen.
    @MainActor static func indexFields() -> ControlRow {
        let s = IndexProgressModel.shared.status
        return ["indexing": IndexProgress.isBuilding(s), "index_done": s.indexed, "index_total": IndexProgress.total(s),
                "index_paused": IndexProgress.isPaused(s), "index_text": IndexProgress.statusLinePart(s) ?? NSNull()]
    }

    /// Width the failing line needs against the room it has, in points (the no-clip probe; two lines allowed).
    static func fit(_ text: String, sidebarWidth: CGFloat) -> (text: CGFloat, box: CGFloat) {
        let box = sidebarWidth - MacShellGeometry.syncLeading - MacShellGeometry.syncTrailing
            - WhooshMetrics.warningDot - WhooshSpacing.s6
        #if os(macOS)
        let font = NSFont.systemFont(ofSize: WhooshTypography.caption.size)
        let w = (text as NSString).size(withAttributes: [.font: font]).width
        #else
        let w: CGFloat = 0
        #endif
        return (w, box * 2)
    }
}

/// The sidebar foot: the sync summary, then the failing account as a link to Settings: Accounts.
struct MacStatusLine: View {
    let line: MacSyncLine
    let open: () -> Void
    @State private var hovering = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let indexing = line.indexing {
                // One line, never wrapped: the indexing part stays whole and the accounts part shortens at its end.
                HStack(spacing: 0) {
                    if !line.summary.isEmpty {
                        Text(line.summary).lineLimit(1).truncationMode(.tail)
                        Text(" · ").lineLimit(1).fixedSize()
                    }
                    Text(indexing).lineLimit(1).fixedSize().layoutPriority(1).contentTransition(.numericText())
                }
                .whooshText(WhooshTypography.caption)
                .foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                .accessibilityElement(children: .combine)
                .accessibilityIdentifier("mac-main.status.index")
            } else if !line.summary.isEmpty {
                Text(line.summary)
                    .whooshText(WhooshTypography.caption)
                    .foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                    .lineLimit(2)
                    .fixedSize(horizontal: false, vertical: true)
            }
            if let failing = line.failing {
                Button(action: open) {
                    HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s6) {
                        Circle().fill(WhooshPalette.warning)
                            .frame(width: WhooshMetrics.warningDot, height: WhooshMetrics.warningDot)
                        Text(failing)
                            .whooshText(WhooshTypography.caption)
                            .underline(hovering)
                            .foregroundStyle(WhooshPalette.label)
                            .lineLimit(2)
                            .multilineTextAlignment(.leading)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .onHover { hovering = $0 }
                .help(line.details ?? Words.text("Open Settings: Accounts"))
                .accessibilityIdentifier("mac-main.status.link")
                .accessibilityHint("Opens Settings: Accounts")
                .transition(.opacity)
            }
        }
        // docs/MOTION.md rule 8: a sync state change (failing, healthy again) cross-fades in place.
        .animation(.easeInOut(duration: WhooshMotion.readerCrossFade), value: line)
        .padding(.leading, MacShellGeometry.syncLeading)
        .padding(.trailing, MacShellGeometry.syncTrailing)
        .padding(.bottom, MacShellGeometry.syncBottom)
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("mac-main.sync.line")
    }
}
