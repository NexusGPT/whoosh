import Foundation

/// `fixture fail-next <op> <reason>`: the next run of `op` fails once with `reason` in words.
/// Fixture core only. A core command's op is its case name in kebab case (`archive`, `move-to-account`);
/// a feature's own save asks `take("notch-setting-save")` before it writes.
@MainActor
public enum FixtureFaults {
    private static var armed: [String: String] = [:]

    public static func arm(_ op: String, reason: String) { armed[op] = reason }

    /// The armed reason for `op`, consumed: a second call returns nil.
    public static func take(_ op: String) -> String? { armed.removeValue(forKey: op) }

    public static func clear() { armed = [:] }

    public static var pending: [String: String] { armed }

    /// `.moveToAccount(...)` -> "move-to-account".
    public static func op(of command: Command) -> String {
        let name = String(describing: command).prefix { $0 != "(" }
        var out = ""
        for ch in name {
            if ch.isUppercase { out += "-" + ch.lowercased() } else { out.append(ch) }
        }
        return out
    }
}
