import SwiftUI

/// The rig's stand-in for system materials. In the hidden rig (`HiddenRig.isOn`, Sources/App/HiddenRig.swift)
/// `shot` renders the window's view tree, which draws no system material: a glass panel would come out empty and
/// its text bare over whatever lies behind. There, every Whoosh material draws an OPAQUE underlay in the window
/// background colour under the same tint, rim, hairline and shadow, so a shot shows each panel's real bounds and
/// contrast. Geometry is identical. The owner's build (rig off) always draws the real system material.
public enum WhooshRigStandIn {
    /// The one switch: true only in the hidden rig on macOS.
    @MainActor public static var isOn: Bool {
        #if os(macOS)
        return HiddenRig.isOn
        #else
        return false
        #endif
    }
    /// The opaque underlay: the window's content background.
    public static var underlay: ThemeColor { WhooshPalette.background }
}

public extension View {
    /// A system material in `shape`, or the rig's opaque stand-in. A feature that fills a system material
    /// directly (`.fill(.regularMaterial)`, an NSVisualEffectView) adopts this to show in rig shots.
    func whooshSystemMaterial<S: Shape>(_ material: Material, in shape: S) -> some View {
        modifier(WhooshSystemMaterialModifier(material: material, shape: shape))
    }
}

struct WhooshSystemMaterialModifier<S: Shape>: ViewModifier {
    let material: Material
    let shape: S

    func body(content: Content) -> some View {
        if WhooshRigStandIn.isOn {
            content.background(WhooshRigStandIn.underlay, in: shape)
        } else {
            content.background(material, in: shape)
        }
    }
}
