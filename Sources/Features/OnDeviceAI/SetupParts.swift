import SwiftUI

/// The header: a quiet tile, the title, one line saying what this is.
struct SetupHeader: View {
    let title: String
    var body: some View {
        HStack(alignment: .top, spacing: WhooshSpacing.s14) {
            Image(systemName: "cpu")
                .font(.system(size: WhooshTypography.title3.size, weight: .semibold))
                .foregroundStyle(WhooshPalette.accent)
                .frame(width: WhooshMetrics.avatar + WhooshSpacing.s8, height: WhooshMetrics.avatar + WhooshSpacing.s8)
                .background(RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).fill(WhooshPalette.mine))
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: WhooshSpacing.s4) {
                Text(title).whooshText(WhooshTypography.largeTitle).foregroundStyle(WhooshPalette.label)
                    .contentTransition(.opacity).accessibilityIdentifier("mac-ondeviceai.title")
                Text(OnDeviceWords.lead).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.secondaryLabel)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// Model, size, licence, where it stays: four facts, label left, value right, hairlines between.
struct SetupFacts: View {
    let spec: MlxModelSpec
    var body: some View {
        VStack(spacing: 0) {
            row(OnDeviceWords.modelLabel, spec.name, last: false)
            row(OnDeviceWords.sizeLabel, ModelInstallState.size(spec.totalBytes), last: false)
            row(OnDeviceWords.licenceLabel, spec.licence, last: false)
            row(OnDeviceWords.whereLabel, OnDeviceWords.whereValue, last: true)
        }
        .background(RoundedRectangle(cornerRadius: WhooshRadius.hint, style: .continuous).fill(WhooshPalette.field))
        .overlay(RoundedRectangle(cornerRadius: WhooshRadius.hint, style: .continuous).stroke(WhooshPalette.separator, lineWidth: WhooshMetrics.separator))
        .accessibilityElement(children: .contain).accessibilityIdentifier("mac-ondeviceai.facts")
    }
    private func row(_ k: String, _ v: String, last: Bool) -> some View {
        VStack(spacing: 0) {
            HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s12) {
                Text(k).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                Spacer(minLength: WhooshSpacing.s12)
                Text(v).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label).multilineTextAlignment(.trailing)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.horizontal, WhooshSpacing.s14).padding(.vertical, WhooshSpacing.s10)
            if !last { Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator) }
        }
    }
}

/// The progress bar moves by transform only: a full-width capsule scaled from its leading edge.
struct SetupBar: View {
    let fraction: Double
    var warn = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    var body: some View {
        Capsule().fill(WhooshPalette.fill).frame(height: WhooshMetrics.progressBar)
            .overlay(alignment: .leading) {
                Capsule().fill(warn ? WhooshPalette.warning : WhooshPalette.accent)
                    .scaleEffect(x: max(0.0001, fraction), y: 1, anchor: .leading)
            }
            .clipShape(Capsule())
            .animation(WhooshMotion.animation(.easeOut(duration: WhooshMotion.readerCrossFade), reduceMotion: reduceMotion), value: fraction)
            .accessibilityElement().accessibilityLabel(Words.text("Download progress")).accessibilityValue(OnDeviceWords.percent(fraction))
            .accessibilityIdentifier("mac-ondeviceai.bar")
    }
}

/// Content swaps under a short blur cross-fade (docs/MOTION.md rule 8): opacity plus a blur radius of 0 to 6 pt.
struct SetupBlur: ViewModifier {
    let radius: CGFloat
    func body(content: Content) -> some View { content.blur(radius: radius) }
}
extension AnyTransition {
    static var setupSwap: AnyTransition { .opacity.combined(with: .modifier(active: SetupBlur(radius: 6), identity: SetupBlur(radius: 0))) }
}
