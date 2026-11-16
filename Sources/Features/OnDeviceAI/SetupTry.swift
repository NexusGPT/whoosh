import SwiftUI

/// A one-line way to ask the model something. It goes through the same engine every AI feature uses, so a
/// missing model, a damaged file and an over-long question each answer here in the same words.
struct SetupTry: View {
    let setup: OnDeviceSetup
    @Bindable var model: AppModel
    @FocusState private var focused: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            Text(OnDeviceWords.tryTitle).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
            HStack(spacing: WhooshSpacing.s8) {
                TextField(OnDeviceWords.tryPlaceholder, text: $model.typed).textFieldStyle(.plain).whooshText(WhooshTypography.body)
                    .focused($focused).onSubmit { setup.ask(model) }.accessibilityIdentifier("mac-ondeviceai.field")
                AIButton(title: OnDeviceWords.ask, primary: !model.typed.isEmpty, id: "mac-ondeviceai.ask") { setup.ask(model) }
            }
            .padding(.leading, WhooshSpacing.s14).padding(.trailing, WhooshSpacing.s6).padding(.vertical, WhooshSpacing.s6)
            .background(RoundedRectangle(cornerRadius: WhooshRadius.hint, style: .continuous).fill(WhooshPalette.field))
            .overlay(RoundedRectangle(cornerRadius: WhooshRadius.hint, style: .continuous)
                .stroke(focused ? WhooshPalette.focusHalo : WhooshPalette.separator, lineWidth: focused ? 2 : WhooshMetrics.separator))
            replyView
        }
        .animation(WhooshMotion.animation(WhooshMotion.readerSwap, reduceMotion: reduceMotion), value: setup.reply)
    }

    @ViewBuilder private var replyView: some View {
        switch setup.reply {
        case .none: EmptyView()
        case .thinking:
            Text(OnDeviceWords.thinking).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).transition(.opacity)
        case .answer(let text):
            Text(text).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label).textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true).transition(.opacity).accessibilityIdentifier("mac-ondeviceai.answer")
        case .refused(let why):
            HStack(alignment: .top, spacing: WhooshSpacing.s8) {
                Circle().fill(WhooshPalette.warning).frame(width: WhooshMetrics.warningDot, height: WhooshMetrics.warningDot).padding(.top, WhooshSpacing.s6)
                Text(why).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label).fixedSize(horizontal: false, vertical: true)
                    .accessibilityIdentifier("mac-ondeviceai.refusal")
            }
            .transition(.opacity)
        }
    }
}
