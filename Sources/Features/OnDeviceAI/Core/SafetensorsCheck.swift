import Foundation

/// A cheap look inside a weights file before MLX is asked to load it: the safetensors header must be
/// readable and every tensor must lie inside the file. A truncated or overwritten file fails here in words,
/// not later as a crash inside the runtime.
public enum SafetensorsCheck {
    public static let headerLimit: UInt64 = 100_000_000

    /// nil when the file looks whole; otherwise one sentence saying what is wrong.
    public static func problem(in url: URL) -> String? {
        guard let h = try? FileHandle(forReadingFrom: url) else { return Words.text("The model weights file is missing.") }
        defer { try? h.close() }
        let size = UInt64((try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.uint64Value ?? 0)
        guard size > 8, let lead = try? h.read(upToCount: 8), lead.count == 8 else { return Words.text("The model weights file is empty or cut short.") }
        let n = lead.withUnsafeBytes { $0.loadUnaligned(as: UInt64.self) }.littleEndian
        guard n > 1, n <= headerLimit, 8 + n <= size else { return Words.text("The model weights file has a damaged header.") }
        guard let head = try? h.read(upToCount: Int(n)), head.count == Int(n),
              let json = try? JSONSerialization.jsonObject(with: head) as? [String: Any] else {
            return Words.text("The model weights file has an unreadable header.")
        }
        let dataSize = size - 8 - n
        for (key, value) in json where key != "__metadata__" {
            guard let t = value as? [String: Any], let o = t["data_offsets"] as? [NSNumber], o.count == 2,
                  o[1].uint64Value >= o[0].uint64Value, o[1].uint64Value <= dataSize else {
                return Words.text("The model weights file is cut short: a tensor lies beyond its end.")
            }
        }
        return nil
    }
}
