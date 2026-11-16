import Foundation

/// The list-row saturation cut the quiet sizes use: EMBLEM_LIST_SATURATION. A Debug build also reads
/// WHOOSH_EMBLEM_SATURATION once at launch (a value in (0, 1]) so the owner's emblem board can shoot one build at
/// several values; absent or out of range, the constant rules. Release compiles the override out.
enum EmblemListSaturation {
    static let value: Double = {
        #if DEBUG
        if let text = ProcessInfo.processInfo.environment["WHOOSH_EMBLEM_SATURATION"],
           let v = Double(text), v.isFinite, v > 0, v <= 1 { return v }
        #endif
        return WhooshConstants.EMBLEM_LIST_SATURATION
    }()
}
