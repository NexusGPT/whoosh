import SwiftUI

/// The ID field. On step 11 it is the slot ("Whoosh keeps it for the last step"); on the last step it is the field the guide ends on.
/// A green tick pops in (160 ms) when the text is an app ID. Both are the same text, kept by the model.
struct EntraGuidePasteView: View {
    @Bindable var model: EntraGuideModel
    let slot: Bool
    @FocusState private var focused: Bool
    @Environment(\.accessibilityReduceMotion) private var reduce

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            field
            if !slot { note }
        }
        .onAppear { if model.pasteFocus { focused = true } }
    }

    private var field: some View {
        let shape = RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous)
        return HStack(spacing: 10) {
            Text(slot ? EntraGuideWords.slotLabel : EntraGuideWords.pasteLabel).whooshText(WhooshTypography.body)
                .foregroundStyle(WhooshPalette.secondaryLabel).frame(width: 120, alignment: .leading)
            TextField(slot ? EntraGuideWords.slotPlaceholder : EntraGuideWords.pastePlaceholder, text: $model.pasted)
                .textFieldStyle(.plain).font(.system(size: 15, design: .monospaced)).focused($focused).autocorrectionDisabled()
                .onSubmit { _ = model.next() }
                .accessibilityIdentifier("\(EntraGuideWords.surface).paste.field")
            if !model.pasted.isEmpty {
                Button { _ = model.clearPasted() } label: { Image(systemName: "xmark.circle.fill").font(.system(size: 13)).foregroundStyle(WhooshPalette.tertiaryLabel) }
                    .buttonStyle(.plain).transition(.opacity).accessibilityIdentifier("\(EntraGuideWords.surface).paste.clear")
            }
            tick
        }
        .padding(.leading, 14).padding(.trailing, 8).frame(height: EntraGuideMetrics.pasteHeight)
        .background(WhooshPalette.field, in: shape)
        .overlay(shape.strokeBorder(focused ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(WhooshPalette.sheetFieldBorder), lineWidth: 1))
        .overlay { shape.stroke(WhooshPalette.focusHalo, lineWidth: 3).padding(-1.5).opacity(focused ? 1 : 0) }
        .animation(.easeOut(duration: 0.08), value: focused)
        .animation(EntraGuideMetrics.motion(.easeOut(duration: EntraGuideMetrics.fadeS), reduce), value: model.pasted.isEmpty)
        .onChange(of: focused) { _, on in model.pasteFocus = on }
        .onChange(of: model.pasteFocus) { _, on in if on && !focused { focused = true } }
        .accessibilityElement(children: .contain).accessibilityIdentifier("\(EntraGuideWords.surface).paste")
    }

    private var tick: some View {
        let ok = model.validId
        return Image(systemName: "checkmark").font(.system(size: 10, weight: .bold)).foregroundStyle(WhooshPalette.onAccent)
            .frame(width: 18, height: 18).background(WhooshPalette.guideOK, in: Circle())
            .scaleEffect(ok ? 1 : 0.6).opacity(ok ? 1 : 0)
            .animation(EntraGuideMetrics.motion(EntraGuideMetrics.ease(EntraGuideMetrics.tickS), reduce), value: ok)
            .accessibilityIdentifier("\(EntraGuideWords.surface).paste.ok")
    }

    @ViewBuilder private var note: some View {
        if !model.pasted.isEmpty && !model.validId {
            Text(EntraGuideWords.idWrong).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.warning).transition(.opacity)
        } else {
            Text(EntraGuideWords.keychain).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).transition(.opacity)
        }
    }
}
