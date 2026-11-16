import SwiftUI

/// A key cap: the key a control answers to, shown beside its words.
struct EntraGuideKeyCap: View {
    let key: String
    var body: some View {
        Text(key).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.label)
            .padding(.horizontal, 5).frame(minWidth: 20, minHeight: 18)
            .background(WhooshPalette.keyCap, in: RoundedRectangle(cornerRadius: 5, style: .continuous))
    }
}

/// The board's `.bt`: 28 pt, standard (grey fill), default (the selection blue) or plain (words only).
struct EntraGuideButtonStyle: ButtonStyle {
    enum Look { case standard, primary, plain }
    let look: Look
    var minWidth: CGFloat?
    @Environment(\.isEnabled) private var enabled

    func makeBody(configuration: Configuration) -> some View {
        let shape = RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous)
        configuration.label
            .foregroundStyle(look == .primary ? AnyShapeStyle(WhooshPalette.onAccent) : look == .plain ? AnyShapeStyle(WhooshPalette.secondaryLabel) : AnyShapeStyle(WhooshPalette.label))
            .padding(.horizontal, look == .plain ? 4 : 12).frame(minWidth: minWidth).frame(height: EntraGuideMetrics.buttonHeight)
            .background {
                switch look {
                case .primary: shape.fill(WhooshPalette.filledControl)
                case .standard: shape.fill(WhooshPalette.fill).overlay(shape.strokeBorder(WhooshPalette.separator, lineWidth: 0.5))
                case .plain: Color.clear
                }
            }
            .overlay { if configuration.isPressed { shape.fill(Color.black.opacity(0.10)) } }
            .opacity(enabled ? 1 : 0.42)
            .contentShape(Rectangle())
    }
}

/// One footer or head button: its words and its key, drawn the way the board draws them.
struct EntraGuideButton: View {
    let id: String
    let title: String
    var look: EntraGuideButtonStyle.Look = .standard
    var minWidth: CGFloat?
    var enabled = true
    let action: () -> Void

    var body: some View {
        let key = EntraGuideWords.key(id)
        Button(action: action) {
            HStack(spacing: 8) {
                Text(title).whooshText(WhooshTypography.body, emphasized: look == .primary).id(title).transition(.opacity)
                if !key.isEmpty {
                    if look == .plain { EntraGuideKeyCap(key: key) }
                    else { Text(key).whooshText(WhooshTypography.caption).foregroundStyle(look == .primary ? AnyShapeStyle(Color.white.opacity(0.72)) : AnyShapeStyle(WhooshPalette.secondaryLabel)) }
                }
            }
        }
        .buttonStyle(EntraGuideButtonStyle(look: look, minWidth: minWidth))
        .disabled(!enabled)
        .help(key.isEmpty ? title : EntraGuideWords.tooltip(title, key))
        .accessibilityIdentifier("\(EntraGuideWords.surface).\(id)")
    }
}
