import Foundation

/// The always-on clean-up: fillers out, a capital and a full stop, on this device, in microseconds. It runs for every dictation, so the
/// words are tidy with no provider and no network; a provider pass (`ProviderCleaner`) may follow it when the owner has one.
///
/// The English rules (fillers, "I", months and weekdays) apply only when the app speaks English: the recogniser is multilingual and
/// the only language signal here is the app's own (`language`). Any other language gets the universal rules only: spaces, the first
/// capital, the closing full stop. A language-specific filler list is the provider pass's job.
public struct LocalCleaner: TextCleaner {
    let language: @Sendable () -> String
    public init(language: @escaping @Sendable () -> String) { self.language = language }

    static let fillers: Set<String> = ["um", "umm", "uh", "uhh", "uhm", "erm", "hmm"]
    static let prepositions: Set<String> = ["in", "on", "by", "until", "till", "from", "since", "during", "before", "after", "of", "this", "next", "last", "early", "late", "mid"]
    static let alwaysCapital = ["january", "february", "april", "june", "july", "august", "september", "october", "november", "december",
                                "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"]
    static let sometimesCapital = ["march", "may"]
    /// "i'm", "i'll", "i've", "i'd", with the straight and the curly apostrophe.
    static let contractions: Set<String> = Set(["m", "ll", "ve", "d"].flatMap { tail in ["'", "\u{2019}"].map { "i" + $0 + tail } })
    static let enders: Set<Character> = [".", "!", "?", "…", "。", "！", "？"]

    public func clean(_ raw: String) async throws -> String {
        let lang = language()
        var words = raw.split(whereSeparator: { $0.isWhitespace }).map(String.init)
        if lang == "en" {
            if words.count > 2, Self.bare(words[0]) == "you", Self.bare(words[1]) == "know" { words.removeFirst(2) }
            words = words.filter { !Self.fillers.contains(Self.bare($0)) }
        }
        if words.isEmpty { return "" }
        if lang == "en" { words = Self.englishCase(words) }
        var text = words.joined(separator: " ")
        text = Self.capitalised(text)
        if let last = text.last, !Self.enders.contains(last) { text += (lang == "ja" || lang == "zh-Hans") ? "。" : "." }
        return text
    }

    /// The word without the punctuation around it, in lower case.
    static func bare(_ w: String) -> String { w.trimmingCharacters(in: .punctuationCharacters).lowercased() }

    static func englishCase(_ words: [String]) -> [String] {
        var out = words
        for i in out.indices {
            let b = bare(out[i])
            let tail = String(out[i].dropFirst(out[i].prefix(while: { $0.isPunctuation }).count + b.count))   // punctuation after the word
            let head = String(out[i].prefix(while: { $0.isPunctuation }))
            let upper: Bool
            if b == "i" || contractions.contains(b) { upper = true }
            else if alwaysCapital.contains(b) { upper = true }
            else if sometimesCapital.contains(b) {
                let before = i > 0 ? bare(out[i - 1]) : ""
                let after = i + 1 < out.count ? out[i + 1] : ""
                upper = prepositions.contains(before) || after.first?.isNumber == true
            } else { upper = false }
            if upper { out[i] = head + b.prefix(1).uppercased() + String(b.dropFirst()) + tail }
        }
        return out
    }

    /// The first letter, and the first letter after a sentence end, in capitals.
    static func capitalised(_ s: String) -> String {
        var out = "", atStart = true
        for ch in s {
            if atStart, ch.isLetter { out += String(ch).uppercased(); atStart = false; continue }
            out.append(ch)
            if enders.contains(ch) { atStart = true } else if !ch.isWhitespace, !ch.isPunctuation { atStart = false }
        }
        return out
    }
}

/// Runs the cleaners in order; a failing or empty later one leaves the earlier result standing, so an owner's words are never lost.
public struct ChainedCleaner: TextCleaner {
    let steps: [any TextCleaner]
    public init(_ steps: [any TextCleaner]) { self.steps = steps }
    public func clean(_ raw: String) async throws -> String {
        var text = raw
        for s in steps {
            if let next = try? await s.clean(text), !next.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { text = next }
        }
        return text
    }
}
