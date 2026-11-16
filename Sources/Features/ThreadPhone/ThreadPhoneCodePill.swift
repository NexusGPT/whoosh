import SwiftUI

/// The sign-in code pill: glass, because it floats over the message. One 44 pt target: a tap copies the code, marks the
/// message read and never archives it. The key turns into a check and the word into "Copied" on the tap's own frame
/// (the state is set before the action returns); the haptic goes with it. Reduced Motion drops the symbol animation.
struct ThreadCodePill: View {
    let code: String
    let copied: Bool
    let copy: () -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        Button(action: copy) {
            HStack(spacing: WhooshSpacing.s10) {
                Image(systemName: copied ? "checkmark.circle.fill" : "key.fill")
                    .font(.system(size: PhoneType.headline.size, weight: .semibold))
                    .foregroundStyle(copied ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(WhooshPalette.secondaryLabel))
                    .contentTransition(reduceMotion ? .identity : .symbolEffect(.replace))
                Text(code).whooshText(ThreadMetrics.code).foregroundStyle(WhooshPalette.label).lineLimit(1)
                Spacer(minLength: WhooshSpacing.s8)
                Text(copied ? ThreadPhoneWords.copied : ThreadPhoneWords.copy)
                    .phoneText(PhoneType.subhead, emphasized: true)
                    .foregroundStyle(copied ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(WhooshPalette.secondaryLabel))
            }
            .padding(.horizontal, PhoneMetrics.gutter)
            .frame(maxWidth: .infinity, minHeight: ThreadMetrics.pillHeight)
            .whooshGlass(in: Capsule())
            .contentShape(Capsule())
        }
        .buttonStyle(PressStyle())
        .sensoryFeedback(.success, trigger: copied) { _, now in now }
        .accessibilityLabel(ThreadPhoneWords.codeLine(code))
        .accessibilityValue(copied ? ThreadPhoneWords.copied : "")
        .accessibilityAddTraits(.isButton)
        .accessibilityIdentifier("iphone-thread.code-pill")
    }
}
