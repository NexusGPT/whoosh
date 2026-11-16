import SwiftUI

/// One footer button, as the board draws it: 28 pt, the title and its key in grey; the default button is the selection blue.
struct AddAccountButton: View {
    let model: AddAccountModel
    let control: AddAccountControl

    var body: some View {
        Button { _ = model.perform(control.id) } label: {
            HStack(spacing: 8) {
                if control.busy { ProgressView().controlSize(.small).scaleEffect(0.6).frame(width: 11, height: 11).colorScheme(.dark) }
                Text(control.title).whooshText(WhooshTypography.body, emphasized: control.look == .primary).lineLimit(1).fixedSize(horizontal: true, vertical: false)
                if control.chevron { Image(systemName: model.manualOpen ? "chevron.up" : "chevron.down").font(.system(size: 10, weight: .semibold)) }
                if let k = control.key { Text(k).whooshText(WhooshTypography.caption).foregroundStyle(control.look == .primary ? AnyShapeStyle(Color.white.opacity(0.72)) : AnyShapeStyle(WhooshPalette.secondaryLabel)) }
            }
        }
        .buttonStyle(AddAccountButtonStyle(look: control.look))
        .disabled(!control.enabled)
        .modifier(AddAccountShortcut(shortcut: Self.shortcut(control.id)))
        .help(control.key.map { AddAccountWords.tooltip(control.title, $0) } ?? control.title)
        .accessibilityIdentifier("\(AddAccountWords.surface).\(control.id)")
    }

    static func shortcut(_ id: String) -> KeyboardShortcut? {
        switch id {
        case "continue", "add", "retry", "done", "keep-checking": return .defaultAction
        case "cancel", "stop": return .cancelAction
        case "add-another": return KeyboardShortcut("n", modifiers: .command)
        default: return nil
        }
    }
}

private struct AddAccountShortcut: ViewModifier {
    let shortcut: KeyboardShortcut?
    func body(content: Content) -> some View { if let shortcut { content.keyboardShortcut(shortcut) } else { content } }
}

struct AddAccountButtonStyle: ButtonStyle {
    let look: AddAccountControl.Look
    @Environment(\.isEnabled) private var enabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .foregroundStyle(look == .primary ? AnyShapeStyle(WhooshPalette.onAccent) : look == .plain ? AnyShapeStyle(WhooshPalette.secondaryLabel) : AnyShapeStyle(WhooshPalette.label))
            .padding(.horizontal, look == .plain ? 4 : 12).frame(minWidth: look == .primary ? AddAccountMetrics.defaultButtonMin : nil)
            .frame(height: AddAccountMetrics.buttonHeight)
            .background {
                let shape = RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous)
                switch look {
                case .primary: shape.fill(WhooshPalette.filledControl)
                case .standard: shape.fill(WhooshPalette.fill).overlay(shape.strokeBorder(WhooshPalette.separator, lineWidth: 0.5))
                case .plain: Color.clear
                }
            }
            .overlay { if configuration.isPressed { RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous).fill(Color.black.opacity(0.10)) } }
            .opacity(enabled ? 1 : 0.42)
            .contentShape(Rectangle())
    }
}
