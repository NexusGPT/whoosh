#if os(macOS)
import AppKit
import SwiftUI

/// The borderless panel over the notch: a fixed transparent canvas, top-centre on the housing, in which the
/// silhouette grows. It never activates Whoosh, never shows in a screen share or another app's capture
/// (sharingType none), floats over every Space and full-screen app, and lets clicks through everywhere the
/// shape is not (NotchPointer).
///
/// Adapted from the owner's ghostty fork (macos/Sources/Features/OneTerminal/NotchPanel.swift, by the owner,
/// MIT): the window class's one lesson, that a panel claiming it is always key makes AppKit skip the real key
/// handoff and typing never reaches the text view, so `canBecomeKey` is true only while the reply asks for the
/// keyboard (NotchKeyboard); the panel's first look at a mouse event (the option-drag, NotchDrag); the level one
/// above the status window, measured there as the highest band still shown over another app's full-screen space.
final class NotchWindow: NSPanel {
    /// True only while the reply field holds the keyboard.
    var wantsKey = false
    override var canBecomeKey: Bool { wantsKey }
    override var canBecomeMain: Bool { false }
    /// The panel's own event loop gets first look at a mouse event: an option-drag taken here never falls
    /// through to be read as an ordinary click. Everything it does not take goes on as before.
    override func sendEvent(_ event: NSEvent) {
        if MainActor.assumeIsolated({ NotchDrag.shared.take(event, in: self) }) { return }
        super.sendEvent(event)
    }
    /// AppKit keeps windows below the menu bar; the notch's top edge belongs at the top of the screen.
    override func constrainFrameRect(_ frameRect: NSRect, to screen: NSScreen?) -> NSRect { frameRect }
}

/// The view the notch draws into: the first click on a non-activating panel acts, it is not spent waking the window.
final class NotchHostingView: NSHostingView<NotchRoot> {
    override var acceptsFirstResponder: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
}

@MainActor
public final class NotchPanel {
    public static let shared = NotchPanel()
    private(set) var window: NotchWindow?
    private var host: NotchHostingView?
    private let model = NotchModel.shared
    let pointer = NotchPointer()
    /// The stage's forced look for shots; nil follows the system.
    var forcedDark: Bool?
    /// What this panel last asked the window server for. `makeWindow` creates every window `.none`, so a new one starts there.
    /// `NSWindow.sharingType` is not read back for this (NotchSharing.swift, round 7: it stayed none while the server read 1).
    var appliedSharing: NSWindow.SharingType = .none

    /// Creates the panel once and keeps it in step with the model.
    public func install() {
        guard window == nil else { return }
        let w = Self.makeWindow()
        let h = NotchHostingView(rootView: NotchRoot(geometry: geometry))
        h.sizingOptions = []
        w.contentView = h
        window = w
        host = h
        model.onChange = { [weak self] in self?.changed() }
        NotificationCenter.default.addObserver(forName: Words.languageDidChange, object: nil, queue: .main) { _ in
            MainActor.assumeIsolated { NotchPanel.shared.window?.title = NotchIds.windowTitle }   // the title follows the language picked
        }
        NotificationCenter.default.addObserver(forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main) { _ in
            MainActor.assumeIsolated { NotchPanel.shared.place() }
        }
        // the panel stopped being the key window (a click in another app or window): a notch taken with the keyboard closes (NotchModel.keyboardLost).
        // The keyboard's own hand-back (NotchKeyboard.release) passes here too, after the notch already let the keyboard go: nothing happens then.
        NotificationCenter.default.addObserver(forName: NSWindow.didResignKeyNotification, object: nil, queue: .main) { note in
            let resigned = (note.object as AnyObject?).map(ObjectIdentifier.init)
            MainActor.assumeIsolated {
                guard let w = NotchPanel.shared.window, resigned == ObjectIdentifier(w) else { return }
                _ = NotchModel.shared.keyboardLost()
            }
        }
        model.settings = NotchSettings.read(AppModel.shared.core, from: model.settings, seen: &model.seenCore)
        NotchFocus.start(real: AppModel.shared.host.fixture == nil)
        pointer.start(panel: self)
        NotchKeyboard.shared.start()
        place()
    }

    /// Going back to hidden from screen shares while the window is up builds a fresh window, `.none` set before it is
    /// ordered in, and moves the content over (NotchSharing.swift says why). Everything the old window carried goes along.
    func replaceWindow(sharing: NSWindow.SharingType) {
        guard let old = window, let h = host else { return }
        NotchKeyboard.shared.release()
        let w = Self.makeWindow()   // already `.none`
        if sharing != .none { w.sharingType = sharing }
        appliedSharing = sharing
        w.appearance = old.appearance
        w.ignoresMouseEvents = old.ignoresMouseEvents
        old.contentView = nil
        // The retired window can stay in the window server's list (still capturable, state 1) until AppKit releases it,
        // which is after this call returns. Round 8 (main53): the list read [0, 1], the new window and the old one.
        // It leaves the list's "Notch" name, draws nothing and takes no clicks the moment it is retired.
        old.title = ""
        old.alphaValue = 0
        old.ignoresMouseEvents = true
        old.setFrame(NSRect(x: old.frame.minX, y: old.frame.minY, width: 1, height: 1), display: false)
        w.contentView = h
        w.setFrame(old.frame, display: false)
        window = w
        w.orderFrontRegardless()
        old.orderOut(nil)
        old.close()
        NotchTwitch.note("panel.rebuilt", NotchTwitch.box(w.frame))
    }

    var geometry: NotchGeometry {
        guard let s = NotchGeometry.screen else { return NotchGeometry(shape: .none, housing: .zero) }
        return NotchGeometry.read(s, override: model.shapeOverride, fraction: NotchDrag.shared.fraction)
    }

    /// The canvas: the widest stage plus its shoulders, the fixed height, its top edge on the screen's top edge.
    func place() {
        applySharing()   // may replace the window: the one below is read after it
        guard let w = window, let h = host, let screen = NotchGeometry.screen else { return }
        let g = geometry
        w.appearance = NSAppearance(named: .darkAqua)   // look A: the notch is the housing's black in both themes; its words use the dark register
        h.rootView = NotchRoot(geometry: g)
        let width = NotchMetrics.openWidth + 2 * (NotchMetrics.shoulder + NotchMetrics.edge + WhooshSpacing.s16)
        let f = NotchGeometry.frame(width: width, height: NotchMetrics.windowHeight, under: g, screenTop: screen.frame.maxY)
        w.setFrame(Self.parkedIfTesting(f), display: true)
        NotchTwitch.note("panel.frame", NotchTwitch.box(w.frame))
        w.orderFrontRegardless()
    }

    /// A rig copy's panel never sits on a display (owner's fork, `parkedIfTesting`): a drive put the fixture's
    /// chats on the owner's own screen, and they read it as their app misbehaving. It keeps every measurement it
    /// makes on the screen it targets and is drawn that far below it; the window server still composites it there,
    /// which is what a check reads. The rig is the process the socket was started for ($WHOOSH_SOCKET_DIR).
    nonisolated static var parkOffset: CGFloat {
        guard let dir = ProcessInfo.processInfo.environment[ControlServer.socketEnv], !dir.isEmpty else { return 0 }
        return CGFloat(WhooshConstants.NOTCH_RIG_PARK_PT)
    }
    nonisolated static func parkedIfTesting(_ f: NSRect) -> NSRect { f.offsetBy(dx: 0, dy: -parkOffset) }


    /// Every model change: the shape animates in SwiftUI; the canvas only checks it is still in front.
    private func changed() {
        window?.orderFrontRegardless()
        // the keyboard goes back to the app that had it when nothing in the notch asks for it any more: an open mail keeps it (its reply, or
        // the keys the person was using on it); every other stage keeps it only while the person took the notch with the keyboard
        let keeps: Bool
        if case .open = model.stage { keeps = true } else { keeps = model.keyboardWanted && model.stage != .rest }
        if !keeps { NotchKeyboard.shared.release() }
        pointer.refresh()
        if model.stage != .rest { DispatchQueue.main.async { MainActor.assumeIsolated { NotchModel.shared.shown() } } }
    }

    /// The stage for shots: other windows step aside so the socket's `shot` renders this panel.
    func stage(dark: Bool?, shape: NotchShape? = nil) -> Bool {
        install()
        forcedDark = dark
        if let shape { model.shapeOverride = shape }
        for other in (NSApp as NSApplication?)?.windows ?? [] where other !== window { other.orderOut(nil) }
        place()
        window?.displayIfNeeded()
        return true
    }

    public var frame: CGRect? { window?.frame }
}
#endif
