import SwiftUI

/// Counts the live window views: it must read 0 once the window is closed (the speed rule: free what is not on screen).
final class SettingsLifeProbe {
    nonisolated(unsafe) static var alive = 0
    init() { Self.alive += 1 }
    deinit { Self.alive -= 1 }
}

/// The Settings window: the content exists only while the window is on screen. Closing it drops every view and its state; the
/// models behind them are small and stay (the speed rule, owner 2026-11-08 14:46).
struct SettingsWindowView: View {
    @State private var live = true
    var body: some View {
        // The lifecycle hooks sit on a container that stays, never on the branch they switch (that would bring it back at once).
        ZStack {
            if live { SettingsWindowContent() } else { Rectangle().fill(WhooshPalette.background) }
        }
        .onAppear { live = true }
        .onDisappear { live = false; SettingsWindowModel.shared.releaseTyped() }
    }
}

/// A sidebar of panes (with search) at the left, the pane at the right under a slim toolbar with back and forward.
/// System Settings' own layout in Native Glass materials (picks SB and S3, owner 2026-11-08).
struct SettingsWindowContent: View {
    @State private var probe = SettingsLifeProbe()
    @State private var model = SettingsWindowModel.shared
    @State private var app = AppModel.shared
    @State private var ai = AISettingsStore.shared
    @State private var accounts = AccountsSession.shared

    var body: some View {
        GeometryReader { geo in
            HStack(spacing: 0) {
                SettingsSidebar(showsSummary: SettingsMetrics.showsSummary(forHeight: geo.size.height, repliesOpen: model.repliesOpen))
                    .frame(width: SettingsMetrics.sidebarWidth(for: geo.size.width))
                SettingsDetail()
            }
        }
        .frame(idealWidth: SettingsMetrics.windowDefault.width, idealHeight: SettingsMetrics.windowDefault.height)
        .ignoresSafeArea()
        .background(WhooshPalette.background)
        .background { SettingsKeys(model: model) }
        .addAccountSheet(isPresented: $accounts.adding)
        #if os(macOS)
        .background(SettingsWindowChrome())
        #endif
        .onChange(of: app.settingsPage) { _, slot in model.follow(slot: slot) }
        .onChange(of: ai.section) { _, section in
            switch section {
            case .providers, .agents: if model.pane != .ai { model.select(.ai) }
            case .decisions, .page: if model.pane != .decisions { model.select(.decisions) }
            }
        }
        // The shared buffer belongs to whoever holds the keys: a closed or unfocused Settings window takes nothing (docs/lanes/typing-lost-r2.md).
        .onChange(of: app.typed) { _, typed in
            guard SettingsWindowModel.holdsKeys(app) else { model.releaseTyped(); return }
            model.absorbTyped(typed)
        }
        .onAppear {
            accounts.reload()
            RepliesRouter.go = { _ = SettingsWindowModel.shared.openReply($0) }
            RepliesRouter.current = { SettingsWindowModel.shared.place.reply ?? .auto }
            model.follow(slot: app.settingsPage)
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("mac-settings-window")
    }
}

/// The part at the right: the toolbar (back, forward, the place, the keys of the two moves) over the pane.
struct SettingsDetail: View {
    @State private var model = SettingsWindowModel.shared
    @State private var accounts = AccountsSession.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(spacing: 0) {
            SettingsToolbar(model: model)
            SettingsPaneSwitch(place: model.place)
                .id(model.place)
                .transition(.opacity.combined(with: .blurReplace))
        }
        .padding(.top, model.titlebarHeight)   // the window runs its content under the title bar; the sidebar clears the traffic lights, the toolbar row clears the band
        #if os(macOS)
        .overlay {
            if let id = accounts.editing, let account = accounts.entry(id) {
                AccountIconPicker(session: accounts, account: account)
                    .transition(.opacity.combined(with: .scale(scale: CGFloat(WhooshConstants.POPOVER_SCALE_FROM))))
                    .zIndex(1)
            }
        }
        .animation(WhooshMotion.animation(WhooshMotion.readerSwap, reduceMotion: reduceMotion), value: accounts.editing)
        #endif
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(WhooshPalette.background)
        .animation(WhooshMotion.animation(WhooshMotion.readerSwap, reduceMotion: reduceMotion), value: model.place)
    }
}

/// `.tb` the toolbar: a glass capsule with back and forward, the place in words, the two keys at the right.
struct SettingsToolbar: View {
    let model: SettingsWindowModel
    private let accounts = AccountsSession.shared

    var body: some View {
        HStack(spacing: 12) {
            HStack(spacing: 0) {
                navButton("chevron.left", ax: "mac-settings-window.back", key: "⌘[", enabled: model.canGoBack, word: Words.text("Back")) { _ = model.goBack() }
                navButton("chevron.right", ax: "mac-settings-window.forward", key: "⌘]", enabled: model.canGoForward, word: Words.text("Forward")) { _ = model.goForward() }
            }
            .padding(.horizontal, 3).whooshGlass(in: Capsule())
            titleText.font(.system(size: 15, weight: .semibold)).foregroundStyle(WhooshPalette.label).lineLimit(1)
                .frame(maxWidth: .infinity, alignment: .leading)
                .whooshAx("mac-settings-window.title")
            HStack(spacing: 4) { SettingsKey(text: "⌘["); SettingsKey(text: "⌘]") }.opacity(0.9)
        }
        .padding(.horizontal, SettingsMetrics.toolbarSide).padding(.top, 0)
        .frame(height: SettingsMetrics.toolbarHeight)
        .accessibilityElement(children: .contain)
        .whooshAx("mac-settings-window.toolbar")
    }

    /// hid-ai round 24 (main114, board ai-replies-r7 FILT: « Brouillon automatique › » muted and regular, then « Quels messages »): a page under a Replies part
    /// names its part in the secondary colour and regular weight, then itself; every other place reads as one word, as before. The words are `title`'s.
    private var titleText: Text {
        if model.accountPage == nil, model.pane == .replies, let page = model.place.reply, let parent = page.parent {
            return Text(parent.title + " › ").fontWeight(.regular).foregroundStyle(WhooshPalette.secondaryLabel) + Text(page.title)
        }
        return Text(title)
    }

    private var title: String {
        if let id = model.accountPage, let a = accounts.entry(id) { return model.pane.title + " › " + a.name }
        if model.pane == .replies { return model.place.reply?.path ?? RepliesPage.auto.title }
        return model.pane.title
    }

    private func navButton(_ symbol: String, ax: String, key: String, enabled: Bool, word: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol).font(.system(size: 13, weight: .semibold))
                .foregroundStyle(enabled ? WhooshPalette.label : WhooshPalette.tertiaryLabel)
                .frame(width: SettingsMetrics.navButton, height: SettingsMetrics.navButton).contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help("\(word) (\(key))")
        .accessibilityLabel(word)
        .whooshAx(ax)
    }
}

/// The pane a place names: the four built panes, the two panes of the AI folder, and the placeholders.
struct SettingsPaneSwitch: View {
    let place: SettingsWindowModel.Place
    var body: some View {
        switch place.pane {
        case .general: GeneralPaneView()
        case .accounts:
            #if os(macOS)
            if let id = place.account { AccountPageView(accountId: id) } else { AccountsPaneView() }
            #else
            SettingsPlaceholderPane(pane: .accounts)
            #endif
        case .decisions: DecisionsPaneView()
        case .ai: AIAgentsPaneView()
        case .outside: McpSettingsPane()
        case .agendas: CalendarsPaneView()
        case .replies: RepliesPaneSwitch(page: place.reply ?? .auto)
        case .notifications: NotificationsPaneView()
        case .shortcuts: ShortcutsPaneView()
        case .viewing, .signatures, .junk: SettingsPlaceholderPane(pane: place.pane)
        }
    }
}

/// The window's keys: ⌘1 to ⌘0 for the panes, ⌘F for the search, ⌘[ and ⌘] for back and forward, ⌘N for Add account.
/// Hidden buttons carry them, so the real keyboard and the control socket run the same actions.
struct SettingsKeys: View {
    let model: SettingsWindowModel
    var body: some View {
        ZStack {
            Button("") { model.requestSearch() }.keyboardShortcut("f", modifiers: .command)
            Button("") { _ = model.goBack() }.keyboardShortcut("[", modifiers: .command)
            Button("") { _ = model.goForward() }.keyboardShortcut("]", modifiers: .command)
            if model.pane == .accounts && model.accountPage == nil {
                Button("") { AccountsSession.shared.adding = true }.keyboardShortcut("n", modifiers: .command)
            }
        }
        .opacity(0).frame(width: 0, height: 0).accessibilityHidden(true)
        #if os(macOS)
        .background(SettingsDigitKeys(model: model))
        #endif
    }
}

#if os(macOS)
import AppKit

/// ⌘1 to ⌘0 select the panes by the PHYSICAL digit key (owner 2026-11-10: "cmd+1, cmd+2 doesnt work still on azerty (as it is
/// cmd+&, cmd+é, etc.)"; SPEC A107). A button's `keyboardShortcut` matches the character the layout types, so ⌘& and ⌘é never
/// matched ⌘1 and ⌘2. This local monitor reads the event through ChordKit.chord, the key-code path the main window uses (codes
/// 18 to 29 are the digit row on every layout), and answers only for its own window.
struct SettingsDigitKeys: NSViewRepresentable {
    let model: SettingsWindowModel
    func makeNSView(context: Context) -> Probe { let v = Probe(); v.model = model; return v }
    func updateNSView(_ view: Probe, context: Context) { view.model = model }

    final class Probe: NSView {
        var model: SettingsWindowModel?
        nonisolated(unsafe) private var monitor: Any?

        override func viewDidMoveToWindow() {
            if let m = monitor { NSEvent.removeMonitor(m); monitor = nil }
            guard window != nil else { return }
            monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] e in
                guard let self, let w = self.window, e.window === w else { return e }
                return MainActor.assumeIsolated { self.take(e) } ? nil : e
            }
        }

        @MainActor private func take(_ e: NSEvent) -> Bool {
            let chord = ChordKit.chord(e)
            guard !chord.isEmpty, let pane = SettingsPane.allCases.first(where: { $0.chord == chord }) else { return false }
            return model?.select(pane) ?? false
        }

        deinit { if let m = monitor { NSEvent.removeMonitor(m) } }
    }
}

/// The window's chrome: the title bar goes quiet and the content runs under it; the traffic lights sit on the
/// sidebar. Default 940 x 680, never smaller than 700 x 480.
struct SettingsWindowChrome: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView { Probe() }
    func updateNSView(_ view: NSView, context: Context) {}

    private final class Probe: NSView {
        /// The windows already sized: a language switch rebuilds this view, and must not put a window the owner resized back to 940 x 680.
        private static let sized = NSHashTable<NSWindow>.weakObjects()

        nonisolated(unsafe) private var observers: [NSObjectProtocol] = []

        override func viewDidMoveToWindow() {
            guard let w = window else { return }
            style(w)
            // SwiftUI sets the window's own chrome again after the view arrives and when the window changes state: say it again then.
            DispatchQueue.main.async { [weak self] in self?.window.map { self?.style($0) } }
            observers.forEach(NotificationCenter.default.removeObserver)
            observers = [NSWindow.didBecomeKeyNotification, NSWindow.didResignKeyNotification, NSWindow.didResizeNotification].map { name in
                NotificationCenter.default.addObserver(forName: name, object: w, queue: .main) { [weak self] _ in self?.window.map { self?.style($0) } }
            }
            guard !Self.sized.contains(w) else { return }
            Self.sized.add(w)
            w.setContentSize(SettingsMetrics.windowDefault)
            w.center()
        }

        deinit { observers.forEach(NotificationCenter.default.removeObserver) }

        private func style(_ w: NSWindow) {
            if w.titleVisibility != .hidden { w.titleVisibility = .hidden }
            if !w.titlebarAppearsTransparent { w.titlebarAppearsTransparent = true }
            if w.titlebarSeparatorStyle != .none { w.titlebarSeparatorStyle = .none }
            if !w.styleMask.contains(.fullSizeContentView) { w.styleMask.insert(.fullSizeContentView) }
            if !w.isMovableByWindowBackground { w.isMovableByWindowBackground = true }
            if w.minSize != SettingsMetrics.windowMin { w.minSize = SettingsMetrics.windowMin }
            SettingsWindowModel.shared.setTitlebar(max(0, w.frame.height - w.contentLayoutRect.height))
        }
    }
}
#endif
