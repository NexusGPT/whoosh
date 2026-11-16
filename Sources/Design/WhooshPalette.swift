import SwiftUI

/// Native Glass colours, light and dark. Source: docs/design/boards/directions/native-glass/index.html:28-31
/// (`.s-light` / `.s-dark`), carried unchanged into v2:28-31 and reader/conversation; each token names its
/// board variable. Account colours are not here: they are data (the fixture's 30 colours), drawn only as rings.
public enum WhooshPalette {
    /// Values two tokens share inside one theme, written once.
    private enum Shared {
        static let white = RGBA(hex: 0xFFFFFF)          // light --bg, --thumb, v2 --fld; text on selection
        static let darkWhite16 = RGBA(255, 255, 255, 0.16) // dark --thumb and the dark --grim outer rim
    }

    // Content
    /// `--bg` list and reader background, flat.
    public static let background = ThemeColor(light: Shared.white, dark: RGBA(hex: 0x1E1E20))
    /// `--label` primary text.
    public static let label = ThemeColor(light: RGBA(hex: 0x1D1D1F), dark: RGBA(hex: 0xF2F2F5))
    /// `--label2` secondary text: times, read subjects, meta (increased-contrast value, passes 4.5:1).
    public static let secondaryLabel = ThemeColor(light: RGBA(hex: 0x6E6E73), dark: RGBA(hex: 0xA1A1A8))
    /// `.s-light .sb{--label2}` secondary text over the vibrant sidebar; dark keeps `--label2`.
    public static let sidebarSecondaryLabel = ThemeColor(light: RGBA(hex: 0x56565B), dark: secondaryLabel.dark)
    /// `--label3` glyphs only (empty-state art), never text.
    public static let tertiaryLabel = ThemeColor(light: RGBA(hex: 0xAEAEB2), dark: RGBA(hex: 0x5E5E64))
    /// `--sep` separators and hairline borders.
    public static let separator = ThemeColor(light: RGBA(0, 0, 0, 0.09), dark: RGBA(255, 255, 255, 0.09))
    /// `--hover` row hover, sidebar selection.
    public static let hover = ThemeColor(light: RGBA(0, 0, 0, 0.045), dark: RGBA(255, 255, 255, 0.055))
    /// `--fill` quiet fills: callout, banner, Ask pill, send button, progress track.
    public static let fill = ThemeColor(light: RGBA(0, 0, 0, 0.04), dark: RGBA(255, 255, 255, 0.065))

    // The single accent
    /// `--accent` unread dot, sparkle, Ask, progress, switches, focus.
    public static let accent = ThemeColor(light: RGBA(hex: 0x007AFF), dark: RGBA(hex: 0x0A84FF))
    /// `--sel` selected row (white text on it: 5.4:1 light, 6.0:1 dark).
    public static let selection = ThemeColor(light: RGBA(hex: 0x0064E1), dark: RGBA(hex: 0x0A5BD6))
    /// Filled controls (Done, Add, the banner button): white text on the selection blue, never on the
    /// accent (white on accent is 4.02:1 light, 3.65:1 dark). Lead ruling F1, 2026-11-07.
    public static var filledControl: ThemeColor { selection }
    /// `.r.sel *{color:#fff}` text and marks on the selection or the accent.
    public static let onAccent = ThemeColor(light: Shared.white, dark: Shared.white)
    /// reader/conversation `--mine` your own blocks in a conversation.
    public static let mine = ThemeColor(light: RGBA(0, 122, 255, 0.08), dark: RGBA(10, 132, 255, 0.16))
    /// `--warn` the failing-account dot only; words beside it carry the meaning.
    public static let warning = ThemeColor(light: RGBA(hex: 0xFF9500), dark: RGBA(hex: 0xFF9F0A))

    // Sidebar (vibrant material, not glass)
    /// `--sbtint` tint laid over the blurred desktop. Board colour; opacity raised from the board's .66 / .62
    /// to the lowest value that keeps sidebar text at 4.5:1 over BOTH a white and a black desktop
    /// (worst case 4.56:1 light, 4.54:1 dark; docs/lanes/designtokens.md "Round 3"). 17 % / 10 % of the
    /// desktop still shows through.
    public static let sidebarTint = ThemeColor(light: RGBA(246, 246, 248, 0.83), dark: RGBA(34, 34, 38, 0.90))
    /// `--sbedge` the sidebar's trailing edge.
    public static let sidebarEdge = ThemeColor(light: RGBA(0, 0, 0, 0.08), dark: RGBA(0, 0, 0, 0.5))

    // Controls
    /// `--thumb` selected segment of the glass split control.
    public static let thumb = ThemeColor(light: Shared.white, dark: Shared.darkWhite16)
    /// `--key` key-cap fill in the trainer hint.
    public static let keyCap = ThemeColor(light: RGBA(hex: 0xF2F2F4), dark: RGBA(hex: 0x3A3A3E))
    /// v2 `--panel` sheets and the settings window.
    public static let panel = ThemeColor(light: RGBA(hex: 0xF6F6F8), dark: RGBA(hex: 0x262628))
    /// v2 `--fld` fields and cards inside a panel.
    public static let field = ThemeColor(light: Shared.white, dark: RGBA(255, 255, 255, 0.07))
    /// v2 `.dim` scrim under a sheet (one value in both themes).
    public static let sheetScrim = ThemeColor(light: RGBA(0, 0, 0, 0.42), dark: RGBA(0, 0, 0, 0.42))
    /// v2 `.dim.lite` lighter scrim under the settings window (one value in both themes).
    public static let windowScrim = ThemeColor(light: RGBA(0, 0, 0, 0.30), dark: RGBA(0, 0, 0, 0.30))
    /// v2 `.st .nm.ed` focus halo around an edited field.
    public static let focusHalo = ThemeColor(light: RGBA(10, 132, 255, 0.35), dark: RGBA(10, 132, 255, 0.35))

    // The in-app Entra guide (A90): its own board's values, one home (the token check reads every feature file).
    /// the guide board's `--ok` (a valid ID, Copied)
    public static let guideOK = ThemeColor(light: RGBA(hex: 0x28A745), dark: RGBA(hex: 0x32D74B))
    /// the Entra page's own paper (a picture, so it is the same in both themes)
    public static let guidePaper = ThemeColor(light: RGBA(hex: 0xF3F2F1), dark: RGBA(hex: 0xF3F2F1))
    /// the aurora band at the guide's end, four stops left to right (the first-start Aurora stage's colours)
    public static let guideAurora: [ThemeColor] = [
        ThemeColor(light: RGBA(110, 156, 255, 1), dark: RGBA(79, 122, 255, 1)),
        ThemeColor(light: RGBA(168, 140, 255, 1), dark: RGBA(138, 92, 255, 1)),
        ThemeColor(light: RGBA(255, 156, 204, 1), dark: RGBA(240, 84, 158, 1)),
        ThemeColor(light: RGBA(255, 199, 161, 1), dark: RGBA(255, 158, 107, 1)),
    ]
    /// the drawn browser chrome above a guide picture (greys of a light window frame; the same in both themes)
    public static let guideChromeDot = ThemeColor(light: RGBA(209, 209, 209, 1), dark: RGBA(209, 209, 209, 1))
    public static let guideChromeLock = ThemeColor(light: RGBA(140, 140, 140, 1), dark: RGBA(140, 140, 140, 1))
    public static let guideChromeHost = ThemeColor(light: RGBA(51, 51, 51, 1), dark: RGBA(51, 51, 51, 1))
    public static let guideChromeLocation = ThemeColor(light: RGBA(102, 102, 102, 1), dark: RGBA(102, 102, 102, 1))
    public static let guideChromeBar = ThemeColor(light: RGBA(250, 250, 250, 1), dark: RGBA(250, 250, 250, 1))
    public static let guideChromeEdge = ThemeColor(light: RGBA(237, 237, 237, 1), dark: RGBA(237, 237, 237, 1))

    // The labelling flow inspector (A94, board labels-flow-r1/base.css): a fact's yes and no tones, the AI card's indigo, the exit card's teal.
    public static let factYes = ThemeColor(light: RGBA(hex: 0x1B8A3C), dark: RGBA(hex: 0x34C759))
    public static let factNo = ThemeColor(light: RGBA(hex: 0xB3261E), dark: RGBA(hex: 0xFF6961))
    public static let flowIndigo = ThemeColor(light: RGBA(hex: 0x5E5CE6), dark: RGBA(hex: 0x5E5CE6))
    public static let flowTeal = ThemeColor(light: RGBA(hex: 0x30B0C7), dark: RGBA(hex: 0x30B0C7))

    /// Exposed for WhooshGlass, which owns the rim tokens.
    static let sharedDarkWhite16 = Shared.darkWhite16
}
