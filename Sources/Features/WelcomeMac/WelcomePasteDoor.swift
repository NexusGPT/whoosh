import Foundation

/// The rig's `fixture paste-accounts <text...>` (docs/CONTROL-SOCKET.md): account lines, or a fixture name, pasted into Welcome. The verb only posts
/// `whoosh.fixture.paste-accounts` (FixtureEvents), so the Welcome feature has to hear it; before this nobody did and the list stayed empty.
/// The fixture name `mac-main-sample` shows the accounts the core already holds (WelcomeModel.showCoreAccounts); any other text is a pasted list of
/// addresses, one row each (the paste area's own path). Fixture core only: the owner's copy never listens. Started by the feature's first
/// table (WelcomeMacFeature.clickTargets / rowSources), which the rig's `click mac-welcome-accounts.reset` reaches before any paste.
@MainActor enum WelcomePasteDoor {
    static let sampleName = "mac-main-sample"
    private static var token: NSObjectProtocol?

    static func listen() {
        guard token == nil, CoreHost.shared.kind == .fixture else { return }
        token = NotificationCenter.default.addObserver(forName: Notification.Name("whoosh.fixture.paste-accounts"), object: nil, queue: .main) { n in
            let args = (n.userInfo?["args"] as? [String]) ?? []
            MainActor.assumeIsolated { paste(args) }
        }
    }

    static func paste(_ args: [String]) {
        let w = WelcomeModel.shared
        AppModel.shared.surface = WelcomeMacFeature.W
        _ = WindowBridge.open(SurfaceSlot(WelcomeMacFeature.W), in: AppModel.shared)   // the window draws the rows: the socket's `drag` finds a row by its drawn frame
        if args == [sampleName] { w.showCoreAccounts(); return }
        w.pasteText = args.joined(separator: " ")
        w.addPasted()
    }
}
