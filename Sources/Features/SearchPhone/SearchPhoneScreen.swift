import SwiftUI

/// The iPhone search screen (layout mail-26, style glass-light, inside the inbox's frame): paper ground, the results where the inbox
/// rows were, and a glass foot docked over the keyboard (cards, filter chips, field). The text is searched as it is typed; nothing waits
/// on the network (the engine reads the local index, the meaning model runs on the phone).
///
/// The inbox's search pill presents it: `InboxPhoneScreen` mounts this view in place of its list while the pill is open
/// (docs/lanes/searchphone.md "Hand-over"). On its own it is the surface `iphone-search`.
public struct SearchPhoneScreen: View {
    var onClose: () -> Void = {}
    private let model = SearchPhoneModel.shared
    @Environment(\.colorScheme) private var scheme

    public init(onClose: @escaping () -> Void = {}) { self.onClose = onClose }

    public var body: some View {
        ZStack(alignment: .bottom) {
            PhoneGround.list.ignoresSafeArea()
            SearchPhoneResults(model: model)
            fade
            SearchPhoneBar(model: model, app: AppModel.shared)
        }
        .onAppear { model.open() }
        .onChange(of: model.active) { _, now in if !now { onClose() } }
    }

    /// `.edgeB`: the result text fades to paper before it reaches the glass.
    private var fade: some View {
        LinearGradient(stops: [.init(color: PhoneGround.list(scheme).opacity(0), location: 0), .init(color: PhoneGround.list(scheme), location: 0.72)],
                       startPoint: .top, endPoint: .bottom)
            .frame(height: PhoneMetrics.edgeFade).frame(maxHeight: .infinity, alignment: .bottom)
            .ignoresSafeArea(edges: .bottom).allowsHitTesting(false)
    }
}
