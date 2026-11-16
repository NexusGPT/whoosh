import SwiftUI

/// The main window's root: whatever feature claims the `mac-main` slot (the macshell lane's root,
/// which mounts the sidebar, list and reader inside it). Until one does, the stub panes below.
public struct RootView: View {
    @State private var model: AppModel = { LaunchClock.once("root-state-init"); return AppModel.shared }()   // perf-archive r4: a launch phase
    @State private var language = Words.selected   // settingsgeneral hook: the whole window redraws in the new language at once
    @Environment(\.openWindow) private var openWindow
    public init() { LaunchClock.once("root-init") }   // perf-archive r3: a launch phase (one Bool read once the launch is over)

    public var body: some View {
        let _ = LaunchClock.once("root-body")   // launch-r3: the main window's first body (the scene and the window exist)
        let shell = Registrations.view(for: .macMain)
        let _ = LaunchClock.once("root-shell-view")   // perf-archive r3: the features' registry asked for the main slot
        Group {
            if let shell { shell } else { Self.defaultShell(model) }
        }
        .id(language)
        .onReceive(NotificationCenter.default.publisher(for: Words.languageDidChange)) { _ in language = Words.selected }
        .overlay { Registrations.view(for: .macOverlay) ?? Self.defaultOverlay }
        .overlay { AddAccountSheetHost() }   // addaccount r2 hook: the Add account sheet over the main window while the surface is mac-add-account
        .onAppear {
            LaunchClock.trace("first-appear")
            if model.launchMs == nil { model.launchMs = LaunchClock.msSinceProcessStart() }
            if model.firstFrameMs == nil {
                DispatchQueue.main.async {
                    if model.firstFrameMs == nil { model.firstFrameMs = LaunchClock.msSinceProcessStart() }
                    LaunchClock.trace("first-frame")
                    model.settle()                                          // the first read of the core and the change subscription
                    DispatchQueue.main.async { AppLaunch.afterFirstFrame() }   // push client and notch: a later turn, the frame is already out
                }
            }
            WindowBridge.openWindow = openWindow
            ControlBootstrap.startOnce()
            LaunchClock.trace("control-socket-end")
        }
    }
}

extension RootView {
    /// The macshell lane's root on the Mac (docs/lanes/macshell.md "## Blocked on the root" 1); elsewhere the stub.
    @ViewBuilder static func defaultShell(_ model: AppModel) -> some View {
        #if os(macOS)
        MacShellRoot()
        #else
        InboxPhoneRoot().onAppear { model.surface = "iphone-inbox" }   // inboxphone gap 1
        #endif
    }
}

extension RootView {
    /// The overlay slot's content until a feature claims `mac-main.overlay`: the trainer lane's hint and
    /// sheet (TrainerHintView, TrainerOverlayView), so a mouse archive shows the hint over the real inbox.
    /// The hint docks at the window's bottom-trailing corner (the reader's foot): centred, it straddled the
    /// list/reader divider beside the list's last row (seen on the rig). The shell cannot see the list column's frame.
    static var defaultOverlay: AnyView {
        #if os(macOS)
        AnyView(ZStack {
            TrainerHintView()
                .padding(.bottom, WhooshMetrics.footStrip + WhooshSpacing.s8)
                .padding(.trailing, WhooshSpacing.s16)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
            TrainerOverlayView()
        })
        #else
        AnyView(EmptyView())
        #endif
    }
}

/// The root's own three panes, drawn only while no feature claims `mac-main`. No look: a stub.
struct StubPanes: View {
    let model: AppModel

    var body: some View {
        HStack(spacing: 0) {
            Color.clear.frame(width: 220)
                .accessibilityIdentifier("mac-main.sidebar")
            Divider()
            VStack(alignment: .leading, spacing: 4) {
                ForEach(Array(model.rows.enumerated()), id: \.element.messageId) { i, r in
                    Text("\(r.senderName)  \(r.subject)")
                        .lineLimit(1)
                        .fontWeight(i == model.focusIndex ? .bold : .regular)
                        .accessibilityIdentifier("mac-main.list.row.\(i)")
                }
                Spacer()
            }
            .padding(8)
            .frame(minWidth: 360, maxHeight: .infinity, alignment: .topLeading)
            .whooshAx("mac-main.list")
            Divider()
            Color.clear.frame(minWidth: 360)
                .accessibilityIdentifier("mac-main.reader")
        }
        .frame(minWidth: 960, minHeight: 600)
        .whooshAx("mac-main.stub")
    }
}

/// Wiring that waits for the first frame: the push client and the notch feed. Neither is on the way to the first paint
/// (docs/lanes/launch-r1.md: the first frame holds the main window only, an idle app keeps no panel).
@MainActor
enum AppLaunch {
    private static var done = false
    #if os(macOS)
    /// Held for the life of the app: the push subscription and its listener.
    private static var push: (router: ArrivalRouter, listener: ArrivalListener, subscription: UInt64)?
    #endif

    /// Called from the main queue's turn after `firstFrameMs` is set and the core has been read (RootView.onAppear), never before.
    static func afterFirstFrame() {
        guard !done else { return }
        done = true
        #if os(macOS)
        let started = PushClientMac.start(core: AppModel.shared.host.core)
        push = started
        LaunchClock.trace("settled-push-end")
        NotchFeature.attach(feed: started.router.notch)
        LaunchClock.trace("settled-notch-end")
        SettingsCalendarsMacFeature.start()   // reads the calendar access state; calendars only if already allowed
        SettingsRepliesMacFeature.start()     // hid-ai round 18: untouched shipped reply defaults take the app's language (and follow its changes)
        #endif
    }
}
