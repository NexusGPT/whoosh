import SwiftUI
#if canImport(AppKit)
import AppKit
#elseif canImport(UIKit)
import UIKit
#endif

/// Where the mark is drawn. Every pixel size is a WhooshConstants value (docs/CONSTANTS.md ACCOUNT_*).
public enum AccountMarkSize: Sendable {
    case sidebar, compact, row, reply, settings
    public var points: CGFloat {
        switch self {
        case .sidebar: return CGFloat(WhooshConstants.ACCOUNT_RING_MIN_PX)
        case .compact: return CGFloat(WhooshConstants.ACCOUNT_MARK_COMPACT_PX)
        case .row: return CGFloat(WhooshConstants.ACCOUNT_MARK_ROW_PX)
        case .reply: return CGFloat(WhooshConstants.ACCOUNT_MARK_REPLY_PX)
        case .settings: return CGFloat(WhooshConstants.ACCOUNT_MARK_SETTINGS_PX)
        }
    }
}

/// What sits inside the ring: a glyph from the 20-icon library, an uploaded image, or the provider's own default.
public enum AccountMarkIcon: Equatable, Sendable {
    case library(String)
    case uploaded(path: String)
    case providerDefault

    /// The library's 20 glyph ids (native-glass v2 board `ICON`), each drawn with an SF Symbol line glyph.
    public static let libraryIds: [String: String] = [
        "briefcase": "briefcase", "house": "house", "chart": "chart.line.uptrend.xyaxis", "star": "star", "pen": "pencil",
        "bolt": "bolt", "globe": "globe", "code": "chevron.left.forwardslash.chevron.right", "bank": "building.columns",
        "heart": "heart", "sail": "sailboat", "key": "key", "mail": "envelope", "book": "book", "phone": "phone",
        "cup": "cup.and.saucer", "plane": "airplane", "flag": "flag", "camera": "camera", "leaf": "leaf",
    ]
    /// The provider default: the glyph an account shows until the owner picks one.
    public static let defaultSymbol = "envelope"
}

enum AccountColour {
    /// The account's colour is data (`#RRGGBB` from the core); a value that does not parse draws the separator grey.
    static func ring(_ hex: String) -> Color {
        let t = hex.trimmingCharacters(in: CharacterSet(charactersIn: "#"))
        guard t.count == 6, let v = UInt32(t, radix: 16) else { return WhooshPalette.separator.color(.light) }
        return RGBA(hex: v).color
    }
}

#if canImport(AppKit)
typealias PlatformImage = NSImage
private func imageView(_ i: PlatformImage) -> Image { Image(nsImage: i) }
#elseif canImport(UIKit)
typealias PlatformImage = UIImage
private func imageView(_ i: PlatformImage) -> Image { Image(uiImage: i) }
#endif

/// The account mark: ONE control for every list row, the sidebar, the reader meta line, search results, the notch and
/// the iPhone rows. A ring in the account's colour holding the account's icon; a dashed ring and an orange dot when
/// the account needs attention. Tooltip: name and address. Accessibility label: the account name; value: the address.
/// Pattern: SwiftUI Circle().strokeBorder + Image clipped to the inner circle (no package needed).
public struct AccountMark: View {
    let name: String
    let address: String
    let colour: String
    let icon: AccountMarkIcon
    let diameter: CGFloat
    var needsAttention = false
    var hasUnread = false
    var onSelection = false
    var chosen = false
    var ax: String?
    private let markSize: AccountMarkSize

    public init(name: String, address: String, colour: String, icon: AccountMarkIcon, size: AccountMarkSize = .row,
                needsAttention: Bool = false, hasUnread: Bool = false, onSelection: Bool = false, chosen: Bool = false) {
        self.name = name; self.address = address; self.colour = colour; self.icon = icon; diameter = size.points; markSize = size
        self.needsAttention = needsAttention; self.hasUnread = hasUnread; self.onSelection = onSelection; self.chosen = chosen
    }

    public init(account: Account, uploadedPath: String? = nil, size: AccountMarkSize = .row, needsAttention: Bool = false,
                hasUnread: Bool = false, onSelection: Bool = false, chosen: Bool = false) {
        let icon: AccountMarkIcon
        if let uploadedPath { icon = .uploaded(path: uploadedPath) }
        else if let id = account.iconId { icon = .library(id) }
        else { icon = .providerDefault }
        self.init(name: account.displayName, address: account.address, colour: account.colour, icon: icon, size: size,
                  needsAttention: needsAttention, hasUnread: hasUnread, onSelection: onSelection, chosen: chosen)
    }

    private var stroke: CGFloat { WhooshAccountMark.stroke(diameter) }
    private var ringColour: Color { onSelection ? WhooshPalette.onAccent.color(.light) : AccountColour.ring(colour) }

    /// The owner's pick (2026-11-07): every account mark is the emblem (Sources/Design/Emblem). This view keeps its
    /// API so every caller switches at once; the ring drawing below stays for the chosen halo and the hidden toggle.
    /// Each mark size maps to the emblem size of the same name, so the quiet saturation rule reaches the sidebar foot and
    /// the compact marks too (2026-11-08: `.points(diameter)` sent every non-row mark down the full-saturation path).
    private var emblemSize: EmblemSize {
        switch markSize {
        case .row: return .row
        case .sidebar: return .sidebar
        case .compact: return .compact
        case .reply: return .reply
        case .settings: return .settings
        }
    }
    private var emblemGlyphId: String? { if case .library(let id) = icon { return id } else { return nil } }
    private var emblemImagePath: String? { if case .uploaded(let path) = icon { return path } else { return nil } }

    /// The mark's measurable id on one surface (frame and radius for `measure`); without one it keeps the shared `account-mark` identifier.
    public func axId(_ id: String?) -> AccountMark { var c = self; c.ax = id; return c }

    public var body: some View {
        EmblemMark(glyph: EmblemLibrary.resolve(emblemGlyphId), colourHex: colour, size: emblemSize,
                   state: needsAttention ? .needsSignIn : .live, imagePath: emblemImagePath, name: name, address: address)
            .frame(width: diameter, height: diameter)
            .background { if chosen { chosenHalo } }
            .accessibilityIdentifier("account-mark")
            .modifier(ComponentAx(id: ax, radius: diameter / 2))
    }

    /// The ring drawing of round 1, kept for reference and the component gallery's "before" cell.
    public var ringBody: some View {
        ZStack {
            Circle().strokeBorder(ringColour, style: StrokeStyle(lineWidth: stroke, dash: needsAttention ? [CGFloat(WhooshConstants.ACCOUNT_RING_DASH_PX)] : []))
            glyph
        }
        .frame(width: diameter, height: diameter)
        .overlay(alignment: .topTrailing) { if needsAttention { badge } }
        .background { if chosen { chosenHalo } }
        .help("\(name) · \(address)")
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(name)
        .accessibilityValue(address)
    }

    @ViewBuilder private var glyph: some View {
        let slot = WhooshAccountMark.iconSlot(diameter)
        let tint: AnyShapeStyle = onSelection ? AnyShapeStyle(WhooshPalette.onAccent)
            : (hasUnread ? AnyShapeStyle(WhooshPalette.label) : AnyShapeStyle(WhooshPalette.secondaryLabel))
        switch icon {
        case .uploaded(let path):
            if let image = PlatformImage(contentsOfFile: path) {
                imageView(image).resizable().interpolation(.high).scaledToFill()
                    .frame(width: diameter - stroke * 2, height: diameter - stroke * 2).clipShape(Circle())
            } else { symbol(AccountMarkIcon.defaultSymbol, slot, tint) }
        case .library(let id):
            symbol(AccountMarkIcon.libraryIds[id] ?? AccountMarkIcon.defaultSymbol, slot, tint)
        case .providerDefault:
            symbol(AccountMarkIcon.defaultSymbol, slot, tint)
        }
    }

    private func symbol(_ name: String, _ slot: CGFloat, _ tint: AnyShapeStyle) -> some View {
        Image(systemName: name).font(.system(size: slot, weight: diameter <= WhooshAccountMark.Size.row ? .semibold : .medium))
            .foregroundStyle(tint).frame(width: slot, height: slot)
    }

    /// `.mk.bad::after`: 6 pt orange dot, 3 pt outside the top-right corner, with a 1.5 pt background halo.
    private var badge: some View {
        Circle().fill(WhooshPalette.warning).frame(width: WhooshAccountMark.badgeDot, height: WhooshAccountMark.badgeDot)
            .padding(WhooshAccountMark.badgeHalo).background(Circle().fill(WhooshPalette.background))
            .offset(x: WhooshAccountMark.badgeOutset + WhooshAccountMark.badgeHalo, y: -(WhooshAccountMark.badgeOutset + WhooshAccountMark.badgeHalo))
    }

    /// `.mk.pick`: a 2 pt background gap, then a 1.5 pt accent ring.
    private var chosenHalo: some View {
        Circle().strokeBorder(WhooshPalette.accent, lineWidth: WhooshAccountMark.pickWidth)
            .background(Circle().fill(WhooshPalette.background))
            .padding(-(WhooshAccountMark.pickGap + WhooshAccountMark.pickWidth))
    }
}
