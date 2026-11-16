import SwiftUI

/// `.cp` the sign-in code chip: the code the message carries and a Copy word. A click puts the code on the
/// clipboard and marks the message read (ReaderActions.copyCode); the word then turns to "Copied" on that frame.
struct ReaderCodePill: View {
    let code: String
    let copied: Bool
    let copy: () -> Void

    var body: some View {
        Button(action: copy) {
            HStack(spacing: WhooshSpacing.s8) {
                Image(systemName: copied ? "checkmark" : "key.fill")
                    .font(.system(size: WhooshTypography.caption.size, weight: .semibold))
                    .foregroundStyle(copied ? WhooshPalette.accent : WhooshPalette.secondaryLabel)
                Text(code).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label)
                Text(copied ? ReaderWords.copied : ReaderWords.copy)
                    .whooshText(WhooshTypography.caption).foregroundStyle(copied ? WhooshPalette.accent : WhooshPalette.secondaryLabel)
            }
            .padding(.horizontal, WhooshSpacing.s12)
            .frame(height: WhooshMetrics.fileTile)
            .background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .whooshAx("mac-main.reader.codepill", radius: WhooshRadius.field)   // the id plus the frame and radius `measure` reads (component-grammar); before the top padding, so the frame is the chip
        .padding(.top, WhooshSpacing.s4)
        .accessibilityLabel(ReaderWords.codeLine(code))
    }
}
