import SwiftUI
#if os(macOS)
import AppKit
#endif

extension EntraGuideModel {
    /// Step 1's button: the browser opens on Microsoft's admin page, then the button reads Next. Under the fixture core the open is recorded, not performed.
    @discardableResult
    func openEntra() -> Bool {
        guard live, step.primary == true else { return false }
        openedLinks.append(EntraGuideWords.entraURL)
        #if os(macOS)
        if AppModel.shared.host.fixture == nil, let u = URL(string: EntraGuideWords.entraURL) { NSWorkspace.shared.open(u) }
        #endif
        opened = true
        return true
    }

    /// Copy ⌘1/⌘2/⌘3: the value goes on the pasteboard (recorded, not written, under the fixture core) and the button reads Copied for 1.6 s.
    @discardableResult
    func copy(_ n: Int) -> Bool {
        guard live, let row = values.first(where: { $0.number == n && n > 0 }) else { return false }
        clipboard = row.value
        #if os(macOS)
        if AppModel.shared.host.fixture == nil {
            NSPasteboard.general.clearContents(); NSPasteboard.general.setString(row.value, forType: .string)
        }
        #endif
        copied = n
        copyTask?.cancel()
        copyTask = Task { @MainActor in
            try? await Task.sleep(nanoseconds: UInt64(EntraGuideMetrics.copiedS * 1_000_000_000))
            if !Task.isCancelled { self.copied = 0 }
        }
        return true
    }

    /// Save and sign in: only on the last step, only when the text is an app ID. The guide closes (reverse morph), then the sheet takes the ID.
    @discardableResult
    func save() -> Bool {
        guard live, isLast, validId else { return false }
        let id = idText, handler = onSave
        return leave { self.lastSaved = id; handler?(id) }
    }

    /// The socket's `type` and a paste: they land in the ID field while it is on screen.
    @discardableResult
    func type(_ text: String) -> Bool {
        guard live, step.wantsId else { return false }
        pasted += text
        return true
    }

    @discardableResult
    func focusPaste() -> Bool {
        guard live, step.wantsId else { return false }
        pasteFocus = true
        return true
    }

    @discardableResult
    func clearPasted() -> Bool {
        guard live, !pasted.isEmpty else { return false }
        pasted = ""
        return true
    }
}
