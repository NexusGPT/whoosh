import Foundation
import SwiftUI

/// Registration for the ondeviceai lane: the "Setting up on-device AI" surface, its rows, clicks and palette command.
public enum OnDeviceAIFeature: FeatureModule {
    public static let id = "ondeviceai"
    public static let surfaces: [String] = ["mac-ondeviceai"]

    public static func view(for slot: SurfaceSlot) -> AnyView? { slot == OnDeviceAIPanel.slot ? AnyView(OnDeviceSetupView()) : nil }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        let s = OnDeviceSetup.shared
        return [
            "mac-ondeviceai.state": { _ in [OnDeviceRows.state(s)] },
            "mac-settings-ai.provider.local": { _ in [OnDeviceRows.provider(s)] },
            "mac-ondeviceai.reply": { _ in OnDeviceRows.reply(s) },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        let s = OnDeviceSetup.shared
        return [
            "mac-ondeviceai.open": { m in OnDeviceAIPanel.open(m) },
            "mac-palette.result.set-up-on-device-ai": { m in OnDeviceAIPanel.open(m) },
            "mac-ondeviceai.close": { m in OnDeviceAIPanel.close(m); return true },
            "mac-ondeviceai.install": { _ in s.install(); return true },
            "mac-ondeviceai.resume": { _ in s.install(); return true },
            "mac-ondeviceai.retry": { _ in s.install(); return true },
            "mac-ondeviceai.pause": { _ in s.pause(); return true },
            "mac-ondeviceai.cancel": { _ in s.cancel(); return true },
            "mac-ondeviceai.ask": { m in s.ask(m) },
            "mac-ondeviceai.reload": { _ in s.refresh(); return true },
        ]
    }

    /// Escape closes the step while it is up; anywhere else it passes.
    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        ["Escape": { m in guard m.surface == OnDeviceAIPanel.slot.id else { return false }; OnDeviceAIPanel.close(m); return true }]
    }

    public static func paletteCommands() -> [(title: String, key: String)] { [(OnDeviceWords.install, "")] }
}
