import SwiftUI

/// Sync: the Mac this phone is paired with, the last time they were in sync, and, when they are not, why and Retry.
struct SettingsPhoneSyncPage: View {
    private let m = AppModel.shared
    private let state = SettingsPhoneState.shared

    var body: some View {
        SettingsPhoneFrame(title: SettingsPhoneWords.sync, titleAx: "iphone-settings.sync.title", onBack: { _ = state.back() }) {
            TimelineView(.periodic(from: .now, by: SettingsPhoneMetrics.tick)) { _ in
                let _ = state.retries
                let r = SettingsPhoneSync.read(m)
                SettingsPhoneCard { status(r) }
            }
            SettingsPhoneNote(text: SettingsPhoneWords.syncLine)
            SettingsPhoneNote(text: SettingsPhoneWords.syncOwn)
        }
    }

    private func status(_ r: SyncReading) -> some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s16) {
            HStack(alignment: .center, spacing: WhooshSpacing.s12) {
                PhonePaneIcon(symbol: symbol(r), hue: hue(r), size: SettingsPhoneMetrics.heroIcon, radius: SettingsPhoneMetrics.heroIconRadius)
                VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                    Text(SettingsPhoneSync.line(r)).phoneText(PhoneType.title3).foregroundStyle(WhooshPalette.label)
                        .fixedSize(horizontal: false, vertical: true).accessibilityIdentifier("iphone-settings.sync.line")
                    let detail = SettingsPhoneSync.detail(r)
                    if !detail.isEmpty {
                        Text(detail).phoneText(PhoneType.subhead)
                            .foregroundStyle(r.phase == .failing ? AnyShapeStyle(WhooshPalette.warning) : AnyShapeStyle(WhooshPalette.secondaryLabel))
                            .fixedSize(horizontal: false, vertical: true).accessibilityIdentifier("iphone-settings.sync.detail")
                    }
                }
                Spacer(minLength: 0)
            }
            .accessibilityElement(children: .combine)
            if r.retry {
                Button { state.retry(m) } label: {
                    Text(SettingsPhoneWords.retry).phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.onAccent)
                        .frame(width: SettingsPhoneMetrics.retryWidth, height: SettingsPhoneMetrics.retryHeight)
                        .background(Capsule().fill(WhooshPalette.accent))
                }
                .buttonStyle(PressStyle()).accessibilityIdentifier("iphone-settings.sync.retry")
                .transition(.opacity)
            }
        }
        .padding(PhoneMetrics.gutter).frame(maxWidth: .infinity, alignment: .leading)
    }

    private func symbol(_ r: SyncReading) -> String {
        switch r.phase { case .inSync: return "checkmark.icloud"; case .failing: return "exclamationmark.icloud"; default: return "icloud" }
    }
    private func hue(_ r: SyncReading) -> ThemeColor {
        switch r.phase { case .inSync: return SettingsHue.green; case .failing: return SettingsHue.orange; default: return SettingsHue.gray }
    }
}
