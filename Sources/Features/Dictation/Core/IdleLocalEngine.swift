import Foundation

/// The local recogniser as the app holds it: nothing loads at launch, the library and model load on the first dictation, and both
/// are let go after `idleSeconds` without one (the embedding model's `IdleHolder`, one number for both: WhooshConstants.EMBED_IDLE_UNLOAD_S).
/// While nothing is loaded no timer runs, so an idle app spends no CPU and no memory here.
///
/// Pressing the key never waits for the load: recording starts at once, the live session is made as soon as the model is up, and the
/// samples heard meanwhile are fed to it in order. The final text comes from the file pass in `transcribe`, which waits for the load.
public final class IdleLocalEngine: LiveTranscriber, @unchecked Sendable {
    public let name = "parakeet.cpp"
    let holder: IdleHolder<any LiveTranscriber>
    private let make: @Sendable () throws -> any LiveTranscriber

    /// - Parameters:
    ///   - make: loads the recogniser (the library and the model); it throws `DictationError.engineUnavailable` when either is not on the device.
    ///   - now, autoSweep: the clock and the sweeper, replaced by a test (`IdleHolder`).
    public init(idleSeconds: Int = WhooshConstants.EMBED_IDLE_UNLOAD_S,
                now: @escaping @Sendable () -> TimeInterval = { ProcessInfo.processInfo.systemUptime },
                autoSweep: Bool = true,
                make: @escaping @Sendable () throws -> any LiveTranscriber) {
        holder = IdleHolder(idleSeconds: idleSeconds, now: now, autoSweep: autoSweep)
        self.make = make
    }

    public var isLoaded: Bool { get async { await holder.isLoaded } }

    func engine() async throws -> any LiveTranscriber {
        let make = self.make
        return try await holder.get { try await Task.detached(priority: .userInitiated) { try make() }.value }
    }

    public func transcribe(audio: URL) async throws -> String {
        let e = try await engine()
        do {
            let text = try await e.transcribe(audio: audio)
            await holder.touch()   // a long recognition is not idleness: the idle time counts from its end
            return text
        } catch {
            await holder.touch()
            throw error
        }
    }

    public func beginLive() -> LiveSession? { LazyLive(owner: self) }

    /// A live session that exists before its model does: samples wait here until the model is up, then go in, in order.
    final class LazyLive: LiveSession, @unchecked Sendable {
        private let lock = NSLock()
        private var pending: [Float] = []
        private var inner: LiveSession?
        private var carry = ""
        private var finished = false

        init(owner: IdleLocalEngine) {
            Task.detached { [weak self] in
                guard let e = try? await owner.engine() else { return }   // no model: the file pass says so in words
                self?.attach(e)
            }
        }

        private func attach(_ e: any LiveTranscriber) {
            lock.lock(); defer { lock.unlock() }
            guard !finished, let s = e.beginLive() else { return }
            inner = s
            if !pending.isEmpty { carry = s.feed(pending); pending = [] }
        }

        func feed(_ pcm: [Float]) -> String {
            lock.lock(); defer { lock.unlock() }
            guard let s = inner else { if !finished { pending += pcm }; return "" }
            let words = [carry, s.feed(pcm)].filter { !$0.isEmpty }.joined(separator: " ")
            carry = ""
            return words
        }

        func finish() -> String {
            lock.lock(); defer { lock.unlock() }
            finished = true
            pending = []
            guard let s = inner else { return "" }   // the model was still loading: the file pass gives the text
            inner = nil
            return [carry, s.finish()].filter { !$0.isEmpty }.joined(separator: " ")
        }
    }
}
