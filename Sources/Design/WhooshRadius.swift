import CoreGraphics

/// Corner radii, points. Board: native-glass v2 CSS (the class each value comes from is named).
public enum WhooshRadius {
    /// `.win` the window.
    public static let window: CGFloat = 16
    /// `.seg` `.cmd` `.cmdc` the two glass toolbar pills (32 pt tall, fully round).
    public static let glassPill: CGFloat = 16
    /// `.seg span` the split thumb; `.undo` the undo capsule (28 pt tall, fully round).
    public static let capsule28: CGFloat = 14
    /// `.sheet` sheets and the settings window; reader/conversation `.bk` blocks.
    public static let sheet: CGFloat = 14
    /// `.hint` the trainer hint; `.card` settings cards.
    public static let hint: CGFloat = 12
    /// `.callout` `.att` `.banner` `.qr .fld` `.stl` `.up` `.prv`, conversation `.ft` file tiles.
    public static let field: CGFloat = 10
    /// `.lib span` icon-library tiles.
    public static let libraryTile: CGFloat = 9
    /// `.r` list rows; `.pv` `.ta` preview and prompt boxes; `.ms` mini segmented control.
    public static let row: CGFloat = 8
    /// `.sbr` sidebar rows, `.r .acts span` row actions, `.acr` account rows, `.kv` `.vf` small fields.
    public static let control: CGFloat = 7
    /// `.aci` account items; `.ms span` mini-segment thumb.
    public static let item: CGFloat = 6
    /// `.kc` key caps.
    public static let keyCap: CGFloat = 5
    /// `.bar` the progress bar.
    public static let progress: CGFloat = 2
}
