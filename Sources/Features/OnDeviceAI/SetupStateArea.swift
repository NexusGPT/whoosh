import SwiftUI

/// The part of the step that changes with the model's state: invitation, progress with its controls, ready, or a failure in words.
struct SetupStateArea: View {
    let setup: OnDeviceSetup
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let s = setup.state
        VStack(alignment: .leading, spacing: WhooshSpacing.s12) {
            switch s {
            case .notInstalled:
                line(s.words, tone: WhooshPalette.secondaryLabel)
                AIButton(title: OnDeviceWords.install, primary: true, id: "mac-ondeviceai.install") { setup.install() }
            case .downloading(let r, let t), .paused(let r, let t):
                progress(r, t, paused: { if case .paused = s { return true } else { return false } }())
            case .verifying:
                SetupBar(fraction: 1)
                line(s.words, tone: WhooshPalette.secondaryLabel)
            case .ready:
                HStack(spacing: WhooshSpacing.s8) {
                    Image(systemName: "checkmark.circle.fill").foregroundStyle(WhooshPalette.accent)
                    Text(OnDeviceWords.readyTitle).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label)
                }
                line(s.words, tone: WhooshPalette.secondaryLabel)
            case .failed(let why), .corrupt(let why):
                HStack(alignment: .top, spacing: WhooshSpacing.s8) {
                    Circle().fill(WhooshPalette.warning).frame(width: WhooshMetrics.warningDot, height: WhooshMetrics.warningDot).padding(.top, WhooshSpacing.s6)
                    Text(why).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label).fixedSize(horizontal: false, vertical: true)
                        .accessibilityIdentifier("mac-ondeviceai.reason")
                }
                AIButton(title: OnDeviceWords.retry, primary: true, id: "mac-ondeviceai.retry") { setup.install() }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .id(kind(s)).transition(.setupSwap)
        .animation(WhooshMotion.animation(WhooshMotion.readerSwap, reduceMotion: reduceMotion), value: kind(s))
    }

    /// The state's kind without its numbers, so bytes arriving never replay the cross-fade.
    private func kind(_ s: ModelInstallState) -> String {
        switch s {
        case .notInstalled: return "none"
        case .downloading: return "down"
        case .paused: return "paused"
        case .verifying: return "verify"
        case .ready: return "ready"
        case .failed: return "failed"
        case .corrupt: return "corrupt"
        }
    }

    private func line(_ text: String, tone: ThemeColor) -> some View {
        Text(text).whooshText(WhooshTypography.subhead).foregroundStyle(tone).fixedSize(horizontal: false, vertical: true)
            .accessibilityIdentifier("mac-ondeviceai.line")
    }

    @ViewBuilder private func progress(_ r: Int64, _ t: Int64, paused: Bool) -> some View {
        SetupBar(fraction: t > 0 ? Double(r) / Double(t) : 0, warn: false)
        HStack(alignment: .firstTextBaseline) {
            Text(OnDeviceWords.of(r, t)).monospacedDigit().whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label)
                .accessibilityIdentifier("mac-ondeviceai.bytes")
            Spacer()
            Text(paused ? Words.text("Paused") : OnDeviceWords.percent(t > 0 ? Double(r) / Double(t) : 0))
                .monospacedDigit().whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
        }
        HStack(spacing: WhooshSpacing.s8) {
            if paused { AIButton(title: OnDeviceWords.resume, primary: true, id: "mac-ondeviceai.resume") { setup.install() } }
            else { AIButton(title: OnDeviceWords.pause, id: "mac-ondeviceai.pause") { setup.pause() } }
            AIButton(title: OnDeviceWords.cancel, id: "mac-ondeviceai.cancel") { setup.cancel() }
        }
    }
}
