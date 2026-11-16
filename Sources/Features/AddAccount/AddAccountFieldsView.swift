import SwiftUI
#if os(macOS)
import AppKit
#endif

/// The two fields: 48 pt, calm, never moving. The Email field carries the provider's mark once the domain is recognised; Password carries the eye.
struct AddAccountFieldsView: View {
    @Bindable var model: AddAccountModel
    let narrow: Bool
    @FocusState private var focus: AddAccountModel.Field?
    @State private var driven = false
    /// The fields' own width: at the Welcome window's minimum size (card 292 pt) "Password or app password" no longer fits beside the eye and the field cuts it
    /// mid-word with no ellipsis (judge, main92, 960x600). Below `fitsLongPlaceholder` the placeholder is the shorter sentence the Welcome rows already use; at 1280 (332 pt) nothing changes.
    @State private var width: CGFloat = 400
    private static let fitsLongPlaceholder: CGFloat = 320
    private var passwordPlaceholder: String { width < Self.fitsLongPlaceholder ? Words.text("App password") : AddAccountWords.passwordPlaceholder }
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(spacing: AddAccountMetrics.fieldGap) {
            row(AddAccountWords.email, focused: focus == .address) {
                TextField(AddAccountWords.addressPlaceholder, text: Binding(get: { model.address }, set: { typed($0, .address) }))
                    .textFieldStyle(.plain).focused($focus, equals: .address).accessibilityIdentifier("\(AddAccountWords.surface).address")
                    // Return in a field is the card's Return (Continue): a focused text keeps the key, so the card's own binding never saw it
                    // (owner, 16 Nov 2026 on main203, the quick start: "I cannot press enter").
                    .onSubmit { _ = model.primaryAction() }
            } trailing: {
                ZStack { Color.clear.frame(width: AddAccountMetrics.fieldChip, height: AddAccountMetrics.fieldChip); if let k = model.fieldKind { MarkChip(kind: k, diameter: AddAccountMetrics.fieldChip) } }
            }
            row(AddAccountWords.password, focused: focus == .password) {
                Group {
                    if model.revealed { TextField(passwordPlaceholder, text: Binding(get: { model.password }, set: { typed($0, .password) })) }
                    else { SecureField(passwordPlaceholder, text: Binding(get: { model.password }, set: { typed($0, .password) })) }
                }
                .textFieldStyle(.plain).focused($focus, equals: .password).accessibilityIdentifier("\(AddAccountWords.surface).password")
                .onSubmit { _ = model.primaryAction() }
            } trailing: {
                Button { _ = model.toggleReveal() } label: {
                    Image(systemName: model.revealed ? "eye.slash" : "eye").font(.system(size: 14)).foregroundStyle(WhooshPalette.secondaryLabel).frame(width: 20, height: 20)
                }
                .buttonStyle(.plain).disabled(model.isWorking)
                .help(model.revealed ? AddAccountWords.hidePassword : AddAccountWords.showPassword)
                .accessibilityLabel(model.revealed ? AddAccountWords.hidePassword : AddAccountWords.showPassword)
                .accessibilityIdentifier("\(AddAccountWords.surface).reveal")
            }
        }
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
        .opacity(model.isWorking ? 0.5 : 1).disabled(model.isWorking)
        .onChange(of: model.focus) { _, f in if f == .address || f == .password, f != focus { driven = true; focus = f } }
        .onChange(of: model.refusalTick) { _, _ in model.setFocus(.password); if focus != .password { driven = true; focus = .password }; selectAll() }
        .onAppear { if model.style == .sheet, model.stage == .entry, model.focus == nil { model.setFocus(.address); driven = true; focus = .address } else if let f = model.focus, f == .address || f == .password { focus = f } }
        .animation(AddAccountMetrics.motion(AddAccountMetrics.swap, reduceMotion), value: model.isWorking)
    }

    /// The person typed: the typing goes to this field. Only a changed text moves the model's focus (a field losing focus writes its text back unchanged).
    private func typed(_ new: String, _ f: AddAccountModel.Field) {
        let old = model.text(f)
        guard new != old else { return }
        model.setFocus(f)
        if f == .address { model.address = new; model.addressEdited() } else { model.password = new }
    }

    /// After a refusal the wrong password is selected, so typing the right one replaces it.
    private func selectAll() {
        #if os(macOS)
        DispatchQueue.main.async { _ = (NSApp as NSApplication?)?.sendAction(#selector(NSText.selectAll(_:)), to: nil, from: nil) }
        #endif
    }

    private func row<C: View, T: View>(_ label: String, focused: Bool, @ViewBuilder content: () -> C, @ViewBuilder trailing: () -> T) -> some View {
        HStack(spacing: WhooshSpacing.s10) {
            Text(label).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.secondaryLabel).frame(width: 72, alignment: .leading)
            content().font(.system(size: 15)).foregroundStyle(WhooshPalette.label)
            trailing()
        }
        .padding(.horizontal, 14).frame(height: AddAccountMetrics.fieldHeight(narrow))
        .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: AddAccountMetrics.fieldRadius, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: AddAccountMetrics.fieldRadius, style: .continuous).strokeBorder(focused ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(WhooshPalette.sheetFieldBorder), lineWidth: 1))
        .overlay { if focused { RoundedRectangle(cornerRadius: AddAccountMetrics.fieldRadius, style: .continuous).stroke(WhooshPalette.focusHalo, lineWidth: 3).padding(-1.5) } }
        .contentShape(Rectangle())
    }
}
