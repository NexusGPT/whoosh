import SwiftUI

/// What each AI row says, once: the page and the probe rows both read these.
@MainActor
enum SettingsPhoneAI {
    static let needs: [AINeed] = [.writing, .decision]

    static func title(_ n: AINeed) -> String { n == .writing ? SettingsPhoneWords.writingModel : SettingsPhoneWords.decisionModel }

    /// Which model does the job: the connected provider for writing, Laya, Jev or the rules for deciding.
    static func name(_ n: AINeed, _ m: AppModel) -> String {
        switch n {
        case .writing: return AISetupStatus.shared.status(.writing).detail
        default:
            switch m.core.decisionModel()?.provider {
            case .jev?: return SettingsPhoneWords.jev
            case .rules?: return SettingsPhoneWords.rules
            default: return SettingsPhoneWords.laya
            }
        }
    }

    /// True while every model the phone shows is usable: the home row shows Ready, else the dot.
    static func allReady() -> Bool { needs.allSatisfy { AISetupStatus.shared.status($0).usable } }
}

/// The missing-setup mark (design W1, one amber dot and one line) as a view the phone can draw: the Mac's `MissingSetupMark` is
/// 12 pt with a 26 pt button; this reads the same `AISetupStatus` and says the same sentence at the phone's type size.
/// It draws nothing while the need is met. The fix is a key or a download on the Mac, so the mark carries no button here.
struct SettingsPhoneMissingMark: View {
    let need: AINeed
    @State private var status = AISetupStatus.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let x = status.status(need)
        Group {
            if x.state == .working {
                VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
                    Text(x.words).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    ZStack(alignment: .leading) {
                        Capsule().fill(WhooshPalette.fill)
                        Capsule().fill(WhooshPalette.accent).scaleEffect(x: max(0.02, min(1, x.progress ?? 0)), y: 1, anchor: .leading)
                    }
                    .frame(height: SettingsPhoneMetrics.bar).clipShape(Capsule()).accessibilityHidden(true)
                }
            } else if !x.usable {
                HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s8) {
                    Circle().fill(WhooshPalette.warning).frame(width: PhoneMetrics.unreadDot, height: PhoneMetrics.unreadDot).accessibilityHidden(true)
                    Text(x.words).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.warning).fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .animation(reduceMotion ? nil : .easeOut(duration: WhooshMotion.rowInsert), value: x.state)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("iphone-settings.ai.\(need.rawValue).mark")
    }
}

/// AI: which model writes and which sorts, their state, with the missing-setup mark where one is missing.
struct SettingsPhoneAIPage: View {
    private let m = AppModel.shared
    private let state = SettingsPhoneState.shared
    @State private var status = AISetupStatus.shared

    var body: some View {
        SettingsPhoneFrame(title: SettingsPhoneWords.ai, titleAx: "iphone-settings.ai.title", onBack: { _ = state.back() }) {
            SettingsPhoneCard {
                ForEach(SettingsPhoneAI.needs, id: \.rawValue) { need in
                    model(need)
                    if need != SettingsPhoneAI.needs.last { SettingsPhoneDivider() }
                }
            }
        }
        .task { status.refresh() }
    }

    private func model(_ need: AINeed) -> some View {
        let x = status.status(need)
        let name = SettingsPhoneAI.name(need, m)
        return VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
            HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s8) {
                Text(SettingsPhoneAI.title(need)).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.label)
                Spacer(minLength: WhooshSpacing.s8)
                if x.usable { Text(SettingsPhoneWords.ready).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel) }
            }
            if !name.isEmpty {
                Text(name).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                    .accessibilityIdentifier("iphone-settings.ai.\(need.rawValue).name")
            }
            SettingsPhoneMissingMark(need: need)
        }
        .padding(.horizontal, PhoneMetrics.gutter).padding(.vertical, WhooshSpacing.s12)
        .frame(maxWidth: .infinity, minHeight: SettingsPhoneMetrics.row, alignment: .leading)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("iphone-settings.ai.\(need.rawValue)")
    }
}
