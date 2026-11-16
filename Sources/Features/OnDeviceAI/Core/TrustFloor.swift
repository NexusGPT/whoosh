import Foundation

/// "ask the model … (trust it from 80 %)" in docs/DECISION-MODEL.md §1: below the floor the list continues to the next entry.
public struct TrustFloor: Equatable, Sendable {
    /// 0 to 1.
    public let level: Double

    public init(level: Double) { self.level = min(1, max(0, level)) }
    public init(percent: Int) { self.init(level: Double(percent) / 100) }

    /// The words the Decisions page shows, exactly as DECISION-MODEL.md writes them.
    public var words: String { "trust it from \(Int((level * 100).rounded())) %" }

    /// Reads "trust it from 80 %" (also "80%" and "80") back into a floor; nil when no number is present.
    public static func parse(_ words: String) -> TrustFloor? {
        let digits = words.split(whereSeparator: { !$0.isNumber }).compactMap { Int($0) }
        guard let first = digits.first else { return nil }
        return TrustFloor(percent: first)
    }

    /// A verdict is trusted when its confidence reaches the floor; otherwise the next list entry decides.
    public func trusts(confidence: Double) -> Bool { confidence + 1e-12 >= level }
}

public enum Confidence {
    /// Jev and Laya style: 1 minus the normalised entropy of the answer distribution (two or more options).
    public static func fromProbabilities(_ p: [Double]) -> Double {
        let k = p.count
        guard k >= 2 else { return 1 }
        let ent = p.reduce(0.0) { $0 - $1 * log(max($1, 1e-12)) }
        return 1 - ent / log(Double(k))
    }
    /// A yes/no answer carries one number (the yes probability); its confidence is the larger side.
    public static func fromYes(_ yes: Double) -> Double { max(yes, 1 - yes) }
}
