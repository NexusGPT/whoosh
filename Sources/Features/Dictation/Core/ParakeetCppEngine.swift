import Foundation

/// The local engine on the Mac: mudler/parakeet.cpp (MIT, ggml port of NVIDIA Parakeet v3 and Nemotron 3.5; the "speech.cpp" of the brief),
/// loaded at run time through its flat C API (include/parakeet_capi.h, ABI 11) with dlopen, so Package.swift needs no new line.
/// The shared library (`-DPARAKEET_SHARED=ON`) and one GGUF model sit in the app container; nothing here touches the network.
public final class ParakeetCppEngine: LiveTranscriber, @unchecked Sendable {
    typealias Ctx = OpaquePointer
    public let name = "parakeet.cpp"
    private let lock = NSLock()
    private let handle: UnsafeMutableRawPointer
    private let ctx: Ctx
    private let lib: Symbols

    struct Symbols {
        let free: @convention(c) (Ctx?) -> Void
        let transcribePath: @convention(c) (Ctx?, UnsafePointer<CChar>, Int32) -> UnsafeMutablePointer<CChar>?
        let freeString: @convention(c) (UnsafeMutablePointer<CChar>?) -> Void
        let lastError: @convention(c) (Ctx?) -> UnsafePointer<CChar>?
        let streamBegin: @convention(c) (Ctx?) -> OpaquePointer?
        let streamFeed: @convention(c) (OpaquePointer?, UnsafePointer<Float>, Int32, UnsafeMutablePointer<Int32>?) -> UnsafeMutablePointer<CChar>?
        let streamFinalize: @convention(c) (OpaquePointer?) -> UnsafeMutablePointer<CChar>?
        let streamFree: @convention(c) (OpaquePointer?) -> Void
    }

    public init(library: URL, model: URL) throws {
        guard let h = dlopen(library.path, RTLD_NOW | RTLD_LOCAL) else {
            throw DictationError.engineUnavailable("speech library not found at \(library.path)")
        }
        func sym<T>(_ n: String, _ t: T.Type) throws -> T {
            guard let p = dlsym(h, n) else { throw DictationError.engineUnavailable("speech library lacks \(n)") }
            return unsafeBitCast(p, to: t)
        }
        let load = try sym("parakeet_capi_load", (@convention(c) (UnsafePointer<CChar>) -> Ctx?).self)
        lib = Symbols(
            free: try sym("parakeet_capi_free", (@convention(c) (Ctx?) -> Void).self),
            transcribePath: try sym("parakeet_capi_transcribe_path", (@convention(c) (Ctx?, UnsafePointer<CChar>, Int32) -> UnsafeMutablePointer<CChar>?).self),
            freeString: try sym("parakeet_capi_free_string", (@convention(c) (UnsafeMutablePointer<CChar>?) -> Void).self),
            lastError: try sym("parakeet_capi_last_error", (@convention(c) (Ctx?) -> UnsafePointer<CChar>?).self),
            streamBegin: try sym("parakeet_capi_stream_begin", (@convention(c) (Ctx?) -> OpaquePointer?).self),
            streamFeed: try sym("parakeet_capi_stream_feed", (@convention(c) (OpaquePointer?, UnsafePointer<Float>, Int32, UnsafeMutablePointer<Int32>?) -> UnsafeMutablePointer<CChar>?).self),
            streamFinalize: try sym("parakeet_capi_stream_finalize", (@convention(c) (OpaquePointer?) -> UnsafeMutablePointer<CChar>?).self),
            streamFree: try sym("parakeet_capi_stream_free", (@convention(c) (OpaquePointer?) -> Void).self))
        guard let c = load(model.path) else { dlclose(h); throw DictationError.engineUnavailable("speech model would not load: \(model.path)") }
        handle = h; ctx = c
    }

    deinit { lib.free(ctx); dlclose(handle) }

    public func transcribe(audio: URL) async throws -> String {
        try await Task.detached(priority: .userInitiated) { [self] in
            try lock.withLock {
                guard let p = lib.transcribePath(ctx, audio.path, 0) else {
                    throw DictationError.engineFailed(lib.lastError(ctx).map { String(cString: $0) } ?? "transcription failed")
                }
                defer { lib.freeString(p) }
                return String(cString: p).trimmingCharacters(in: .whitespacesAndNewlines)
            }
        }.value
    }

    public func beginLive() -> LiveSession? {
        lock.lock(); defer { lock.unlock() }
        guard let s = lib.streamBegin(ctx) else { return nil }   // an offline-only model gives no live words; the file pass still runs
        return Live(engine: self, stream: s)
    }

    private final class Live: LiveSession, @unchecked Sendable {
        let engine: ParakeetCppEngine
        var stream: OpaquePointer?
        init(engine: ParakeetCppEngine, stream: OpaquePointer) { self.engine = engine; self.stream = stream }
        func feed(_ pcm: [Float]) -> String {
            engine.lock.lock(); defer { engine.lock.unlock() }
            guard let s = stream, !pcm.isEmpty else { return "" }
            return pcm.withUnsafeBufferPointer { b in
                guard let p = engine.lib.streamFeed(s, b.baseAddress!, Int32(b.count), nil) else { return "" }
                defer { engine.lib.freeString(p) }
                return String(cString: p)
            }
        }
        func finish() -> String {
            engine.lock.lock(); defer { engine.lock.unlock() }
            guard let s = stream else { return "" }
            stream = nil
            defer { engine.lib.streamFree(s) }
            guard let p = engine.lib.streamFinalize(s) else { return "" }
            defer { engine.lib.freeString(p) }
            return String(cString: p)
        }
        deinit { if let s = stream { engine.lib.streamFree(s) } }
    }
}
