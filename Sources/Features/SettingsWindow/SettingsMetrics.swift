import SwiftUI

/// The Settings window's measures, in points, from the picked boards (docs/design/boards/settings-r1/settings-sb and
/// settings-r2/settings-sb-s3 CSS). The file docs/design/geometry/mac-settings.json carries the same numbers; the
/// check par-settingswindow measures the running window against that file.
enum SettingsMetrics {
    static let windowDefault = CGSize(width: CGFloat(WhooshConstants.SETTINGS_WINDOW_W_PT), height: CGFloat(WhooshConstants.SETTINGS_WINDOW_H_PT))
    static let windowMin = CGSize(width: CGFloat(WhooshConstants.SETTINGS_WINDOW_MIN_W_PT), height: CGFloat(WhooshConstants.SETTINGS_WINDOW_MIN_H_PT))
    /// `.sb` 250 at the default size, 210 at the minimum; between the two it follows the window.
    static let sidebarWide = CGFloat(WhooshConstants.SETTINGS_SIDEBAR_W_PT)
    static let sidebarMin = CGFloat(WhooshConstants.SETTINGS_SIDEBAR_MIN_W_PT)
    static func sidebarWidth(for windowWidth: CGFloat) -> CGFloat {
        min(sidebarWide, max(sidebarMin, (windowWidth * sidebarWide / windowDefault.width).rounded()))
    }
    /// The sidebar shows the accounts summary card only while the whole column fits (the six parts of Réponses count while it is open, so the card
    /// yields its height to them at the default window and comes back when Réponses closes): at the 700 x 480 minimum the card
    /// gives way, as on the board (S3 state 6).
    static func showsSummary(forHeight height: CGFloat, repliesOpen: Bool = false) -> Bool {
        let panes = CGFloat(SettingsPane.allCases.count) * paneRowHeight + (repliesOpen ? CGFloat(RepliesPage.panes.count) * childRowHeight : 0)
        let groups = CGFloat(SettingsPane.Group.allCases.count)
        let column = sidebarTop + searchHeight + 10 + 50 + (groups - 1) * (groupHeaderHeight + 8) + groupHeaderHeight + panes
        return height >= column + 12
    }
    /// `.sb .in` padding: 48 top (clears the traffic lights), 10 sides.
    static let sidebarTop: CGFloat = 48
    static let sidebarSide: CGFloat = 10
    /// `.srch` height 28, radius 8. `.sum` padding 8, radius 10. `.sbr` height 28, radius 7. `.sbh` height 22.
    static let searchHeight: CGFloat = 28
    static let searchRadius: CGFloat = 8
    static let summaryRadius: CGFloat = 10
    static let paneRowHeight: CGFloat = 28
    /// `.sbr.kid`: a part of Réponses under its parent.
    static let childRowHeight: CGFloat = 23
    static let paneRowRadius: CGFloat = 7
    static let groupHeaderHeight: CGFloat = 22
    /// `.sq` squircles: the sidebar's 20, the pane header's 40 (radius 10); 62 % symbol.
    static let paneIcon: CGFloat = 20
    static let paneIconRadius: CGFloat = 5
    static let headerIcon: CGFloat = 40
    static let headerIconRadius: CGFloat = 10
    static let iconSymbolShare: CGFloat = 0.62
    /// `.tb` toolbar 52 high, 18 side padding; `.nav` 28 high, 28 wide buttons.
    static let toolbarHeight: CGFloat = 52
    static let toolbarSide: CGFloat = 18
    static let navButton: CGFloat = 28
    /// `.cnt` padding 4 / 28 / 24; the column reads at most this wide in a very wide window.
    static let contentSide: CGFloat = 28
    static let contentTop: CGFloat = 4
    static let contentBottom: CGFloat = 24
    static let contentMax: CGFloat = 720
    /// hid-ai round 21: the fade at the top of a scrolled column, under the toolbar (SettingsScrollEdge): about one label line.
    static let scrollEdge: CGFloat = 16
    /// `.card` radius 10; `.row` min height 44, padding 8 / 14, gap 10.
    static let cardRadius: CGFloat = 10
    static let rowMin: CGFloat = 44
    static let rowSideX: CGFloat = 14
    static let rowSideY: CGFloat = 8
    /// `.pop` 24 high, radius 7; `.sw` 32 x 19, knob 16; `.btn` 26 high.
    static let popupHeight: CGFloat = 24
    static let popupRadius: CGFloat = 7
    static let switchSize = CGSize(width: 32, height: 19)
    static let buttonHeight: CGFloat = 26
    static let fieldHeight: CGFloat = 24
    /// `.mini` appearance thumbnails 62 x 40, radius 7.
    static let miniSize = CGSize(width: 62, height: 40)
    /// `.rcap` row caption 11.5, `.rt` 13; the ring around a found row: 2 pt accent, 6 pt halo.
    static let ringWidth: CGFloat = 2
    static let ringHalo: CGFloat = 4
}

/// The Settings window's own tokens: the card fill, its edge, the switch's off tone, and the ten pane hues
/// (the lock amendment of 2026-11-08 15:39: coloured pane icons, inside this window only).
enum SettingsPalette {
    /// `--card` `--cardedge`
    static let card = ThemeColor(light: RGBA(0, 0, 0, 0.028), dark: RGBA(255, 255, 255, 0.045))
    static let cardEdge = ThemeColor(light: RGBA(0, 0, 0, 0.075), dark: RGBA(255, 255, 255, 0.07))
    /// `--swoff`
    static let switchOff = ThemeColor(light: RGBA(120, 120, 128, 0.22), dark: RGBA(120, 120, 128, 0.36))
    /// `.sum` summary card fill is `--hover`; `.srch` is `--fill`.
    /// `.sb` `--sbtint` as the S3 board draws it: the desktop shows through.
    static let sidebarTint = ThemeColor(light: RGBA(246, 246, 248, 0.66), dark: RGBA(34, 34, 38, 0.62))
    static let danger = WhooshSystemHue.danger
}

/// One system hue per pane (the board's `--h`): light value from the board, dark the system's dark variant.
enum SettingsHue {
    static let gray = WhooshSystemHue.gray
    static let blue = WhooshSystemHue.blue
    static let indigo = WhooshSystemHue.indigo
    static let orange = WhooshSystemHue.orange
    static let purple = WhooshSystemHue.purple
    static let brown = WhooshSystemHue.brown
    static let pink = WhooshSystemHue.pink
    static let teal = WhooshSystemHue.teal
    static let red = WhooshSystemHue.red
    static let green = WhooshSystemHue.green
}
