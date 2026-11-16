import SwiftUI

/// The sheet: the add-account sheet grown to 880 pt. The glass, the 0.82 backing and the shadow are the add-account sheet's own; inside,
/// the plate fills the height that is left and the words sit under it at a fixed height.
struct EntraGuideSheetView: View {
    @Bindable var model: EntraGuideModel

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: EntraGuideMetrics.radius, style: .continuous)
        VStack(alignment: .leading, spacing: 0) {
            EntraGuideHeadView(model: model)
            EntraGuideProgressView(model: model).padding(.top, 14)
            VStack(spacing: 0) {
                EntraGuideWhereBar(location: model.data.plate(model.step.plate)?.location ?? "")
                EntraGuidePlateView(model: model).frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            .frame(maxHeight: .infinity)
            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(Color.black.opacity(0.10), lineWidth: 1))
            .padding(.top, 14)
            EntraGuideTextView(model: model).padding(.top, 14)
            EntraGuideFooterView(model: model).padding(.top, 16)
        }
        .padding(.horizontal, EntraGuideMetrics.padSide).padding(.top, EntraGuideMetrics.padTop).padding(.bottom, EntraGuideMetrics.padBottom)
        .whooshGlass(in: shape)
        .background(shape.fill(WhooshPalette.background.opacity(0.82)))
        .shadow(color: .black.opacity(0.16), radius: 22, y: 10)
        .accessibilityElement(children: .contain).whooshAx("\(EntraGuideWords.surface).sheet")
    }
}

/// The guide over a window: the sheet grows out of the add-account sheet's rectangle (scale and opacity, 240 ms), and shrinks back on Close
/// or Save. Over the add-account sheet its own scrim is already up, so that caller passes `scrim: false`.
struct EntraGuideOverlay: View {
    @Bindable var model: EntraGuideModel
    var scrim = true
    @Environment(\.accessibilityReduceMotion) private var reduce
    @State private var entered = false

    var body: some View {
        GeometryReader { g in
            if model.isOpen {
                let narrow = EntraGuideMetrics.narrow(g.size.width)
                let top = EntraGuideMetrics.top(narrow)
                let w = min(EntraGuideMetrics.width, g.size.width - 32), h = max(g.size.height - 2 * top, 200)
                let shown = entered && !model.leaving
                ZStack(alignment: .top) {
                    if scrim { Rectangle().fill(WhooshPalette.windowScrim).contentShape(Rectangle()).onTapGesture {}.transition(.opacity) }
                    EntraGuideSheetView(model: model).frame(width: w, height: h)
                        .scaleEffect(x: shown ? 1 : EntraGuideMetrics.ghostWidth / w, y: shown ? 1 : EntraGuideMetrics.ghostHeight / h, anchor: .top)
                        .opacity(shown ? 1 : 0)
                        .padding(.top, top)
                }
                .frame(width: g.size.width, height: g.size.height, alignment: .top)
                .animation(EntraGuideMetrics.motion(EntraGuideMetrics.ease(EntraGuideMetrics.enterS), reduce), value: shown)
                .onAppear { entered = true }
                .onDisappear { entered = false }
                .accessibilityIdentifier("\(EntraGuideWords.surface).overlay")
            }
        }
        .ignoresSafeArea()
    }
}
