import SwiftUI

/// The thread's private note (J8.4), opened from More › Note. A sheet over the thread: one editor, Save, Close. The note is
/// the core's (`notes(target:)`, `addNote`, `editNote`), so the Mac shows the same text.
struct ThreadNoteSheet: View {
    @Bindable var state: ThreadPhoneState
    private let m = AppModel.shared

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s12) {
            HStack {
                Button { state.noteOpen = false } label: {
                    Image(systemName: "xmark").font(.system(size: PhoneType.headline.size, weight: .semibold)).foregroundStyle(WhooshPalette.label)
                        .frame(width: PhoneMetrics.roundButton, height: PhoneMetrics.roundButton).contentShape(Circle())
                }
                .buttonStyle(PressStyle()).accessibilityLabel(ThreadPhoneWords.done).accessibilityIdentifier("iphone-thread.note.close")
                Spacer()
                Text(ThreadPhoneWords.note).phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.label).accessibilityAddTraits(.isHeader)
                Spacer()
                Button { _ = state.saveNote(m) } label: {
                    Text(ThreadPhoneWords.save).phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.selection)
                        .frame(minWidth: PhoneMetrics.roundButton, minHeight: PhoneMetrics.roundButton).contentShape(Rectangle())
                }
                .buttonStyle(PressStyle()).accessibilityIdentifier("iphone-thread.note.save")
            }
            TextEditor(text: $state.noteDraft)
                .phoneText(PhoneType.body).foregroundStyle(WhooshPalette.label)
                .scrollContentBackground(.hidden)
                .padding(WhooshSpacing.s10)
                .frame(minHeight: ThreadMetrics.noteHeight)
                .background(PhoneGround.trackFill, in: RoundedRectangle(cornerRadius: PhoneMetrics.tileRadius, style: .continuous))
                .overlay(alignment: .topLeading) {
                    if state.noteDraft.isEmpty {
                        Text(ThreadPhoneWords.notePlaceholder).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.tertiaryLabel)
                            .padding(.horizontal, WhooshSpacing.s14).padding(.vertical, WhooshSpacing.s16).allowsHitTesting(false)
                    }
                }
                .accessibilityIdentifier("iphone-thread.note.field")
            Text(ThreadPhoneWords.noteLine).phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.secondaryLabel)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, PhoneMetrics.gutter).padding(.top, WhooshSpacing.s8)
        .presentationDetents([.medium])
        .accessibilityIdentifier("iphone-thread.note")
    }
}
