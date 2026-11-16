import Foundation

/// The request layout of GLiDE, the third family of on-device decision model (SPEC A133): a Qwen 3.5 text model that reads one prompt per
/// question and whose answer is read from a 255-way readout at the last token, the "(" that follows "Answer:". Nothing is generated.
///
/// A Swift port of the authors' prompt code, `glide/prompt.py` of the model repository fastino/GLiDE-0.8B at revision
/// 9eacdeec79fa48ec52a8d6f54b331376b4bbfaf0 (options_for, Prompter.body, Prompter.text; Fastino Community License). The sentences are theirs,
/// byte for byte: the weights were trained on them. Pure text: no model and no tokenizer here (`GlideEngine` holds both).
/// `checks/glide-layout-check.py` compares what this file writes with the authors' own output on made-up mail
/// (checks/corpora/glide/reference.json).
public enum GlideLayout {
    /// The model's chat template around one user turn, as the authors' `Prompter` renders it once and reuses (thinking on, reasoning effort
    /// "medium"): the text before the body, and the text after it, which ends the turn, opens the assistant's with an empty thought and
    /// stops on the answer cue.
    public static let wrapperBefore = "<|im_start|>user\n"
    public static let wrapperAfter = "<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\nAnswer: ("
    /// The last sentence of every body (BODY_TAIL).
    public static let bodyTail = "\n\nThink it through briefly, then answer with the option letter only."
    /// The most options one question can have (MAX_OPTIONS: the readout has 255 outputs).
    public static let maxOptions = 255

    /// One question as the layout shows it: the stem, the answer keys in request order, and the description shown after each key.
    public struct Question: Sendable, Equatable {
        public enum Kind: String, Sendable { case choice, score, noul }
        public let id: String
        public let kind: Kind
        public let instructions: String
        public let keys: [String]
        public let descriptions: [String]

        /// Yes or no (options_for, type "noul" without criteria): the keys "true" and "false", described as "Yes" and "No".
        public static func noul(id: String, instructions: String) -> Question {
            Question(id: id, kind: .noul, instructions: instructions, keys: ["true", "false"], descriptions: ["Yes", "No"])
        }

        /// One of several labels (type "choice"): a label with no description is described by itself, and is then shown alone.
        public static func choice(id: String, instructions: String, options: [(label: String, description: String?)]) -> Question {
            Question(id: id, kind: .choice, instructions: instructions, keys: options.map { $0.label },
                     descriptions: options.map { o in o.description.flatMap { $0.isEmpty ? nil : $0 } ?? o.label })
        }

        /// A level on a scale (type "score" with a list of levels): the keys are the levels' indices, each described by its level.
        public static func score(id: String, instructions: String, levels: [String]) -> Question {
            Question(id: id, kind: .score, instructions: instructions, keys: levels.indices.map(String.init), descriptions: levels)
        }
    }

    // MARK: the state

    /// The state as the model reads it: the authors show an object as `json.dumps(state, ensure_ascii=False)`, one line, the fields in the
    /// order given, ", " between two fields and ": " after a key.
    public static func state(_ entries: [(key: String, value: String)]) -> String {
        "{" + entries.map { jsonString($0.key) + ": " + jsonString($0.value) }.joined(separator: ", ") + "}"
    }

    /// A mail's state: its fields in the order of their keys, then its text (the app's convention, as for a label reader).
    public static func state(fields: [String: String], text: String) -> String {
        state(fields.keys.sorted().map { (key: $0, value: fields[$0]!) } + [(key: "text", value: text)])
    }

    /// Python's `json.dumps(str, ensure_ascii=False)`: the text between double quotes; a backslash, a double quote and the five named
    /// controls (\b \f \n \r \t) escaped by name, every other character under U+0020 as \u00XX in lowercase hexadecimal, all else as it is.
    static func jsonString(_ s: String) -> String {
        var out = String.UnicodeScalarView()
        out.append("\"")
        for u in s.unicodeScalars {
            switch u.value {
            case 0x22: out.append(contentsOf: "\\\"".unicodeScalars)
            case 0x5C: out.append(contentsOf: "\\\\".unicodeScalars)
            case 0x08: out.append(contentsOf: "\\b".unicodeScalars)
            case 0x0C: out.append(contentsOf: "\\f".unicodeScalars)
            case 0x0A: out.append(contentsOf: "\\n".unicodeScalars)
            case 0x0D: out.append(contentsOf: "\\r".unicodeScalars)
            case 0x09: out.append(contentsOf: "\\t".unicodeScalars)
            case 0x00..<0x20: out.append(contentsOf: ("\\u00" + (u.value < 0x10 ? "0" : "") + String(u.value, radix: 16)).unicodeScalars)
            default: out.append(u)
            }
        }
        out.append("\"")
        return String(out)
    }

    // MARK: the prompt

    /// The body of one question (Prompter.body): the state under "Context:", the stem after "Question:", one line per option, "(code) key"
    /// when the description is the key itself, else "(code) key: description"; white space at the very end is cut (the authors' `rstrip()`),
    /// then the last sentence. `codes` are the model's option codes in order (decision_config.json `codes`), at least as many as the options.
    public static func body(state: String, question: Question, codes: [String]) -> String {
        let lines = zip(codes, zip(question.keys, question.descriptions)).map { c, o in
            o.0 == o.1 ? "(\(c)) \(o.0)" : "(\(c)) \(o.0): \(o.1)"
        }
        let text = "Context:\n\(state)\n\nQuestion: \(question.instructions)\n\nOptions:\n" + lines.joined(separator: "\n")
        return trimmedEnd(text) + bodyTail
    }

    /// The whole prompt of one question (Prompter.text): the wrapper around the body. Its last token is the "(" the readout is read at.
    public static func prompt(state: String, question: Question, codes: [String]) -> String {
        wrapperBefore + body(state: state, question: question, codes: codes) + wrapperAfter
    }

    /// Python's `str.rstrip()`: the white space of `str.isspace` cut from the end.
    static func trimmedEnd(_ s: String) -> String {
        var scalars = Array(s.unicodeScalars)
        while let last = scalars.last, last.properties.isWhitespace || (0x1C...0x1F).contains(last.value) { scalars.removeLast() }
        var out = String.UnicodeScalarView(); out.append(contentsOf: scalars)
        return String(out)
    }

    // MARK: reading the answer

    /// The probabilities of the options from the readout's logits of their codes, at temperature T (the authors' `_softmax`): nil when
    /// none is finite.
    public static func probabilities(_ logits: [Double], temperature: Double = 1) -> [Double]? {
        guard let peak = logits.map({ $0 / temperature }).max(), peak.isFinite else { return nil }
        let w = logits.map { exp($0 / temperature - peak) }
        let sum = w.reduce(0, +)
        return w.map { $0 / sum }
    }
}
