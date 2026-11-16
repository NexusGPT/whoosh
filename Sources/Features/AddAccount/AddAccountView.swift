import SwiftUI

/// Add account by IMAP with provider autodetection. The sheet itself is drawn by `AddAccountSheetHost` (over the main window) and
/// `addAccountSheet(isPresented:)` (over Settings); this view is the card without the sheet's chrome, for Welcome's quick start
/// (`onClose` is unused there: the card has no Cancel), and the content of the surface's own window (`shot mac-add-account`).
public struct AddAccountView: View {
    @State private var model = AddAccountModel.shared
    @Environment(\.openURL) private var openURL
    public init(onClose: (() -> Void)? = nil) { _ = onClose }

    public var body: some View {
        AddAccountCardView(model: model, narrow: false, markSurface: "welcome")
            .frame(maxWidth: 520)
            .onAppear { model.style = .inline; model.openURL = { openURL($0) } }
            .frame(maxWidth: .infinity)
    }
}

/// The surface's own window: the same sheet over a scrim, on the panel colour.
struct AddAccountSurfaceView: View {
    @State private var model = AddAccountModel.shared
    @Environment(\.openURL) private var openURL
    var body: some View {
        ZStack { Rectangle().fill(WhooshPalette.panel).ignoresSafeArea(); AddAccountOverlay(model: model) }
            .onAppear { model.style = .sheet; model.openURL = { openURL($0) } }
            .accessibilityIdentifier(AddAccountWords.surface)
    }
}
