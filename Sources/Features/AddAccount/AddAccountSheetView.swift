import SwiftUI

/// The sheet: the search dropdown's glass at sheet size (the shared `whooshGlass`, backed so the window behind does not show through),
/// 560 wide, its top edge fixed, growing only downward.
struct AddAccountSheetView: View {
    let model: AddAccountModel
    let narrow: Bool

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: AddAccountMetrics.radius, style: .continuous)
        AddAccountCardView(model: model, narrow: narrow, markSurface: "add-account")
            .padding(.horizontal, AddAccountMetrics.paddingSide).padding(.top, AddAccountMetrics.paddingTop).padding(.bottom, AddAccountMetrics.paddingBottom)
            .frame(width: AddAccountMetrics.width(narrow))
            .whooshGlass(in: shape)
            .background(shape.fill(WhooshPalette.background.opacity(0.82)))
            .shadow(color: .black.opacity(0.16), radius: 22, y: 10)
            .accessibilityElement(children: .contain).accessibilityIdentifier("\(AddAccountWords.surface).sheet")
    }
}

/// The scrim and the sheet over a window: windowScrim 0.30, the sheet fades in over POPOVER_MS and scales 0.96 to 1 from its top edge.
struct AddAccountOverlay: View {
    let model: AddAccountModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        GeometryReader { g in
            let narrow = AddAccountMetrics.narrow(g.size.width)
            ZStack(alignment: .top) {
                Rectangle().fill(WhooshPalette.windowScrim).contentShape(Rectangle()).onTapGesture {}
                    .transition(.opacity).accessibilityIdentifier("\(AddAccountWords.surface).scrim")
                AddAccountSheetView(model: model, narrow: narrow).padding(.top, AddAccountMetrics.top(narrow))
                    .opacity(EntraGuideModel.shared.isOpen ? 0 : 1).allowsHitTesting(!EntraGuideModel.shared.isOpen)   // A90: the guide takes the sheet's place while open
                    .animation(.easeOut(duration: 0.12), value: EntraGuideModel.shared.isOpen)
                    .transition(reduceMotion ? .opacity : .opacity.combined(with: .scale(scale: CGFloat(WhooshConstants.POPOVER_SCALE_FROM), anchor: .top)))
                EntraGuideOverlay(model: EntraGuideModel.shared, scrim: false)   // A90: the in-app guide over the sheet's own scrim (Settings > Accounts path included)
            }
            .frame(width: g.size.width, height: g.size.height, alignment: .top)
        }
        .ignoresSafeArea()
        .background(TitleStripScrim())
    }
}

/// Presents the sheet over the main window while the surface is mac-add-account (one line in the root view's overlay).
public struct AddAccountSheetHost: View {
    @State private var app = AppModel.shared
    @State private var model = AddAccountModel.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.openURL) private var openURL
    public init() {}

    public var body: some View {
        ZStack {
            if app.surface == AddAccountWords.surface {
                AddAccountOverlay(model: model).onAppear { model.style = .sheet; model.openURL = { openURL($0) } }
            }
        }
        .animation(AddAccountMetrics.motion(AddAccountMetrics.arrive, reduceMotion), value: app.surface == AddAccountWords.surface)
    }
}

public extension View {
    /// The same sheet over any view (Settings > Accounts): the host's own flag opens it, and Done or Cancel closes it.
    func addAccountSheet(isPresented: Binding<Bool>) -> some View {
        modifier(AddAccountSheetModifier(isPresented: isPresented))
    }
}

struct AddAccountSheetModifier: ViewModifier {
    @Binding var isPresented: Bool
    @State private var model = AddAccountModel.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.openURL) private var openURL

    func body(content: Content) -> some View {
        content.overlay {
            ZStack {
                if isPresented {
                    AddAccountOverlay(model: model).onAppear { model.style = .sheet; model.onClose = { isPresented = false }; model.openURL = { openURL($0) } }
                        .onDisappear { model.onClose = nil }
                }
            }
            .animation(AddAccountMetrics.motion(AddAccountMetrics.arrive, reduceMotion), value: isPresented)
        }
    }
}

#if os(macOS)
/// The scrim also covers the window's title band: that strip belongs to the window frame, outside the content, so the same scrim colour
/// is laid over it while the sheet is up (one scrim, not a second layer on the content).
struct TitleStripScrim: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView { Anchor() }
    func updateNSView(_ v: NSView, context: Context) {}
    final class Anchor: NSView {
        private var strip: NSView?
        override func viewDidMoveToWindow() {
            strip?.removeFromSuperview(); strip = nil
            guard let w = window, let frame = w.contentView?.superview else { return }
            let band = frame.bounds.height - w.contentLayoutRect.height
            let v = NSView(frame: NSRect(x: 0, y: frame.bounds.height - max(band, 0), width: frame.bounds.width, height: max(band, 0)))
            v.autoresizingMask = [.width, .minYMargin]
            v.wantsLayer = true
            v.layer?.backgroundColor = NSColor.black.withAlphaComponent(0.30).cgColor
            frame.addSubview(v)
            strip = v
        }
        override func removeFromSuperview() { strip?.removeFromSuperview(); strip = nil; super.removeFromSuperview() }
    }
}
#else
struct TitleStripScrim: View { var body: some View { EmptyView() } }
#endif
