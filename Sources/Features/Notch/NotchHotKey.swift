#if os(macOS)
import AppKit
import Carbon.HIToolbox
import KeyboardShortcuts
import Observation

extension KeyboardShortcuts.Name {
    /// The notch's key from any app. Its start value (Option and the key that types M on the keyboard in use) is written once, by
    /// NotchHotKey.seedOnce: the keyboard's layout is read on the main thread, never while this name is first made.
    static let notch = Self("notch")
}

/// The notch's key, from any app (owner's plan of 14 Nov 2026: "⌥M to start. You change it in Settings. The same key closes the notch").
/// System-wide keys go through the KeyboardShortcuts package (Sindre Sorhus, MIT; GlobalKeys.swift names it); the key is kept on this Mac
/// by the package, in the app's own settings store. A hidden test copy never registers the key: two copies of the app would both answer it,
/// and the copy under test would act on the owner's own key press. There the key's act has a door (NotchIds.stageSummon).
@MainActor
enum NotchHotKey {
    private static var started = false

    /// The mark that the start key was written once: a key the person removed or changed is never written over.
    static let seededKey = "whoosh.notch.key.seeded"

    /// Option and the key that types M on this keyboard (an AZERTY keyboard has M where a US one has the semicolon).
    static var startShortcut: KeyboardShortcuts.Shortcut {
        KeyboardShortcuts.Shortcut(KeyboardShortcuts.Key(rawValue: keyCode(typing: "m") ?? kVK_ANSI_M), modifiers: [.option])
    }

    /// The first time only: the start key is written to the app's settings.
    static func seedOnce() {
        guard !UserDefaults.standard.bool(forKey: seededKey) else { return }
        UserDefaults.standard.set(true, forKey: seededKey)
        if KeyboardShortcuts.getShortcut(for: .notch) == nil { KeyboardShortcuts.setShortcut(startShortcut, for: .notch) }
    }

    /// Once, from the notch's wiring: the key's press runs the notch's own act (NotchModel.summon).
    static func start() {
        seedOnce()
        guard !started, !HiddenRig.isOn, NotchPanel.parkOffset == 0 else { return }
        started = true
        KeyboardShortcuts.onKeyDown(for: .notch) {
            MainActor.assumeIsolated {
                NotchPanel.shared.install()
                _ = NotchModel.shared.summon(.shared)
            }
        }
    }

    /// The key as its caps ("⌥M"); nil when there is none.
    static var words: String? { KeyboardShortcuts.getShortcut(for: .notch).map { $0.description } }

    /// A new key from the recorder: one of Command, Option or Control with one other key (the owner's rule of 7 Nov 2026: "dont be having
    /// complex shortcuts"). nil when the key is saved; else why it is not, in words.
    static func set(_ shortcut: KeyboardShortcuts.Shortcut) -> String? {
        let held = shortcut.modifiers.intersection([.command, .option, .control, .shift])
        guard held == [.command] || held == [.option] || held == [.control] else {
            return Words.text("Use Command, Option or Control with one other key.")
        }
        KeyboardShortcuts.setShortcut(shortcut, for: .notch)
        return nil
    }

    /// Back to the start key.
    static func reset() { KeyboardShortcuts.setShortcut(startShortcut, for: .notch) }

    /// The key code that types this letter on the keyboard layout in use; nil when the layout cannot be read.
    static func keyCode(typing letter: Character) -> Int? {
        guard let source = TISCopyCurrentKeyboardLayoutInputSource()?.takeRetainedValue(),
              let raw = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData) else { return nil }
        let data = Unmanaged<CFData>.fromOpaque(raw).takeUnretainedValue() as Data
        let wanted = String(letter).lowercased()
        return data.withUnsafeBytes { bytes -> Int? in
            guard let layout = bytes.baseAddress?.assumingMemoryBound(to: UCKeyboardLayout.self) else { return nil }
            for code in 0..<128 {
                var dead: UInt32 = 0
                var length = 0
                var chars = [UniChar](repeating: 0, count: 4)
                let status = UCKeyTranslate(layout, UInt16(code), UInt16(kUCKeyActionDisplay), 0, UInt32(LMGetKbdType()),
                                            OptionBits(kUCKeyTranslateNoDeadKeysMask), &dead, chars.count, &length, &chars)
                if status == noErr, length > 0, String(utf16CodeUnits: chars, count: length).lowercased() == wanted { return code }
            }
            return nil
        }
    }
}

/// The recorder of the notch's key in Settings > Notifications: "Change" listens for the next key press in the Settings window; Escape leaves
/// the key as it is. The words are the app's own (the package's recorder speaks the system's language, not the one picked in Whoosh).
@MainActor @Observable
final class NotchKeyRecorder {
    static let shared = NotchKeyRecorder()
    private(set) var recording = false
    private(set) var refusal: String?
    /// Moves when the key changes, so the row reads it again.
    private(set) var revision = 0

    var words: String? { _ = revision; return NotchHotKey.words }

    func toggle() {
        recording ? stop() : begin()
    }

    private func begin() {
        recording = true
        refusal = nil
        // the shell's key monitor asks this first: while the recorder listens, the next key press is the new key and runs no command
        ChordKit.capture = { [weak self] e in
            guard let self, self.recording else { return false }
            if e.keyCode == UInt16(kVK_Escape) { self.stop(); return true }
            guard let shortcut = KeyboardShortcuts.Shortcut(event: e) else { return true }
            self.take(shortcut)
            return true
        }
    }

    private func stop() {
        recording = false
        ChordKit.capture = nil
    }

    /// The recorder's own door: the pressed key is saved, or refused in words (the recorder keeps listening after a refusal).
    func take(_ shortcut: KeyboardShortcuts.Shortcut) {
        if let why = NotchHotKey.set(shortcut) { refusal = why; return }
        refusal = nil
        revision += 1
        stop()
    }

    /// The rig's two keys for the recorder's own door (a hidden window gets no key press): one the recorder takes, one it refuses.
    func takeForRig(_ word: String) -> Bool {
        switch word {
        case "control-n": take(.init(.n, modifiers: [.control])); return true
        case "shift-command-n": take(.init(.n, modifiers: [.shift, .command])); return true
        default: return false
        }
    }

    func reset() {
        NotchHotKey.reset()
        refusal = nil
        revision += 1
        stop()
    }
}
#endif
