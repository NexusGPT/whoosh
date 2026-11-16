#if os(macOS)
import SwiftUI

private let P = "mac-settings-general"

/// Settings > Notifications, "The notch's key": the key that opens the notch from any app (Option and M to start), the button that records a
/// new one and the button that puts the start key back. While it records, the next key press in this window is the new key (Escape leaves the
/// key as it is); a key with two modifiers, or with none, is refused in words under the title.
struct NotchKeyRow: View {
    @State private var recorder = NotchKeyRecorder.shared

    var body: some View {
        SettingsRow(ax: "\(P).notch.key", title: GeneralWords.notchKey, caption: recorder.refusal ?? GeneralWords.notchKeyLine, captionStrong: recorder.refusal != nil) {
            HStack(spacing: WhooshSpacing.s8) {
                if recorder.recording {
                    Text(GeneralWords.notchKeyPress).font(.system(size: 12.5, weight: .medium)).foregroundStyle(WhooshPalette.accent).lineLimit(1)
                } else if let words = recorder.words {
                    SettingsKey(text: words)
                }
                SettingsButton(title: recorder.recording ? GeneralWords.cancel : GeneralWords.change, ax: "\(P).notch.key.change") { recorder.toggle() }
                SettingsButton(title: GeneralWords.defaultKey, ax: "\(P).notch.key.default") { recorder.reset() }
            }
        }
    }
}
#endif
