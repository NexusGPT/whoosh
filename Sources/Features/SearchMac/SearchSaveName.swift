import SwiftUI

/// The name field that replaces the results header while a search is being saved as a smart folder; a refusal is said beside it in words.
struct SaveNameField: View {
    let model: SearchMacModel
    @Bindable var app: AppModel
    @FocusState private var focused: Bool
    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Image(systemName: "folder.badge.plus").foregroundStyle(WhooshPalette.accent)
            TextField(Words.text("Name the smart folder"), text: $app.typed)
                .textFieldStyle(.plain).whooshText(WhooshTypography.body)
                .focused($focused)
                .onSubmit { model.claim(); _ = model.enter() }   // Return in the name field is the name field's, never the list's Enter, whatever the surface says
                .accessibilityIdentifier("mac-search.save-as-smart-folder.field")
            if !model.fieldSays.isEmpty {
                Text(model.fieldSays).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.warning).lineLimit(1).fixedSize()
            }
        }
        .padding(.horizontal, WhooshSpacing.s10)
        .frame(height: SearchField.height)
        .background(RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).fill(WhooshPalette.field))
        .overlay(RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).strokeBorder(WhooshPalette.focusHalo))
        .onAppear { focused = true }
    }
}
