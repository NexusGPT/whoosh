import SwiftUI
#if os(macOS)
import AppKit

/// The badges, drawn above the three panes: one KeyCap in the top trailing corner of each hinted control, fading in on the hold and
/// out on release (opacity only, POPOVER_MS; none under Reduced Motion). It never takes a click.
struct MacHintOverlay: View {
    @State private var hints = MacHintState.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ZStack(alignment: .topLeading) {
            ForEach(hints.strips) { st in
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .fill(WhooshPalette.panel)
                    .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: 1))
                    .shadow(color: .black.opacity(0.14), radius: 4, y: 1)
                    .frame(width: st.pill.width, height: st.pill.height)
                    .position(x: st.pill.midX, y: st.pill.midY)
                ForEach(st.pairs, id: \.cap.ax) { p in
                    Image(systemName: p.symbol).font(.system(size: 11)).foregroundStyle(WhooshPalette.secondaryLabel)
                        .position(x: p.icon.midX, y: p.icon.midY)
                    MacCompactCap(label: p.cap.label).fixedSize().position(x: p.cap.rect.midX, y: p.cap.rect.midY)
                }
            }
            ForEach(hints.badges) { b in
                (b.ax.contains(".act.") ? AnyView(MacCompactCap(label: b.label)) : AnyView(KeyCap(b.label)))
                    .shadow(color: .black.opacity(0.12), radius: 2, y: 1)
                    .fixedSize()
                    .position(x: b.rect.midX, y: b.rect.midY)
                    .accessibilityIdentifier("mac-main.hint.\(b.ax)")
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .opacity(hints.visible ? 1 : 0)
        .animation(reduceMotion ? nil : .easeOut(duration: Double(WhooshConstants.POPOVER_MS) / 1000), value: hints.visible)
        .allowsHitTesting(false)
        .ignoresSafeArea()
    }
}

/// ⌘ held alone starts the timer; any other key or modifier cancels it. One local flags monitor lives for the app; the key monitor
/// exists only while ⌘ is down, so nothing runs per keystroke while ⌘ is up.
///
/// The caps never stay stuck (owner 2026-11-10: "sometimes the cmd stays stuck and I see the shortcuts everywhere, I have to
/// repress cmd to have them disappear"; SPEC A106). The ⌘ key-up is lost when ⌘ leaves with the focus (⌘-Tab, a chord that opens
/// a window or a sheet), so the hold ends on every one of these as well: the app resigning active, any window resigning key, any
/// key or mouse event whose own `modifierFlags` lack ⌘, and the app becoming active while the live `NSEvent.modifierFlags` are not
/// ⌘ alone. The socket's `app-active off|on` runs the same two handlers.
@MainActor
final class MacHintMonitor {
    static let shared = MacHintMonitor()
    private var flags: Any?
    private var keys: Any?
    private var observers: [NSObjectProtocol] = []

    /// Mutant `stuck` only (the check's proof): none of the focus-loss or flag-less-event exits, as before this round.
    private static var stuck: Bool { MacShellGeometry.mutant == "caps-stuck" }

    func start() {
        guard flags == nil else { return }
        flags = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged) { [weak self] e in
            MainActor.assumeIsolated { self?.flagsChanged(e.modifierFlags.intersection(.deviceIndependentFlagsMask)) }
            return e
        }
        let nc = NotificationCenter.default
        observers = [
            nc.addObserver(forName: NSApplication.didResignActiveNotification, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.focusLost() }
            },
            nc.addObserver(forName: NSWindow.didResignKeyNotification, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.focusLost() }
            },
            nc.addObserver(forName: NSApplication.didBecomeActiveNotification, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.recheck() }
            },
        ]
    }

    private func flagsChanged(_ f: NSEvent.ModifierFlags) {
        if f == .command {
            MacHintState.shared.holdBegan()
            if keys == nil {
                keys = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .keyUp, .leftMouseDown, .rightMouseDown, .scrollWheel]) { [weak self] e in
                    MainActor.assumeIsolated { self?.event(e) }
                    return e
                }
            }
        } else {
            cancel()
        }
    }

    /// While ⌘ is down: a key or a click joins it (cancel, as ever), and so does any event that no longer carries ⌘ (the key-up was lost).
    private func event(_ e: NSEvent) {
        let joined = e.type == .keyDown || e.type == .leftMouseDown || e.type == .rightMouseDown
        if joined || (!Self.stuck && !e.modifierFlags.contains(.command)) { cancel() }
    }

    /// The app or a window lost the focus: whatever ⌘ is doing now, no key-up will reach us.
    private func focusLost() {
        guard !Self.stuck else { return }
        cancel()
    }

    /// The app is active again: the live flags say whether ⌘ is still the only key held; if not, nothing is held.
    private func recheck() {
        guard !Self.stuck else { return }
        if NSEvent.modifierFlags.intersection(.deviceIndependentFlagsMask) != .command { cancel() }
    }

    /// The socket's `app-active off|on` (the hidden rig never changes the real focus): runs the handler a real focus change runs,
    /// off = the app resigned active, on = it became active again with the live flags read. The notifications themselves are not
    /// posted: other screens listen to them and a check must not move their state.
    func simulate(active: Bool) {
        if active { recheck() } else { focusLost() }
    }

    /// The socket's four doors for the exits `app-active` does not reach (the hidden rig has no keyboard and no key window). Each runs the
    /// code a real event runs: `simulateHold` is a ⌘-alone flags change (the timer starts and the key monitor is installed), `simulateWindowResign`
    /// is what didResignKey runs, `simulateEventWithoutCommand` sends a real key-up with no flags through the application's event path, where the
    /// monitor installed by the hold reads it.
    func simulateHold() { flagsChanged(.command) }
    func simulateRelease() { flagsChanged([]) }   // ⌘ up: the flags change that carries no ⌘
    func simulateWindowResign() { focusLost() }
    func simulateEventWithoutCommand() {
        let number = NSApp.windows.first(where: { $0.isVisible })?.windowNumber ?? 0
        guard let e = NSEvent.keyEvent(with: .keyUp, location: .zero, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                       windowNumber: number, context: nil, characters: "", charactersIgnoringModifiers: "", isARepeat: false, keyCode: 0) else { return }
        NSApp.sendEvent(e)
    }

    private func cancel() {
        MacHintState.shared.holdEnded()
        if let k = keys { NSEvent.removeMonitor(k); keys = nil }
    }
}
#endif
