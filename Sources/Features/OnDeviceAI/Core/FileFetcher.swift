import CryptoKit
import Foundation

/// Fetches ONE file into a `.part` file, resuming from the bytes already there, hashing every byte it writes.
/// It does not decide what to do about a pause or a failure; ModelInstaller does.
final class FileFetcher: NSObject, URLSessionDataDelegate, @unchecked Sendable {
    enum Failure: Error { case status(Int), rangeRefused, io(String) }
    struct Result { let bytes: Int64; let sha256: String }

    private let url: URL, part: URL, onBytes: @Sendable (Int64) -> Void
    private let lock = NSLock()
    private var hasher = SHA256(), handle: FileHandle?, offset: Int64 = 0, failure: Error?
    private var cont: CheckedContinuation<Result, Error>?, task: URLSessionDataTask?, session: URLSession?

    init(url: URL, part: URL, onBytes: @escaping @Sendable (Int64) -> Void) { self.url = ModelsFolder.rigSafe(url); self.part = part; self.onBytes = onBytes }   // a rig copy never asks the real host

    func stop() { lock.lock(); let t = task; lock.unlock(); t?.cancel() }

    func run() async throws -> Result {
        try prepare()
        var req = URLRequest(url: url)
        if offset > 0 { req.setValue("bytes=\(offset)-", forHTTPHeaderField: "Range") }
        let s = URLSession(configuration: .ephemeral, delegate: self, delegateQueue: nil)
        let t = s.dataTask(with: req)
        remember(s, t)
        defer { s.finishTasksAndInvalidate(); try? handle?.close() }
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { (c: CheckedContinuation<Result, Error>) in
                setCont(c); t.resume()
            }
        } onCancel: { stop() }
    }

    private func remember(_ s: URLSession, _ t: URLSessionDataTask) { lock.withLock { session = s; task = t } }
    private func setCont(_ c: CheckedContinuation<Result, Error>) { lock.withLock { cont = c } }

    /// Opens the part file and hashes what is already in it, so the final checksum covers every byte.
    private func prepare() throws {
        let fm = FileManager.default
        if !fm.fileExists(atPath: part.path) {
            try fm.createDirectory(at: part.deletingLastPathComponent(), withIntermediateDirectories: true)
            guard fm.createFile(atPath: part.path, contents: nil) else { throw Failure.io(part.path) }
        }
        let reader = try FileHandle(forReadingFrom: part)
        while let chunk = try reader.read(upToCount: 1 << 20), !chunk.isEmpty { hasher.update(data: chunk); offset += Int64(chunk.count) }
        try reader.close()
        handle = try FileHandle(forWritingTo: part)
        try handle?.seekToEnd()
    }

    private func restartFromZero() throws {
        try handle?.truncate(atOffset: 0); try handle?.seek(toOffset: 0)
        hasher = SHA256(); offset = 0
    }

    func urlSession(_ s: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse,
                    completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        let code = (response as? HTTPURLResponse)?.statusCode ?? 0
        do {
            if code == 200 && offset > 0 { try restartFromZero() }            // the server ignored Range: begin again
            else if code == 416 { failure = Failure.rangeRefused; completionHandler(.cancel); return }
            else if code != 200 && code != 206 { failure = Failure.status(code); completionHandler(.cancel); return }
            completionHandler(.allow)
        } catch { failure = Failure.io("\(error)"); completionHandler(.cancel) }
    }

    func urlSession(_ s: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        do { try handle?.write(contentsOf: data) } catch { failure = Failure.io("\(error)"); dataTask.cancel(); return }
        hasher.update(data: data); offset += Int64(data.count)
        onBytes(offset)
    }

    func urlSession(_ s: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        lock.lock(); let c = cont; cont = nil; lock.unlock()
        if let failure { c?.resume(throwing: failure) }
        else if let error { c?.resume(throwing: error) }
        else {
            let hex = hasher.finalize().map { String(format: "%02x", $0) }.joined()
            c?.resume(returning: Result(bytes: offset, sha256: hex))
        }
    }
}
