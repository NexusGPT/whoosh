import SwiftUI

/// "Setting up on-device AI": the install step as one calm column on the panel canvas.
struct OnDeviceSetupView: View {
    @State private var setup = OnDeviceSetup.shared
    @State private var model = AppModel.shared

    var body: some View {
        ZStack {
            Rectangle().fill(WhooshPalette.panel).ignoresSafeArea()
            ScrollView {
                VStack(alignment: .leading, spacing: WhooshSpacing.s20) {
                    SetupHeader(title: OnDeviceWords.title)
                    SetupFacts(spec: setup.spec)
                    SetupStateArea(setup: setup)
                    Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator)
                    SetupTry(setup: setup, model: model)
                    HStack { Spacer(); AIButton(title: OnDeviceWords.close, id: "mac-ondeviceai.close") { OnDeviceAIPanel.close(model) } }
                }
                .frame(maxWidth: OnDeviceAIPanel.column, alignment: .leading)
                .padding(WhooshSpacing.s32)
                .frame(maxWidth: .infinity)
            }
            .scrollIndicators(.never)
        }
        .accessibilityIdentifier("mac-ondeviceai.panel")
        .onAppear { setup.refresh() }
    }
}

@MainActor
enum OnDeviceAIPanel {
    static let slot = SurfaceSlot("mac-ondeviceai")
    static let column: CGFloat = 520
    static let window = CGSize(width: 600, height: 700)

    @discardableResult
    static func open(_ m: AppModel) -> Bool {
        if m.surface != slot.id { m.typed = "" }
        OnDeviceSetup.shared.refresh()
        guard WindowBridge.open(slot, in: m) else { return false }
        #if os(macOS)
        WindowBridge.window(for: slot)?.makeKeyAndOrderFront(nil)
        #endif
        return true
    }

    static func close(_ m: AppModel) {
        #if os(macOS)
        WindowBridge.window(for: slot)?.close()
        #endif
        m.surface = SurfaceSlot.macMain.id
        WindowBridge.bringMainToFront()
    }
}
