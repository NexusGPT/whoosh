#if os(macOS)
import AppKit
#endif

/// The alert's sound (Settings > Notifications, "Sound", off to start): one short system sound for each alert the notch shows. A hidden test copy
/// plays nothing, so a drive never makes a sound on the owner's Mac; the model counts the ask (`soundsAsked`).
@MainActor
enum NotchSound {
    static func play() {
        #if os(macOS)
        guard !HiddenRig.isOn else { return }
        NSSound(named: NSSound.Name("Tink"))?.play()
        #endif
    }
}
