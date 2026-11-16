import Foundation

/// Work the owner does not see in the frame an act changes runs after that frame's commit (hid-search round 17, J2.4/one-interaction-budget-ms:
/// 61.8 ms judged on main105 for Enter on a search result). One non-repeating observer of the main run loop's next idle point, ordered after
/// Core Animation's commit (2_000_000) and after the interaction probe's own commit read (2_000_001, InteractionProbe.arm): the state change is
/// on screen first, then this runs, before the main thread waits for the next input (so a socket verb or a key that follows sees it done).
@MainActor
enum SearchAfterCommit {
    static func run(_ work: @escaping @MainActor () -> Void) {
        let o = CFRunLoopObserverCreateWithHandler(nil, CFRunLoopActivity.beforeWaiting.rawValue, false, 2_000_002) { _, _ in
            MainActor.assumeIsolated { work() }
        }
        CFRunLoopAddObserver(CFRunLoopGetMain(), o, .commonModes)
    }
}
