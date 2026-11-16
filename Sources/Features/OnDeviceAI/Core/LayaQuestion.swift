import Foundation

/// A typed question in the shape Laya and Jev both take (research/decision-model.md "The one contract").
public struct LayaQuestion: Equatable, Sendable {
    public enum Kind: String, Sendable { case choice, score, noul
        /// the index the checkpoint's type embedding uses (rl_common.py QTYPES)
        var index: Int { switch self { case .choice: return 0; case .score: return 1; case .noul: return 2 } }
    }
    public let id: String
    public let kind: Kind
    public let instructions: String
    /// choice: label and optional description, in order. score: the ordered level descriptions (label unused).
    public let options: [(label: String, description: String?)]
    /// noul: the wording for "true" and "false" (nil uses the authors' defaults).
    public let trueWording: String?
    public let falseWording: String?

    public static func == (a: LayaQuestion, b: LayaQuestion) -> Bool {
        a.id == b.id && a.kind == b.kind && a.instructions == b.instructions
            && a.options.map { "\($0.label)|\($0.description ?? "")" } == b.options.map { "\($0.label)|\($0.description ?? "")" }
            && a.trueWording == b.trueWording && a.falseWording == b.falseWording
    }

    public static func choice(id: String, instructions: String, options: [(label: String, description: String?)]) -> LayaQuestion {
        .init(id: id, kind: .choice, instructions: instructions, options: options, trueWording: nil, falseWording: nil)
    }
    public static func score(id: String, instructions: String, levels: [String]) -> LayaQuestion {
        .init(id: id, kind: .score, instructions: instructions, options: levels.map { ($0, nil) }, trueWording: nil, falseWording: nil)
    }
    public static func noul(id: String, instructions: String, yes: String? = nil, no: String? = nil) -> LayaQuestion {
        .init(id: id, kind: .noul, instructions: instructions, options: [], trueWording: yes, falseWording: no)
    }

    /// Option texts in label-index order; a noul is always [false, true] so index 1 is "yes" (rl_common.py render_options).
    public func renderedOptions() -> [String] {
        switch kind {
        case .choice:
            return options.map { o in (o.description?.isEmpty ?? true) ? o.label : "\(o.label): \(o.description!)" }
        case .score:
            return options.enumerated().map { "level \($0.offset): \($0.element.label)" }
        case .noul:
            let f = (falseWording?.isEmpty ?? true) ? "no, the statement does not hold" : falseWording!
            let t = (trueWording?.isEmpty ?? true) ? "yes, the statement holds" : trueWording!
            return ["false: " + f, "true: " + t]
        }
    }
}

public struct LayaAnswer: Equatable, Sendable {
    public enum Value: Equatable, Sendable {
        case choice(label: String, probabilities: [String: Double])
        case score(value: Double, probabilities: [Double])
        case noul(yes: Double)
    }
    public let id: String
    public let value: Value
    /// 1 minus normalised entropy for choice and score; the larger side for a yes/no.
    public let confidence: Double
}

/// The state text, byte for byte what Python's json.dumps(dict(sorted(fields), text=text), ensure_ascii=False) writes,
/// so the Swift runtime and the authors' reference see the same string.
public enum LayaState {
    public static func text(fields: [String: String], text: String) -> String {
        var parts: [String] = []
        for key in fields.keys.sorted() { parts.append("\(quote(key)): \(quote(fields[key]!))") }
        parts.append("\"text\": \(quote(text))")
        return "{" + parts.joined(separator: ", ") + "}"
    }
    static func quote(_ s: String) -> String {
        var out = "\""
        for u in s.unicodeScalars {
            switch u {
            case "\"": out += "\\\""
            case "\\": out += "\\\\"
            case "\n": out += "\\n"
            case "\r": out += "\\r"
            case "\t": out += "\\t"
            case "\u{08}": out += "\\b"
            case "\u{0C}": out += "\\f"
            default:
                if u.value < 0x20 { out += String(format: "\\u%04x", u.value) } else { out.unicodeScalars.append(u) }
            }
        }
        return out + "\""
    }
}
