import Foundation
import Testing

/// The contract tests drive the RUNNING app through its control socket (docs/CONTROL-SOCKET.md).
/// Run them under the rig, which publishes $WHOOSH_SOCKET_DIR (or $D) for exactly one app copy:
///
///   sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" swift test --filter Contract'
///
/// With no socket in the environment every contract test is DISABLED (shown as skipped in the run),
/// so a plain `swift test` of the other lanes stays green. A set socket that nothing answers on is a
/// RED test, never a skip: a pass over a dead app is not a pass.
enum Ctl {
    static var dir: String? {
        let e = ProcessInfo.processInfo.environment
        for k in ["WHOOSH_SOCKET_DIR", "D"] { if let v = e[k], !v.isEmpty { return v } }
        return nil
    }
    static var available: Bool { dir != nil }
    /// Where the APP writes its files (shots). The socket directory, unless a relay sits in between
    /// (the mutant runs of docs/lanes/contracttests.md set WHOOSH_APP_DIR to the real one).
    static var appDir: String? { ProcessInfo.processInfo.environment["WHOOSH_APP_DIR"] ?? dir }

    /// The fixture the app was launched on, read from the repo (never from a built screen).
    nonisolated(unsafe) static let fixture: [String: Any] = {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let data = try! Data(contentsOf: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"))
        return try! JSONSerialization.jsonObject(with: data) as! [String: Any]
    }()
    static var messages: [[String: Any]] { fixture["messages"] as! [[String: Any]] }
    static var tabs: [[String: Any]] { fixture["tabs"] as! [[String: Any]] }
    static var accounts: [[String: Any]] { fixture["accounts"] as! [[String: Any]] }
    static var mailboxes: [[String: Any]] { fixture["mailboxes"] as! [[String: Any]] }

    struct Failure: Error, CustomStringConvertible { let description: String }

    /// One request line in, one reply line out, on a fresh connection.
    static func raw(_ line: String) throws -> [String: Any] {
        guard let dir else { throw Failure(description: "no WHOOSH_SOCKET_DIR") }
        let path = dir + "/whoosh.sock"
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { throw Failure(description: "socket() failed") }
        defer { close(fd) }
        var tv = timeval(tv_sec: 10, tv_usec: 0)
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, socklen_t(MemoryLayout<timeval>.size))
        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let bytes = Array(path.utf8)
        guard bytes.count < MemoryLayout.size(ofValue: addr.sun_path) else { throw Failure(description: "socket path too long") }
        withUnsafeMutableBytes(of: &addr.sun_path) { buf in
            for (i, b) in bytes.enumerated() { buf[i] = b }
            buf[bytes.count] = 0
        }
        let rc = withUnsafePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) }
        }
        guard rc == 0 else { throw Failure(description: "no app answers on \(path) (errno \(errno))") }
        let out = Array((line + "\n").utf8)
        var off = 0
        while off < out.count {
            let w = out[off...].withUnsafeBufferPointer { write(fd, $0.baseAddress, $0.count) }
            guard w > 0 else { throw Failure(description: "write failed") }
            off += w
        }
        var buffer = Data()
        var chunk = [UInt8](repeating: 0, count: 65536)
        while !buffer.contains(0x0A) {
            let n = read(fd, &chunk, chunk.count)
            guard n > 0 else { throw Failure(description: "the socket closed or timed out with no reply to \(line)") }
            buffer.append(contentsOf: chunk[0..<n])
        }
        let reply = buffer.prefix(while: { $0 != 0x0A })
        guard let obj = try JSONSerialization.jsonObject(with: Data(reply)) as? [String: Any] else {
            throw Failure(description: "the reply is not a JSON object: \(String(decoding: reply, as: UTF8.self))")
        }
        return obj
    }

    /// `{"verb": verb, "args": args}`; args are String, Int or Double.
    static func ask(_ verb: String, _ args: [Any] = []) throws -> Reply {
        let body: [String: Any] = ["verb": verb, "args": args]
        let data = try JSONSerialization.data(withJSONObject: body)
        return Reply(try raw(String(decoding: data, as: UTF8.self)))
    }

    /// Like `ask`, but the verb must succeed.
    @discardableResult
    static func ok(_ verb: String, _ args: [Any] = []) throws -> Reply {
        let r = try ask(verb, args)
        #expect(r.ok, "\(verb) \(args) should be ok, got error: \(r.error ?? "nil")")
        return r
    }

    static func state() throws -> [String: Any] {
        let r = try ask("uistate")
        guard r.ok, let s = r.fields["state"] as? [String: Any] else { throw Failure(description: "uistate failed: \(r.error ?? "no state")") }
        return s
    }
    static func rows(_ ax: String) throws -> [[String: Any]] {
        let r = try ask("rows", [ax])
        guard r.ok, let rows = r.fields["rows"] as? [[String: Any]] else { throw Failure(description: "rows \(ax) failed: \(r.error ?? "no rows")") }
        return rows
    }
    static func unread(_ tab: String) throws -> Int {
        guard let c = try state()["counts"] as? [String: Any], let n = c[tab] as? Int else { throw Failure(description: "uistate.counts has no number for \(tab)") }
        return n
    }
}

/// One reply, split into the `ok` flag, the `error` words and the verb's own fields.
struct Reply {
    let all: [String: Any]
    init(_ all: [String: Any]) { self.all = all }
    var ok: Bool { all["ok"] as? Bool ?? false }
    var error: String? { all["error"] as? String }
    var keys: Set<String> { Set(all.keys) }
    /// every key except `ok` and `error`
    var fields: [String: Any] { all.filter { $0.key != "ok" && $0.key != "error" } }
}

/// True when the JSON value is a real boolean, not the number 0 or 1.
func isJSONBool(_ v: Any?) -> Bool {
    guard let n = v as? NSNumber else { return false }
    return CFGetTypeID(n) == CFBooleanGetTypeID()
}

/// Every contract suite nests in this one: serialized, because they share ONE app and its state.
@Suite("Contract", .serialized, .enabled(if: Ctl.available, "no WHOOSH_SOCKET_DIR: run under scripts/launch-one.sh"))
struct Contract {}

extension Ctl {
    /// Message ids of the Primary list as a live test copy serves it (the order `rows mac-main.list` serves): the pinned rows, the rejected flow rows
    /// newest first, then the rest. The one definition is FixtureFileFigures.primaryIds (Tests/Support, the same test target).
    static var primaryIds: [String] { try! FixtureFileFigures.primaryIds() }
    static func message(_ id: String) -> [String: Any] { messages.first { $0["id"] as? String == id }! }
    /// A tab's unread as a live test copy serves it (the file's figure plus the flow rows seeded into the tab): FixtureFileFigures.servedTabUnread.
    static func tabUnread(_ id: String) -> Int { try! FixtureFileFigures.servedTabUnread(id) }
    static func accountName(_ id: String) -> String { accounts.first { $0["id"] as? String == id }!["name"] as! String }
}
