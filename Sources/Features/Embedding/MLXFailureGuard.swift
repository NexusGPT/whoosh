import Foundation
#if canImport(MLX)
import MLX
#endif

/// The model library must never stop the app (the lead, 2026-11-12, from the installed app's log: `MLX/ErrorHandler.swift:345: Fatal error:
/// [METAL] Command buffer execution failed: Caused GPU Timeout Error`). mlx-swift sends an error raised outside a `withError` scope (a Metal
/// command buffer that fails reports from its completion handler, on another thread, where no task-local handler exists) to the process-wide
/// handler, and with none set it calls `fatalError` (ErrorHandler.swift `dispatch`). `install()` sets that process-wide handler ONCE: it records
/// the failure and returns, so the app lives. The code that evaluated the arrays then throws (`check(since:)`) and its caller retries:
/// the index builder retries a failed batch one text at a time after a pause, a text that fails again keeps no vector and is asked again at the
/// next search (it is still missing), and the search answers with words meanwhile (the engine's model-failed stage and its existing sentence).
/// The counter is plain Swift, so the builder and its tests compile without the library.
public enum MLXFailureGuard {
    private final class Box: @unchecked Sendable { let lock = NSLock(); var count = 0; var last = "" }
    private static let box = Box()

    /// How many library failures the handler recorded since launch.
    public static var failures: Int { box.lock.lock(); defer { box.lock.unlock() }; return box.count }

    /// Records one failure (the handler's work; also the seam a test can call without a GPU).
    public static func record(_ message: String) {
        box.lock.lock(); box.count += 1; box.last = message; box.lock.unlock()
    }

    /// Throws `EmbedStepFailure(step: .run)` with the library's words when a failure was recorded after `mark` (a value of `failures`).
    public static func check(since mark: Int) throws {
        box.lock.lock(); let n = box.count, words = box.last; box.lock.unlock()
        if n != mark { throw EmbedStepFailure(step: .run, detail: words) }
    }

    #if canImport(MLX)
    private static let installed: Void = {
        // deprecated in mlx-swift in favour of the scoped `withError`, which cannot reach a completion handler's thread; it is the one hook there
        setErrorHandler({ message, _ in MLXFailureGuard.record(message.map { String(cString: $0) } ?? "") }, data: nil, dtor: nil)
    }()
    #endif

    /// Installs the process-wide handler; every call after the first does nothing. Called before the first model loads.
    public static func install() {
        #if canImport(MLX)
        _ = installed
        #endif
    }
}
