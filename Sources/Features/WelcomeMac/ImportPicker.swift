#if os(macOS)
import AppKit
import UniformTypeIdentifiers

/// The two doors to macOS the import panel needs: the file picker for an Outlook archive or a mailbox file, and the Full Disk Access pane.
/// A test never calls either (the rig clicks `import.choose-*` doors that hand the model a path); neither opens anything by itself.
enum ImportPicker {
    @MainActor static func choose(_ model: ImportModel) {
        let panel = NSOpenPanel()
        panel.message = ImportWords.chooseMessage
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.allowedContentTypes = ["olm", "mbox", "mbx"].compactMap { UTType(filenameExtension: $0) }
        panel.begin { response in
            guard response == .OK, let path = panel.url?.path else { return }
            model.scan(picking: path)
        }
    }

    /// System Settings, Privacy & Security, Full Disk Access (the path the contract's words name).
    @MainActor static func openFullDiskAccess() {
        // A test copy never opens a system window (the owner's screen): it counts the press, as the Calendars door does (`opened_settings`); the door and the button share this.
        if CoreHost.shared.kind == .fixture || CorePlace.onRig { ImportModel.shared.fdaPresses += 1; return }
        guard let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles") else { return }
        NSWorkspace.shared.open(url)
    }
}
#endif
