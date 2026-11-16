import SwiftUI

/// The reader's door for two keys: ⇧H expands or collapses the marked message's header (SPEC A70); ⌘U leaves the
/// mailing list through the message's List-Unsubscribe (delta 10) after an in-pane confirm that says exactly what will
/// happen. ⇧E (mark not done) is the inbox list's command alone (TriageKeys.markNotDone): one owner per command. Each returns false
/// when it has nothing to do, so the key passes on. No system alert: the confirm is pane state (hidden rigs included).
@MainActor
enum ReaderListActions {
    /// ⇧H: the marked message (N and P), else the open one.
    static func toggleHeader(_ m: AppModel) -> Bool {
        guard let t = ReaderKeys.thread(m), let id = ReaderKeys.messageId(t, ReaderState.shared.focused(t)) else { return false }
        withAnimation(ReaderHeaderMotion.keyAnimation) { ReaderState.shared.toggleHeader(id) }
        return true
    }

    /// The id of the message whose one-click request is on its way (a second Enter meanwhile sends nothing).
    private(set) static var sending: String?
    /// The pages the confirm opened for a web-link list, in order: the drive reads them (the fixture host opens nothing).
    private(set) static var opened: [String] = []
    /// How many times the confirm asked the engine to unsubscribe (the drive reads it: a key before the confirm, or a second ⌘U, adds none).
    private(set) static var calls = 0

    /// ⌘U: arms the confirm for the shown message when it comes from a list. False when it does not. A list already left (the core keeps
    /// the mark and its date) arms nothing and sends nothing: the pane says when it was left.
    static func unsubscribe(_ m: AppModel) -> Bool {
        guard let t = ReaderKeys.thread(m), let how = m.core.unsubscribeMethod(messageId: t.messageId) else { return false }
        ReaderState.shared.actionNote = nil
        if m.core.state(path: "message/\(t.messageId)/unsubscribed") == "true" {
            ReaderState.shared.unsubscribe = nil
            ReaderState.shared.actionNote = leftWords(m, t.messageId)
            return true
        }
        ReaderState.shared.unsubscribe = (t.messageId, how)
        return true
    }

    /// "Unsubscribed on <date>" from the core's `unsubscribed_at` (milliseconds); just "Unsubscribed" when the core gives no date.
    static func leftWords(_ m: AppModel, _ id: String) -> String {
        guard let at = m.core.state(path: "message/\(id)/unsubscribed_at").flatMap({ Double($0) }) else { return Words.text("Unsubscribed") }
        let f = DateFormatter()
        f.locale = Locale(identifier: Words.current); f.dateStyle = .long; f.timeStyle = .none
        return Words.text("Unsubscribed on %@", f.string(from: Date(timeIntervalSince1970: at / 1000)))
    }

    /// The confirm's primary button (and Enter while it shows): the one place the engine is asked. A one-click request goes over the
    /// network, so it runs off the main actor and the pane says "Unsubscribing" at once; a mail link (queued) or a web link (marked) is local.
    /// After a web link is marked the page opens in the browser, from here: the engine never opens one.
    static func confirmUnsubscribe(_ m: AppModel) -> Bool {
        guard let u = ReaderState.shared.unsubscribe else { return false }
        ReaderState.shared.unsubscribe = nil
        guard sending == nil else { return true }
        if case .oneClick = u.method, m.host.kind != .fixture {
            sending = u.messageId
            calls += 1
            ReaderState.shared.actionNote = Words.text("Unsubscribing")
            let core = m.core, id = u.messageId
            Task { @MainActor in
                let failure: String? = await Task.detached {
                    do { _ = try core.run(command: .unsubscribe(messageId: id)); return nil }
                    catch CoreError.Action(let f) { return f.reasonWords }
                    catch { return "\(error)" }
                }.value
                sending = nil
                finish(m, u, ok: failure == nil, failure: failure)
            }
            return true
        }
        calls += 1
        finish(m, u, ok: m.perform(.unsubscribe(messageId: u.messageId)), failure: nil)
        return true
    }

    private static func finish(_ m: AppModel, _ u: (messageId: String, method: UnsubscribeMethod), ok: Bool, failure: String?) {
        guard ok else {
            if let failure { m.lastFailure = ActionFailed(accountName: "Whoosh", reasonWords: failure, retryable: true); m.refresh() }
            ReaderState.shared.actionNote = nil
            return
        }
        ReaderState.shared.actionNote = Words.text("Unsubscribed")
        if case .link(let page) = u.method { open(page) }
    }

    /// The list's own page, in the system browser (never a web view); http and https only. The fixture host logs it and starts nothing.
    private static func open(_ page: String) {
        guard let url = URL(string: page), let scheme = url.scheme?.lowercased(), scheme == "https" || scheme == "http" else { return }
        opened.append(page)
        #if canImport(AppKit)
        if CoreHost.shared.kind != .fixture { NSWorkspace.shared.open(url) }
        #endif
    }

    /// The confirm's Cancel (and Escape while it shows).
    static func cancelUnsubscribe(_ m: AppModel) -> Bool {
        guard ReaderState.shared.unsubscribe != nil else { return false }
        ReaderState.shared.unsubscribe = nil
        return true
    }

    /// What the confirm says will happen, per method (ux-canon: target, content, scope before the fire).
    static func consequence(_ how: UnsubscribeMethod) -> String {
        switch how {
        case .oneClick(let url): return Words.text("Whoosh sends the list's one-click request to %@. No page opens.", host(url))
        case .mail(let address, _): return Words.text("Whoosh sends a message to %@ from the address that receives this list.", address)
        case .link(let url): return Words.text("The list's page opens in your browser: %@", host(url))
        }
    }

    private static func host(_ url: String) -> String { URL(string: url)?.host ?? url }
}

/// The in-pane confirm for ⌘U: what happens, then Cancel (Esc) and Unsubscribe (Enter), each with its key cap.
struct UnsubscribeConfirm: View {
    let sender: String
    let how: UnsubscribeMethod
    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            Text(Words.text("Unsubscribe from %@?", sender)).whooshText(WhooshTypography.body, emphasized: true)
                .foregroundStyle(WhooshPalette.label)
            Text(ReaderListActions.consequence(how)).whooshText(WhooshTypography.caption)
                .foregroundStyle(WhooshPalette.secondaryLabel).fixedSize(horizontal: false, vertical: true)
            HStack(spacing: WhooshSpacing.s8) {
                Spacer(minLength: 0)
                Button { _ = ReaderListActions.cancelUnsubscribe(AppModel.shared) } label: { KeyHint("Esc", Words.text("Cancel")) }
                    .buttonStyle(.plain).whooshAx("mac-main.reader.unsubscribe.cancel")
                Button { _ = ReaderListActions.confirmUnsubscribe(AppModel.shared) } label: { KeyHint("↩", Words.text("Unsubscribe")) }
                    .buttonStyle(.plain).whooshAx("mac-main.reader.unsubscribe.confirm")
            }
        }
        .padding(WhooshSpacing.s14)
        .background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: WhooshRadius.sheet, style: .continuous))
        .frame(maxWidth: WhooshMetrics.readerMeasure, alignment: .leading)
        .accessibilityIdentifier("mac-main.reader.unsubscribe")
    }
}

/// Under the header band: the ⌘U confirm while it waits, else the last ⇧E / ⌘U outcome in words.
struct ReaderActionNotice: View {
    let sender: String
    @State private var state = ReaderState.shared  // local-ui: the confirm and the note, observed
    var body: some View {
        Group {
            if let u = state.unsubscribe {
                UnsubscribeConfirm(sender: sender, how: u.method).transition(.opacity)
            } else if let note = state.actionNote {
                Text(note).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                    .accessibilityIdentifier("mac-main.reader.note")
            }
        }
        .padding(.horizontal, WhooshMetrics.readerSide)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
