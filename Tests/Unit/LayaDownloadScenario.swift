import Foundation
#if canImport(WhooshUI)
@testable import WhooshUI
#endif

/// A loopback HTTP server on 127.0.0.1 for the decision model's download: serves the five file names of `LayaModelStore.remoteFiles`, the first one
/// slowly (so progress can be seen), HEAD answers delayed (so "the click shows at once" can be told from "the sizes arrived"). No real network.
final class LayaLoopbackServer: @unchecked Sendable {
    static let fileSize = 150_000
    static let chunks = 10, chunkPauseUs: UInt32 = 120_000, headPauseUs: UInt32 = 300_000
    private let lock = NSLock()
    private var _notFound = Set<String>(), _firstByteAt: Date?, _slow = Set<String>()
    private var fd: Int32 = -1
    private(set) var port: UInt16 = 0
    var notFound: Set<String> { get { lock.lock(); defer { lock.unlock() }; return _notFound } set { lock.lock(); _notFound = newValue; lock.unlock() } }
    var slow: Set<String> { get { lock.lock(); defer { lock.unlock() }; return _slow } set { lock.lock(); _slow = newValue; lock.unlock() } }
    var firstByteAt: Date? { lock.lock(); defer { lock.unlock() }; return _firstByteAt }
    var base: URL { URL(string: "http://127.0.0.1:\(port)/")! }

    init() throws {
        fd = socket(AF_INET, SOCK_STREAM, 0)
        var yes: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &yes, socklen_t(MemoryLayout<Int32>.size))
        var addr = sockaddr_in()
        addr.sin_family = sa_family_t(AF_INET); addr.sin_addr.s_addr = inet_addr("127.0.0.1"); addr.sin_port = 0
        let ok = withUnsafePointer(to: &addr) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) } }
        guard ok == 0, listen(fd, 16) == 0 else { throw NSError(domain: "loopback", code: 1) }
        var len = socklen_t(MemoryLayout<sockaddr_in>.size)
        _ = withUnsafeMutablePointer(to: &addr) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { getsockname(fd, $0, &len) } }
        port = UInt16(bigEndian: addr.sin_port)
        let listener = fd
        Thread.detachNewThread { [weak self] in
            while true {
                let c = accept(listener, nil, nil)
                if c < 0 { return }
                Thread.detachNewThread { self?.serve(c) }
            }
        }
    }

    func stop() { shutdown(fd, SHUT_RDWR); close(fd) }

    private func serve(_ c: Int32) {
        defer { close(c) }
        var buf = [UInt8](repeating: 0, count: 4096)
        let n = recv(c, &buf, buf.count, 0)
        guard n > 0, let head = String(bytes: buf[0..<n], encoding: .utf8), let line = head.split(separator: "\r\n").first else { return }
        let parts = line.split(separator: " ")
        guard parts.count >= 2 else { return }
        let method = String(parts[0]), path = String(parts[1].dropFirst())
        func send(_ s: String) { _ = s.withCString { Darwin.send(c, $0, strlen($0), 0) } }
        if notFound.contains(path) { send("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"); return }
        if method == "HEAD" {
            usleep(Self.headPauseUs)
            send("HTTP/1.1 200 OK\r\nContent-Length: \(Self.fileSize)\r\nConnection: close\r\n\r\n"); return
        }
        send("HTTP/1.1 200 OK\r\nContent-Length: \(Self.fileSize)\r\nConnection: close\r\n\r\n")
        let slowFile = slow.contains(path)
        let piece = Self.fileSize / Self.chunks
        let body = [UInt8](repeating: 0x41, count: piece)
        for i in 0..<Self.chunks {
            lock.lock(); if _firstByteAt == nil { _firstByteAt = Date() }; lock.unlock()
            let size = i == Self.chunks - 1 ? Self.fileSize - piece * (Self.chunks - 1) : piece
            _ = body.withUnsafeBufferPointer { Darwin.send(c, $0.baseAddress, size, 0) }
            if slowFile { usleep(Self.chunkPauseUs) }
        }
    }
}

/// What the decision model's download must do, as a list of failures (empty = all hold). Run by the unit test and by the standalone runner
/// (_captures/settingsaimac-r4/laya-download/run.sh), which also runs it against two mutants of LayaModelStore.
enum LayaDownloadScenario {
    final class Log: @unchecked Sendable {
        private let lock = NSLock()
        private var items: [(Date, LayaModelState)] = []
        func add(_ s: LayaModelState) { lock.lock(); items.append((Date(), s)); lock.unlock() }
        var all: [(Date, LayaModelState)] { lock.lock(); defer { lock.unlock() }; return items }
    }

    static func run() async -> [String] {
        var fails: [String] = []
        func expect(_ ok: Bool, _ what: @autoclosure () -> String) { if !ok { fails.append(what()) } }
        guard let server = try? LayaLoopbackServer() else { return ["the loopback server did not start"] }
        defer { server.stop() }
        let temp = FileManager.default.temporaryDirectory.appendingPathComponent("laya-dl-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: temp) }
        let files = LayaModelStore.remoteFiles
        let store = LayaModelStore(directory: temp, baseURL: server.base, files: files)
        let S = Int64(LayaLoopbackServer.fileSize)

        // 1. the first file is slow: progress is seen inside it; the first state arrives at the click, before any size or byte.
        server.slow = [files[0]]
        let log = Log(), t0 = Date()
        let end = await store.download { log.add($0) }
        let states = log.all
        if let first = states.first {
            var isDownloading = false
            if case .downloading = first.1 { isDownloading = true }
            expect(isDownloading, "the first state is not .downloading: \(first.1)")
            expect(first.0.timeIntervalSince(t0) < 0.15, "the first state came \(Int(first.0.timeIntervalSince(t0) * 1000)) ms after the click, not at once")
            if let b = server.firstByteAt { expect(first.0 < b, "the first state came after the first byte") }
        } else { expect(false, "no progress state at all") }
        let inside = Set(states.compactMap { _, s -> Int64? in if case .downloading(let r, _) = s, r > 0, r < S { return r } else { return nil } })
        expect(inside.count >= 3, "\(inside.count) distinct received values while the first file downloaded (need 3): \(inside.sorted())")
        if case .ready(let bytes) = end { expect(bytes == S * Int64(files.count), "ready with \(bytes) bytes, expected \(S * Int64(files.count))") }
        else { expect(false, "the end state is not .ready: \(end)") }
        if case .ready = store.state {} else { expect(false, "state after the download is not .ready: \(store.state)") }

        // 2. a 404 on one file ends .failed with a sentence that names the file.
        let temp2 = temp.appendingPathComponent("two", isDirectory: true)
        let store2 = LayaModelStore(directory: temp2, baseURL: server.base, files: files)
        server.slow = []
        let bad = files[files.count - 1]
        server.notFound = [bad]
        let failed = await store2.download()
        if case .failed(let why) = failed { expect(why.contains(bad), "the failure does not name \(bad): \(why)") } else { expect(false, "a 404 did not end .failed: \(failed)") }

        // 2b. the failure stays: a later read answers .failed with its sentence, not "not downloaded", until the next try.
        if case .failed(let why) = store2.refresh() { expect(why.contains(bad), "refresh() after a failure lost its sentence: \(why)") }
        else { expect(false, "refresh() after a failure fell back to \(store2.refresh()), not .failed") }
        if case .failed = store2.state {} else { expect(false, "state after a failure is not .failed: \(store2.state)") }

        // 3. a second download() after the failure retries and ends ready.
        server.notFound = []
        let retried = await store2.download()
        if case .ready = retried {} else { expect(false, "the retry after a failure did not end .ready: \(retried)") }

        // 4. a download in flight is joined: a second call returns the running state and starts nothing.
        let temp3 = temp.appendingPathComponent("three", isDirectory: true)
        let store3 = LayaModelStore(directory: temp3, baseURL: server.base, files: files)
        server.slow = [files[0]]
        async let first = store3.download()
        try? await Task.sleep(nanoseconds: 600_000_000)
        let joined = await store3.download()
        if case .downloading = joined {} else { expect(false, "a second call during a download did not return the running state: \(joined)") }
        if case .downloading = store3.state {} else { expect(false, "refresh() wiped the running download: \(store3.state)") }
        _ = await first

        // 4b. Cancel stops the download at once and leaves no half file; a download after it still works.
        let temp6 = temp.appendingPathComponent("six", isDirectory: true)
        let store6 = LayaModelStore(directory: temp6, baseURL: server.base, files: files)
        server.slow = [files[0]]
        async let cancelled = store6.download()
        try? await Task.sleep(nanoseconds: 700_000_000)
        let c0 = Date()
        store6.cancel()
        let endC = await cancelled
        let took = Date().timeIntervalSince(c0)
        if case .notDownloaded = endC {} else { expect(false, "Cancel did not end in .notDownloaded: \(endC)") }
        expect(took < 0.6, "Cancel took \(Int(took * 1000)) ms to stop the download (the file takes about 1.2 s)")
        var half: [String] = []
        if let en = FileManager.default.enumerator(at: temp6, includingPropertiesForKeys: [.fileSizeKey, .isRegularFileKey]) {
            for u in (en.allObjects as? [URL] ?? []) {   // an enumerator's own iterator is unavailable in an async function
                let v = try? u.resourceValues(forKeys: [.fileSizeKey, .isRegularFileKey])
                if v?.isRegularFile == true, Int64(v?.fileSize ?? 0) != S { half.append(u.lastPathComponent) }
            }
        }
        expect(half.isEmpty, "Cancel left files that are not whole: \(half)")
        server.slow = []
        let again = await store6.download()
        if case .ready = again {} else { expect(false, "a download after Cancel did not end .ready: \(again)") }

        // 5. a rig copy keeps everything under its own run folder, and fetches only from loopback; the owner's copy is untouched by the rig's variables.
        let rig = ["WHOOSH_SOCKET_DIR": "/tmp/rigrun-x"]
        expect(ModelsFolder.whooshRoot(env: rig).path == "/tmp/rigrun-x", "a rig copy's app folder is \(ModelsFolder.whooshRoot(env: rig).path), not its run folder")
        expect(ModelsFolder.root(env: rig).path == "/tmp/rigrun-x/Models", "a rig copy's models folder is \(ModelsFolder.root(env: rig).path)")
        let owner = ModelsFolder.root(env: [:]).path
        expect(owner.hasSuffix("Application Support/Whoosh/Models"), "the owner's models folder is \(owner)")
        expect(ModelsFolder.rigLayaBase(env: rig.merging(["WHOOSH_RIG_LAYA_BASE": "http://127.0.0.1:9999/"]) { $1 })?.absoluteString == "http://127.0.0.1:9999/", "a loopback base was not kept on a rig copy")
        expect(ModelsFolder.rigLayaBase(env: rig.merging(["WHOOSH_RIG_LAYA_BASE": "https://huggingface.co/x/"]) { $1 })?.host == "127.0.0.1", "a real host was kept on a rig copy")
        expect(ModelsFolder.rigLayaBase(env: ["WHOOSH_RIG_LAYA_BASE": "http://127.0.0.1:9999/"]) == nil, "the owner's copy took the rig base")
        return fails
    }
}
