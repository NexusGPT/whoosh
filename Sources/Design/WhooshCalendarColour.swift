import Foundation

/// The colours that travel as "#RRGGBB" text because the Mac's calendars and the core's reply sources carry a
/// calendar's colour as text (token-check round-fixes item 1: no hex literal outside Sources/Design).
/// A real calendar's own colour comes from EventKit at run time and is never written here; these are only
/// the two sample calendars of the fixture (Travail, Personnel) and the grey a calendar with no colour gets.
/// Each equals the light value of the system hue of the same name in WhooshSystemHue (blue 0x007AFF is the
/// accent, green, gray).
public enum WhooshCalendarColour {
    /// Fixture calendar "Travail" (system blue).
    public static let fixtureWork = "#007AFF"
    /// Fixture calendar "Personnel" (system green).
    public static let fixturePersonal = "#34C759"
    /// A calendar the system gave no usable colour (system gray).
    public static let noColour = "#8E8E93"
}
