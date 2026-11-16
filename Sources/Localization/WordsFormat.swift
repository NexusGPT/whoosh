import Foundation

/// The pure half of `Words`: fills `%@`, `%lld` and their positional forms (`%1$@`) from text, so a placeholder
/// and an argument of another kind never crash a label. `%%` is a percent sign. A missing argument leaves nothing.
public enum WordsFormat {
    public static func fill(_ format: String, _ args: [String]) -> String {
        let chars = Array(format)
        var out = ""
        var next = 0
        var i = 0
        while i < chars.count {
            let c = chars[i]
            guard c == "%", i + 1 < chars.count else { out.append(c); i += 1; continue }
            if chars[i + 1] == "%" { out.append("%"); i += 2; continue }
            var j = i + 1
            var digits = ""
            while j < chars.count, chars[j].isNumber { digits.append(chars[j]); j += 1 }
            var position: Int?
            if !digits.isEmpty, j < chars.count, chars[j] == "$" { position = Int(digits); j += 1 } else { j = i + 1 }
            let rest = String(chars[j...])
            let kind = ["@", "lld", "ld", "d"].first { rest.hasPrefix($0) }
            guard let kind else { out.append(c); i += 1; continue }
            let index = position.map { $0 - 1 } ?? next
            if position == nil { next += 1 }
            if index >= 0, index < args.count { out += args[index] }
            i = j + kind.count
        }
        return out
    }
}
