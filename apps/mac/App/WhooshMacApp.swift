import SwiftUI
import WhooshUI

/// The Mac shell: nothing but the scene. Everything else lives in the WhooshUI package.
@main
struct WhooshMacApp: App {
    @NSApplicationDelegateAdaptor(KeyMonitor.self) var keys
    /// The rig's hidden mode hooks go in before the first window exists (Sources/App/HiddenRig.swift).
    init() { HiddenRig.install() }
    var body: some Scene { WhooshScene() }
}

/// Real keystrokes take the same path as the socket's `key` verb: the shortcuts engine (ChordKit)
/// names the chord and runs it, and returns nil for an event it took (docs/lanes/shortcuts.md item 2).
final class KeyMonitor: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        LaunchClock.mark("did-finish-launching")   // a launch phase (perf-archive r2): one clock read
        defer { LaunchClock.mark("global-keys-end") }
        NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            // NSEvent is not Sendable and assumeIsolated's closure is: the event crosses in an unchecked box and the
            // closure answers a Bool (taken or not). The local monitor runs on the main thread (lead fix).
            let boxed = UncheckedEvent(event)
            let taken = MainActor.assumeIsolated { ChordKit.handle(boxed.event) == nil }
            return taken ? nil : event
        }
        MainActor.assumeIsolated { GlobalKeys.start() }
    }
}

/// Carries an NSEvent into a Sendable closure on the thread it came from; never stored.
private struct UncheckedEvent: @unchecked Sendable { let event: NSEvent; init(_ e: NSEvent) { event = e } }
