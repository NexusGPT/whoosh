import SwiftUI

/// The system hues, light and dark, for the few places the lock lets a hue in: the Settings window's pane
/// icons (lock amendment 2026-11-08 15:39, the board's `--h`: light from the board, dark the system's dark
/// variant) and the destructive tone. Moved from Sources/Features/SettingsWindow/SettingsMetrics.swift
/// (token-check, designtokens round 7). Blue and orange are the accent and the warning: one value each.
public enum WhooshSystemHue {
    /// Values two tokens share inside one theme, written once.
    private enum Shared {
        static let redDark = RGBA(hex: 0xFF453A)   // dark red and dark danger
    }
    public static let gray = ThemeColor(light: RGBA(hex: 0x8E8E93), dark: RGBA(hex: 0x98989D))
    public static var blue: ThemeColor { WhooshPalette.accent }
    public static let indigo = ThemeColor(light: RGBA(hex: 0x5856D6), dark: RGBA(hex: 0x5E5CE6))
    public static var orange: ThemeColor { WhooshPalette.warning }
    public static let purple = ThemeColor(light: RGBA(hex: 0xAF52DE), dark: RGBA(hex: 0xBF5AF2))
    public static let brown = ThemeColor(light: RGBA(hex: 0xA2845E), dark: RGBA(hex: 0xAC8E68))
    public static let pink = ThemeColor(light: RGBA(hex: 0xFF2D55), dark: RGBA(hex: 0xFF375F))
    public static let teal = ThemeColor(light: RGBA(hex: 0x30B0C7), dark: RGBA(hex: 0x40CBE0))
    public static let red = ThemeColor(light: RGBA(hex: 0xFF3B30), dark: Shared.redDark)
    public static let green = ThemeColor(light: RGBA(hex: 0x34C759), dark: RGBA(hex: 0x30D158))
    /// Destructive text and buttons (Settings "Remove account"): the system's increased-contrast red in light.
    public static let danger = ThemeColor(light: RGBA(hex: 0xD70015), dark: Shared.redDark)
}
