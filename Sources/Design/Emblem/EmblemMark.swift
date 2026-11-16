import SwiftUI

/// Where the emblem is drawn. Sizes are the ledger's ACCOUNT_MARK_* values; `hidden` is the list's "Show account
/// emblems" toggle turned off, and costs nothing (EmptyView). `points` is for a caller holding its own ledger value.
public enum EmblemSize: Equatable, Sendable {
    case settings, sidebar, compact, row, reply, hidden
    case points(CGFloat)
    public var diameter: CGFloat {
        switch self {
        case .settings: return CGFloat(WhooshConstants.ACCOUNT_MARK_SETTINGS_PX)
        case .sidebar: return CGFloat(WhooshConstants.ACCOUNT_RING_MIN_PX)
        case .compact: return CGFloat(WhooshConstants.ACCOUNT_MARK_COMPACT_PX)
        case .row: return CGFloat(WhooshConstants.ACCOUNT_MARK_ROW_PX)
        case .reply: return CGFloat(WhooshConstants.ACCOUNT_MARK_REPLY_PX)
        case .hidden: return 0
        case .points(let p): return p
        }
    }
    /// A list row draws the account colour at EMBLEM_LIST_SATURATION of its saturation (owner: "subtle enough").
    /// Quiet sizes: the row, the sidebar and the compact marks all cut saturation (owner: "subtle enough on the UI so it
    /// is not overpowering"; the sidebar lane measured the full-saturation discs as loud at 20 and 18 pt). Settings and reply keep the full colour.
    public var isListRow: Bool { self == .row || self == .sidebar || self == .compact }
}

/// The account's state, the same set WhooshAccountMark carries: live, first sync, needs a sign-in. An uploaded image
/// is the fourth case and travels as `imagePath`.
public enum EmblemState: String, Sendable { case live, syncing, needsSignIn }

/// The account emblem: a lit glass disc in the account colour carrying one two-tone pictogram (board
/// docs/design/boards/badges/emblem). Tooltip: name and address; accessibility label the name, value the address.
public struct EmblemMark: View {
    let name: String, address: String, colourHex: String
    let glyph: EmblemGlyph
    let size: EmblemSize
    let state: EmblemState
    let imagePath: String?
    @Environment(\.colorScheme) private var scheme

    public init(account: Account, size: EmblemSize = .row, state: EmblemState = .live, imagePath: String? = nil) {
        self.init(glyph: EmblemLibrary.resolve(account.iconId), colourHex: account.colour, size: size, state: state,
                  imagePath: imagePath, name: account.displayName, address: account.address)
    }

    public init(glyph: EmblemGlyph, colourHex: String, size: EmblemSize, state: EmblemState = .live, imagePath: String? = nil,
                name: String = "", address: String = "") {
        self.glyph = glyph; self.colourHex = colourHex; self.size = size; self.state = state
        self.imagePath = imagePath; self.name = name.isEmpty ? glyph.name : name; self.address = address
    }

    public var body: some View {
        if size == .hidden { EmptyView() } else { mark }
    }

    private var recipe: EmblemRecipe {
        EmblemRecipe(colourHex: colourHex, diameter: size.diameter, dark: scheme == .dark, listRow: size.isListRow)
    }

    private var mark: some View {
        EmblemDisc(recipe: recipe, glyph: glyph, image: imagePath.flatMap(EmblemImage.load))
            .overlay { if state != .live { EmblemStatusDot(state: state, recipe: recipe) } }
            .help(address.isEmpty ? name : "\(name) · \(address)")
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(name)
            .accessibilityValue(state == .needsSignIn ? "\(address), needs sign-in" : address)
            .accessibilityIdentifier("account-emblem")
    }
}

/// The failing (orange, "!") or syncing (accent, arrow) dot on the bottom-trailing edge, cut out by a background ring.
/// At the 16 pt tier the dot carries no glyph: the words in the sidebar and in Settings carry the meaning.
struct EmblemStatusDot: View {
    let state: EmblemState
    let recipe: EmblemRecipe
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let C = EmblemConstruction.self, d = recipe.diameter
        let r = d * (recipe.tier == .detailed ? C.dotDetailed : (recipe.tier == .tiny ? C.dotTiny : C.dotSmall))
        let ring = d * (recipe.tier == .detailed ? C.dotRingDetailed : (recipe.tier == .tiny ? C.dotRingTiny : C.dotRingSmall))
        let centre = d - C.dotInset * r
        let fill = (state == .needsSignIn ? WhooshPalette.warning : WhooshPalette.accent).color(scheme)
        ZStack {
            Circle().fill(WhooshPalette.background.color(scheme)).frame(width: (r + ring) * 2, height: (r + ring) * 2)
            Circle().fill(fill).frame(width: r * 2, height: r * 2)
            if recipe.tier != .tiny {
                Image(systemName: state == .needsSignIn ? "exclamationmark" : "arrow.clockwise")
                    .font(.system(size: r * 1.1, weight: .bold)).foregroundStyle(.white)
            }
        }
        .position(x: centre, y: centre)
        .frame(width: d, height: d)
        .accessibilityHidden(true)
    }
}

enum EmblemImage {
    static func load(_ path: String) -> Image? {
        #if canImport(AppKit)
        return NSImage(contentsOfFile: path).map { Image(nsImage: $0) }
        #else
        return UIImage(contentsOfFile: path).map { Image(uiImage: $0) }
        #endif
    }
}
