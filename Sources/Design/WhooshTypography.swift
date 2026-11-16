import CoreText
import SwiftUI

/// One step of the type ramp: size and line height in points, its regular and emphasised weights.
public struct TextStyleToken: Sendable, Equatable {
    public let size: CGFloat
    public let lineHeight: CGFloat
    public let weight: Font.Weight
    public let emphasis: Font.Weight
    public let tracking: CGFloat
    public let monospaced: Bool

    public var font: Font { font(emphasized: false) }
    public func font(emphasized: Bool) -> Font {
        .system(size: size, weight: emphasized ? emphasis : weight, design: monospaced ? .monospaced : .default)
    }

    /// The system font's own line height at this size (ascent + descent + leading, from CoreText).
    public var naturalLineHeight: CGFloat {
        let f = CTFontCreateUIFontForLanguage(.system, size, nil)
        guard let f else { return size }
        return CTFontGetAscent(f) + CTFontGetDescent(f) + CTFontGetLeading(f)
    }

    /// Extra space SwiftUI must add so a line occupies `lineHeight`, as the board's CSS line-height does.
    public var extraLeading: CGFloat { max(0, lineHeight - naturalLineHeight) }
}

/// Native Glass type ramp, SF Pro, tabular figures everywhere. Board: native-glass v2 "Type scale"
/// (doc table) and its CSS (`.ttl` `.empty h4` `.body p` `.r .sub` `.kc`, `.kv` for the mono key field).
public enum WhooshTypography {
    /// Thread subject in the reader: 22 / 28, 600, tracking −0.26.
    public static let largeTitle = TextStyleToken(size: 22, lineHeight: 28, weight: .semibold, emphasis: .semibold,
                                                  tracking: -0.26, monospaced: false)
    /// Empty-state and first-sync titles, sheet titles: 17 / 22, 600.
    public static let title3 = TextStyleToken(size: 17, lineHeight: 22, weight: .semibold, emphasis: .semibold,
                                              tracking: 0, monospaced: false)
    /// Reader body, list sender (600 when unread), sidebar rows, folded lines, command field: 13 / 20.
    public static let body = TextStyleToken(size: 13, lineHeight: 20, weight: .regular, emphasis: .semibold,
                                            tracking: 0, monospaced: false)
    /// List subject, split segments (600 selected), account filter, callout, undo capsule: 12 / 18.
    public static let subhead = TextStyleToken(size: 12, lineHeight: 18, weight: .regular, emphasis: .semibold,
                                               tracking: 0, monospaced: false)
    /// Times, counts, meta line, sidebar section headers (600), sync line, key caps (600): 11 / 16.
    public static let caption = TextStyleToken(size: 11, lineHeight: 16, weight: .regular, emphasis: .semibold,
                                               tracking: 0, monospaced: false)
    /// Overflow counts inside a mark-sized capsule ("+25" in the sidebar's account strip): 9 / 12; owner 2026-11-08 "should be smaller font".
    public static let caption2 = TextStyleToken(size: 9, lineHeight: 12, weight: .regular, emphasis: .semibold,
                                                tracking: 0, monospaced: false)
    /// The API-key field in Settings: SF Mono 12 on a 26 pt field.
    public static let monoField = TextStyleToken(size: 12, lineHeight: 26, weight: .regular, emphasis: .regular,
                                                 tracking: 0, monospaced: true)

    public static let ramp: [TextStyleToken] = [largeTitle, title3, body, subhead, caption, monoField]
}

public extension View {
    /// Applies a ramp step: font, tracking, tabular figures and the line height the board draws.
    func whooshText(_ style: TextStyleToken, emphasized: Bool = false) -> some View {
        font(style.font(emphasized: emphasized))
            .tracking(style.tracking)
            .monospacedDigit()
            .lineSpacing(style.extraLeading)
            .padding(.vertical, style.extraLeading / 2)
    }
}
