#if os(macOS)
import AppKit
import ObjectiveC

/// The hidden test copy only (HiddenRig.install): an Objective-C exception that ends the app names itself (hid-search round 18). main107's crash
/// reports read `+[NSApplication _crashOnException:]` under `_NSViewLayout` with no reason and no app frame: AppKit catches an exception raised in a
/// layout pass and ends the app itself, so an uncaught-exception handler alone never sees it. Three doors write the same lines: the uncaught-exception
/// handler, `-[NSApplication reportException:]` and AppKit's `+[NSApplication _crashOnException:]`, each then doing what it did before.
/// Written: the exception's name, its reason and the first CRASH_STACK_LINES symbols of the stack it was raised on, to stderr (the launcher keeps it in
/// the run folder's app.out) and to `$WHOOSH_SOCKET_DIR/crash-reason.txt`. Identifiers and AppKit's own words only; nothing of the owner's mail.
enum RigCrashReason {
    static let stackLines = 12
    private nonisolated(unsafe) static var installed = false

    static func install() {
        guard !installed else { return }
        installed = true
        NSSetUncaughtExceptionHandler { e in RigCrashReason.write(e, door: "uncaught") }
        wrapInstance(NSApplication.self, NSSelectorFromString("reportException:"), door: "reportException")
        wrapClass(NSApplication.self, NSSelectorFromString("_crashOnException:"), door: "_crashOnException")
    }

    static func lines(_ e: NSException, door: String) -> String {
        let stack = e.callStackSymbols.prefix(stackLines).joined(separator: "\n")
        return "crash-reason door \(door)\nname \(e.name.rawValue)\nreason \(e.reason ?? "none")\nstack\n\(stack)\n"
    }

    static func write(_ e: NSException, door: String) {
        let text = lines(e, door: door)
        FileHandle.standardError.write(Data(text.utf8))
        guard let dir = ProcessInfo.processInfo.environment["WHOOSH_SOCKET_DIR"], !dir.isEmpty else { return }
        let url = URL(fileURLWithPath: dir).appendingPathComponent("crash-reason.txt")
        if let h = try? FileHandle(forWritingTo: url) {
            h.seekToEndOfFile(); h.write(Data(text.utf8)); try? h.close()
        } else {
            try? Data(text.utf8).write(to: url)
        }
    }

    private typealias Fn = @convention(c) (AnyObject, Selector, NSException) -> Void

    /// Replaces an instance method taking one NSException with one that writes the lines first, then calls the original.
    private static func wrapInstance(_ cls: AnyClass, _ sel: Selector, door: String) {
        guard let m = class_getInstanceMethod(cls, sel) else { return }
        wrap(m, sel, door: door)
    }

    /// The same for a class method.
    private static func wrapClass(_ cls: AnyClass, _ sel: Selector, door: String) {
        guard let m = class_getClassMethod(cls, sel) else { return }
        wrap(m, sel, door: door)
    }

    private static func wrap(_ m: Method, _ sel: Selector, door: String) {
        let original = unsafeBitCast(method_getImplementation(m), to: Fn.self)
        let block: @convention(block) (AnyObject, NSException) -> Void = { target, e in
            RigCrashReason.write(e, door: door)
            original(target, sel, e)
        }
        method_setImplementation(m, imp_implementationWithBlock(block))
    }
}
#endif
