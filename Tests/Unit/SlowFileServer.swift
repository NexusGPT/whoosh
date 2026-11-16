import CryptoKit
import Foundation
import Network

/// A loopback file server that sends each file slowly: `chunk` bytes, then `pause` seconds, and holds every GET for `firstByteDelay` seconds before its first byte. A path it does not hold answers 404. Counts every request. Loopback only: no real network.
final class SlowFileServer: @unchecked Sendable {
    let listener: NWListener
    private(set) var port: UInt16 = 0
    private let lock = NSLock()
    private var files: [String: Data]
    private var asked: [String] = []
    private var starts: [Get] = []
    let chunk: Int, pause: TimeInterval, firstByteDelay: TimeInterval

    init(files: [String: Data], chunk: Int = 4000, pause: TimeInterval = 0.12, firstByteDelay: TimeInterval = 0.5) throws {
        self.files = files; self.chunk = chunk; self.pause = pause; self.firstByteDelay = firstByteDelay
        let params = NWParameters.tcp
        params.requiredLocalEndpoint = NWEndpoint.hostPort(host: "127.0.0.1", port: .any)
        listener = try NWListener(using: params)
        listener.newConnectionHandler = { [weak self] conn in self?.serve(conn) }
    }

    var base: URL { URL(string: "http://127.0.0.1:\(port)/")! }
    var requests: [String] { lock.withLock { asked } }
    /// One GET of a file this server holds, and the byte it was asked to start from (0 when the request carried no `Range`).
    struct Get: Equatable { let path: String; let from: Int }
    var gets: [Get] { lock.withLock { starts } }
    func put(_ path: String, _ data: Data) { lock.withLock { files[path] = data } }

    func start() async {
        let ready = AsyncStream<UInt16> { c in
            listener.stateUpdateHandler = { st in
                if case .ready = st, let p = self.listener.port?.rawValue { c.yield(p); c.finish() }
                if case .failed = st { c.yield(0); c.finish() }
            }
        }
        listener.start(queue: .global())
        for await p in ready { port = p }
    }
    func stop() { listener.cancel() }

    private func serve(_ conn: NWConnection) {
        conn.start(queue: .global())
        conn.receive(minimumIncompleteLength: 1, maximumLength: 8192) { [self] data, _, _, _ in
            let text = String(decoding: data ?? Data(), as: UTF8.self)
            let line = text.split(separator: "\r\n").first.map(String.init) ?? ""
            let parts = line.split(separator: " ")
            let path = parts.count > 1 ? String(parts[1].dropFirst()) : ""
            lock.withLock { asked.append("\(parts.first ?? "") \(path)") }
            guard let body = lock.withLock({ files[path] }) else {
                conn.send(content: Data("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".utf8), completion: .contentProcessed { _ in conn.cancel() })
                return
            }
            var offset = 0
            if let r = text.range(of: "bytes=", options: .caseInsensitive) { offset = Int(text[r.upperBound...].prefix { $0.isNumber }) ?? 0 }
            if parts.first == "GET" { lock.withLock { starts.append(Get(path: path, from: offset)) } }
            let rest = body.suffix(from: min(offset, body.count))
            let status = offset > 0 ? "206 Partial Content" : "200 OK"
            let range = offset > 0 ? "Content-Range: bytes \(offset)-\(body.count - 1)/\(body.count)\r\n" : ""
            let head = "HTTP/1.1 \(status)\r\n\(range)Content-Length: \(rest.count)\r\nConnection: close\r\n\r\n"
            if parts.first == "HEAD" { conn.send(content: Data(head.utf8), completion: .contentProcessed { _ in conn.cancel() }); return }
            let big = rest.count > chunk
            DispatchQueue.global().asyncAfter(deadline: .now() + firstByteDelay) { [self] in
                conn.send(content: Data(head.utf8), completion: .contentProcessed { _ in })
                send(Data(rest), conn: conn, slow: big)
            }
        }
    }

    private func send(_ data: Data, conn: NWConnection, slow: Bool) {
        guard !data.isEmpty else { conn.cancel(); return }
        let n = slow ? min(chunk, data.count) : data.count
        conn.send(content: data.prefix(n), completion: .contentProcessed { [self] error in
            if error != nil { conn.cancel(); return }
            DispatchQueue.global().asyncAfter(deadline: .now() + (slow ? pause : 0)) { self.send(Data(data.dropFirst(n)), conn: conn, slow: slow) }
        })
    }

    static func hex(_ data: Data) -> String { SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined() }
}
