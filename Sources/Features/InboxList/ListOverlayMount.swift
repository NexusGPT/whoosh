import SwiftUI

/// The mount point for a picker another feature draws over the list (repo-shape rule 3: a registry, never a hub edit).
/// A feature type conforms in its own folder; the list asks every registered feature for the overlay `AppModel.overlay`
/// names and draws the first answer above the rows. e.g. the labels lane answers "move" (V) with its move picker.
@MainActor
public protocol ListOverlayProviding {
    /// The view for this overlay id, or nil when the feature does not draw it.
    static func listOverlay(_ id: String, model: AppModel) -> AnyView?
}

@MainActor
enum ListOverlays {
    static func view(_ m: AppModel) -> AnyView? {
        guard let id = m.overlay else { return nil }
        for f in FeatureRegistry.all {
            if let p = f as? any ListOverlayProviding.Type, let v = p.listOverlay(id, model: m) { return v }
        }
        return nil
    }
}

/// Draws the mounted overlay at the top of the list, scaling in from its anchor (docs/MOTION.md rule 6: 0.96 to 1.0).
struct ListOverlayHost: View {
    let model: AppModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let mounted = ListOverlays.view(model)
        ZStack(alignment: .top) {
            if let mounted {
                mounted
                    .transition(reduceMotion ? .opacity : .opacity.combined(with: .scale(scale: CGFloat(WhooshConstants.POPOVER_SCALE_FROM), anchor: .top)))
                    .whooshAx("mac-main.list.overlay")
            }
        }
        .animation(WhooshMotion.animation(.easeOut(duration: Double(WhooshConstants.POPOVER_MS) / 1000), reduceMotion: reduceMotion), value: model.overlay)
    }
}
