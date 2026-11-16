import Foundation

/// The control socket (docs/CONTROL-SOCKET.md): newline-delimited JSON over a Unix stream socket at
/// $WHOOSH_SOCKET_DIR/whoosh.sock. Started only when that variable is set, which only the rig does
/// (scripts/launch-one.sh). Each request runs on the main actor, so a reply reflects what is on screen.
public final class ControlServer: @unchecked Sendable {
    public static let socketEnv = "WHOOSH_SOCKET_DIR"
    public static let socketName = "whoosh.sock"
    private var fd: Int32 = -1
    private let path: String
    private let handle: @Sendable (String) -> String

    public init?(handle: @escaping @Sendable (String) -> String) {
        guard let dir = ProcessInfo.processInfo.environment[Self.socketEnv], !dir.isEmpty else { return nil }
        path = (dir as NSString).appendingPathComponent(Self.socketName)
        self.handle = handle
    }

    public func start() throws {
        unlink(path)
        fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { throw POSIXError(.EIO) }
        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let bytes = Array(path.utf8)
        guard bytes.count < MemoryLayout.size(ofValue: addr.sun_path) else { throw POSIXError(.ENAMETOOLONG) }
        withUnsafeMutableBytes(of: &addr.sun_path) { buf in
            for (i, b) in bytes.enumerated() { buf[i] = b }
            buf[bytes.count] = 0
        }
        let len = socklen_t(MemoryLayout<sockaddr_un>.size)
        let rc = withUnsafePointer(to: &addr) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, len) } }
        guard rc == 0, listen(fd, 8) == 0 else { throw POSIXError(.EADDRINUSE) }
        let listener = fd
        let thread = Thread { [self] in
            while true {
                let c = accept(listener, nil, nil)
                if c < 0 { continue }
                let t = Thread { [self] in serve(c) }
                t.start()
            }
        }
        thread.name = "whoosh.control.accept"
        thread.start()
    }

    private func serve(_ c: Int32) {
        var buffer = Data()
        var chunk = [UInt8](repeating: 0, count: 65536)
        defer { close(c) }
        while true {
            let n = read(c, &chunk, chunk.count)
            if n <= 0 { return }
            buffer.append(contentsOf: chunk[0..<n])
            while let nl = buffer.firstIndex(of: 0x0A) {
                let line = String(decoding: buffer[buffer.startIndex..<nl], as: UTF8.self)
                buffer.removeSubrange(buffer.startIndex...nl)
                ControlArrival.stamp()
                let reply = Array((handle(line) + "\n").utf8)
                var off = 0
                while off < reply.count {
                    let w = reply[off...].withUnsafeBufferPointer { write(c, $0.baseAddress, $0.count) }
                    if w <= 0 { return }
                    off += w
                }
            }
        }
    }
}

/// One request: `{"verb": "<verb>", "args": [...]}`. Parsed, never cast: a wrong shape is an error reply.
public struct ControlRequest: Sendable {
    public let verb: String
    public let args: [String]

    public static func parse(_ line: String) -> Result<ControlRequest, ControlFailure> {
        guard let data = line.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return .failure(ControlFailure("the request is not a JSON object"))
        }
        guard let verb = obj["verb"] as? String, !verb.isEmpty else { return .failure(ControlFailure("the request has no verb")) }
        let raw = obj["args"] as? [Any] ?? []
        var args: [String] = []
        for a in raw {
            switch a {
            case let s as String: args.append(s)
            case let n as NSNumber where n === kCFBooleanTrue || n === kCFBooleanFalse:
                return .failure(ControlFailure("argument \(args.count) of \(verb) is a boolean; arguments are text or numbers"))
            case let n as NSNumber: args.append(n.stringValue)
            default: return .failure(ControlFailure("argument \(args.count) of \(verb) is neither text nor a number"))
            }
        }
        return .success(ControlRequest(verb: verb, args: args))
    }
}

public struct ControlFailure: Error, Sendable {
    public let words: String
    public init(_ words: String) { self.words = words }
}

public enum ControlReply {
    public static func ok(_ fields: [String: Any] = [:]) -> String {
        var f = fields; f["ok"] = true
        return encode(f)
    }
    public static func fail(_ words: String) -> String { encode(["ok": false, "error": words]) }
    static func encode(_ obj: [String: Any]) -> String {
        guard JSONSerialization.isValidJSONObject(obj),
              let d = try? JSONSerialization.data(withJSONObject: obj, options: [.sortedKeys]) else {
            return #"{"ok":false,"error":"the reply could not be encoded"}"#
        }
        return String(decoding: d, as: UTF8.self)
    }
}
