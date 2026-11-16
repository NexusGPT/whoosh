#if os(macOS)
import AppKit

/// The menu items' target: each item calls the same act as the chip's keys and the socket's clicks.
@MainActor
final class AttachmentMenuTarget: NSObject {
    nonisolated(unsafe) static var key: UInt8 = 0
    let ref: AttachmentRef
    init(_ ref: AttachmentRef) { self.ref = ref }

    func item(_ id: String, _ title: String, _ key: String, _ mods: NSEvent.ModifierFlags, _ action: Selector, on: Bool = true) -> NSMenuItem {
        let i = NSMenuItem(title: title, action: action, keyEquivalent: key)
        i.keyEquivalentModifierMask = mods
        i.target = self
        i.identifier = NSUserInterfaceItemIdentifier(id)
        i.isEnabled = on
        return i
    }

    @objc func open() { _ = AttachmentActs.open(ref) }
    @objc func quickLook() { _ = QuickLookController.shared.show(ref) }
    @objc func saveToDownloads() { _ = AttachmentActs.saveToDownloads(ref) }
    @objc func saveAs() { _ = AttachmentActs.saveAs(ref) }
    @objc func copyFile() { _ = AttachmentActs.copy(ref) }
    @objc func showInFinder() { _ = AttachmentActs.showInFinder(ref) }
    @objc func remove() { _ = AttachmentActs.remove(ref) }
    @objc func openWithApp(_ sender: NSMenuItem) {
        guard let app = sender.representedObject as? URL else { return }
        let ref = self.ref
        Task { @MainActor in if await AttachmentStore.shared.file(ref) != nil { _ = AttachmentActs.open(ref, with: app) } }
    }
}
#endif
