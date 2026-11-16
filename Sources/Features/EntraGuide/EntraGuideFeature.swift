import SwiftUI

/// The entra-guide lane: the in-app guide "Get your organisation’s app ID" (mac-entra-guide). It opens from "How to get one" on the
/// add-account sheet and grows that sheet to 880 pt; Save and sign in hands the pasted ID back to the sheet. This feature also claims the
/// surface's own window (`shot mac-entra-guide`), registers its rows, clicks and keys.
public enum EntraGuideFeature: FeatureModule {
    public static let id = "entraguide"
    public static let surfaces: [String] = [EntraGuideWords.surface]
    static let A = EntraGuideWords.surface

    public static func view(for slot: SurfaceSlot) -> AnyView? { slot.id == A ? AnyView(EntraGuideSurfaceView()) : nil }

    public static func hasFocus(_ m: AppModel) -> Bool { EntraGuideModel.shared.isOpen || m.surface == A }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "\(A).state": { _ in EntraGuideRows.state() }, "\(A).step": { _ in EntraGuideRows.step() }, "\(A).steps": { _ in EntraGuideRows.steps() },
            "\(A).controls": { _ in EntraGuideRows.controls() }, "\(A).values": { _ in EntraGuideRows.values() }, "\(A).paste": { _ in EntraGuideRows.paste() },
            "\(A).progress": { _ in EntraGuideRows.progress() }, "\(A).plate": { _ in EntraGuideRows.plate() },
        ]
    }

    /// Every click acts on the guide while it is open and answers false (did nothing) while it is not; `open` opens it at step 1.
    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        let model = { EntraGuideModel.shared }
        var out: [String: (AppModel) -> Bool] = [
            "\(A).open": { m in m.surface = A; if !model().isOpen { model().open() }; return true },
            "\(A).next": { _ in model().step.n > 0 && !model().isLast ? model().next() : false },
            "\(A).save": { _ in model().isLast ? model().save() : false },
            "\(A).back": { _ in model().back() },
            "\(A).skip": { _ in model().skip() },
            "\(A).close": { _ in model().close() },
            "\(A).paste.field": { _ in model().focusPaste() },
            "\(A).paste.clear": { _ in model().clearPasted() },
        ]
        for n in 1...3 { out["\(A).copy.\(n)"] = { _ in model().copy(n) } }
        for n in 1...40 { out["\(A).goto.\(n)"] = { _ in model().jump(to: n) } }   // the rig's door: any step, for the shots
        return out
    }

    /// Return, Esc, ⌘[, ⌘↓ and the Copy keys while the guide is up. A key it has no use for returns false and passes on.
    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        let model = { EntraGuideModel.shared }
        // while the guide shrinks away it still owns Return and Esc, so the sheet under it never sees them
        var out: [String: (AppModel) -> Bool] = [
            "Enter": { _ in model().leaving ? true : model().next() }, "Escape": { _ in model().leaving ? true : model().close() },
            "⌘[": { _ in model().back() }, "⌘Down": { _ in model().skip() },
        ]
        for n in 1...3 { out["⌘\(n)"] = { _ in model().copy(n) } }
        return out
    }

    public static func typeText(_ text: String, _ m: AppModel) -> Bool { EntraGuideModel.shared.type(text) }
}

/// The surface's own window: the guide over a scrim on the panel colour (opened at step 1 when the window shows).
struct EntraGuideSurfaceView: View {
    @State private var model = EntraGuideModel.shared
    var body: some View {
        ZStack { Rectangle().fill(WhooshPalette.panel).ignoresSafeArea(); EntraGuideOverlay(model: model) }
            .onAppear { if !model.isOpen { model.open() } }
            .accessibilityIdentifier(EntraGuideWords.surface)
    }
}

/// Presents the guide over the main window while the surface is mac-entra-guide (one line in the root view's overlay).
public struct EntraGuideSheetHost: View {
    @State private var app = AppModel.shared
    @State private var model = EntraGuideModel.shared
    public init() {}
    public var body: some View {
        ZStack { if app.surface == EntraGuideWords.surface { EntraGuideOverlay(model: model).onAppear { if !model.isOpen { model.open() } } } }
    }
}
