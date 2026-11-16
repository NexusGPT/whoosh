import SwiftUI

/// The words under the plate: the sentence, its hint, the values, the ID field. When the step changes the words cross-fade under a 6 pt
/// blur in 120 + 120 ms (rule 8) while the camera pans; the block has one fixed height, so the plate never changes size from step to step.
struct EntraGuideTextView: View {
    @Bindable var model: EntraGuideModel
    @Environment(\.accessibilityReduceMotion) private var reduce
    @State private var shown = 0
    @State private var out = false

    var body: some View {
        let step = model.data.steps.indices.contains(shown) ? model.data.steps[shown] : model.step
        VStack(alignment: .leading, spacing: 0) {
            Text(step.say(domain: model.domain)).whooshText(WhooshTypography.title3, emphasized: false).fontWeight(.medium)
                .foregroundStyle(WhooshPalette.label).fixedSize(horizontal: false, vertical: true)
                .accessibilityIdentifier("\(EntraGuideWords.surface).say")
            if let hint = step.hintText {
                Text(hint).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).padding(.top, 4)
                    .fixedSize(horizontal: false, vertical: true).accessibilityIdentifier("\(EntraGuideWords.surface).hint")
            }
            if step.group != nil { EntraGuideValuesView(model: model).padding(.top, 12) }
            if step.slot == true { EntraGuidePasteView(model: model, slot: true).padding(.top, 12) }
            if step.paste == true { EntraGuidePasteView(model: model, slot: false).padding(.top, 12) }
            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, minHeight: EntraGuideMetrics.textHeight, maxHeight: EntraGuideMetrics.textHeight, alignment: .topLeading)
        .opacity(out ? 0 : 1).blur(radius: out && !reduce ? 6 : 0).offset(x: out && !reduce ? CGFloat(-8 * model.direction) : 0)
        .onAppear { shown = model.index }
        .onChange(of: model.index) { _, n in swap(to: n) }
        .accessibilityElement(children: .contain).accessibilityIdentifier("\(EntraGuideWords.surface).text")
    }

    /// Out in 120 ms, the new words in at the same spot, in 120 ms.
    private func swap(to n: Int) {
        withAnimation(EntraGuideMetrics.motion(.easeIn(duration: EntraGuideMetrics.stepHalfS), reduce)) { out = true }
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: UInt64(EntraGuideMetrics.stepHalfS * 1_000_000_000))
            shown = model.index
            withAnimation(EntraGuideMetrics.motion(EntraGuideMetrics.ease(EntraGuideMetrics.stepHalfS), reduce)) { out = false }
        }
    }
}
