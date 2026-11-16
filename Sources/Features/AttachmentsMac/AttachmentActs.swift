#if os(macOS)
import AppKit

/// What each control of the proxy does: the chip's double-click, Return and ⌘O, every menu item and the socket's
/// clicks all call these, so a driven check exercises the real act. Acts that need the bytes wait for the download.
@MainActor
enum AttachmentActs {
    static var store: AttachmentStore { .shared }

    static func run(_ act: AttachmentWaitingAct, _ ref: AttachmentRef) {
        switch act {
        case .quickLook: _ = QuickLookController.shared.show(ref)
        case .open: _ = open(ref)
        }
    }

    /// Opens the file in its default app (NSWorkspace), from the local copy. Waits while it downloads.
    static func open(_ ref: AttachmentRef) -> Bool {
        if AttachmentRig.mutant("no-open") { return true }
        guard let url = store.peek(ref).url else { return store.whenReady(ref, .open) }
        let app = NSWorkspace.shared.urlForApplication(toOpen: url)
        return launch(ref, url, app: app) { NSWorkspace.shared.open(url) }
    }

    /// Opens the file with the app the owner picked in Open With.
    static func open(_ ref: AttachmentRef, with app: URL) -> Bool {
        guard let url = store.url(ref) else { return false }
        return launch(ref, url, app: app) {
            NSWorkspace.shared.open([url], withApplicationAt: app, configuration: NSWorkspace.OpenConfiguration())
        }
    }

    private static func launch(_ ref: AttachmentRef, _ url: URL, app: URL?, _ go: () -> Void) -> Bool {
        let name = app.map { FileManager.default.displayName(atPath: $0.path).replacingOccurrences(of: ".app", with: "") } ?? ""
        if AttachmentRig.launchesApps { go() }
        store.opened[ref] = .init(path: url.path, app: name, launched: AttachmentRig.launchesApps, count: (store.opened[ref]?.count ?? 0) + 1)
        store.notice[ref] = name.isEmpty ? nil : AttachmentWords.opened(name)
        return true
    }

    /// Copies the file into Downloads under a free name ("quote 2.pdf" when "quote.pdf" is taken).
    static func saveToDownloads(_ ref: AttachmentRef) -> Bool {
        guard let url = store.url(ref) else { return false }
        return save(ref, url, to: AttachmentPlaces.free(ref.name, in: AttachmentPlaces.downloads))
    }

    static func save(_ ref: AttachmentRef, _ url: URL, to dest: URL) -> Bool {
        do {
            try FileManager.default.createDirectory(at: dest.deletingLastPathComponent(), withIntermediateDirectories: true)
            if FileManager.default.fileExists(atPath: dest.path) { try FileManager.default.removeItem(at: dest) }
            try FileManager.default.copyItem(at: url, to: dest)
        } catch { return false }
        store.saved[ref] = dest
        store.notice[ref] = AttachmentWords.saved(dest.deletingLastPathComponent().lastPathComponent)
        return true
    }

    /// Save As…: the system save panel as a sheet on the window, named after the attachment.
    static func saveAs(_ ref: AttachmentRef) -> Bool {
        guard let url = store.url(ref) else { return false }
        if HiddenRig.isOn {                    // no system sheet on a hidden rig copy: the save it would propose, as state
            let dir = AttachmentPlaces.rigDir?.appendingPathComponent("save-as", isDirectory: true) ?? AttachmentPlaces.downloads
            SaveAsSheet.intent = (ref, url, dir.appendingPathComponent(ref.name))
            return true
        }
        guard let window = AttachmentChipFrames.view(ref)?.window ?? (NSApp as NSApplication?)?.keyWindow ?? (NSApp as NSApplication?)?.mainWindow else { return false }
        window.makeKey()                       // the out-of-process save sheet waits for its window to be key
        let panel = NSSavePanel()
        panel.nameFieldStringValue = ref.name
        panel.directoryURL = AttachmentPlaces.rigDir?.appendingPathComponent("save-as", isDirectory: true) ?? AttachmentPlaces.downloads
        if let d = panel.directoryURL { try? FileManager.default.createDirectory(at: d, withIntermediateDirectories: true) }
        SaveAsSheet.current = panel
        // Presented after the click returns: the out-of-process sheet can take seconds to connect, and nothing waits on it.
        Task { @MainActor in
            panel.beginSheetModal(for: window) { answer in
                MainActor.assumeIsolated {
                    SaveAsSheet.current = nil
                    let dest = panel.url ?? panel.directoryURL?.appendingPathComponent(panel.nameFieldStringValue)
                    if answer == .OK, let dest { _ = save(ref, url, to: dest) }
                }
            }
        }
        return true
    }

    /// Copy: the file itself on the pasteboard (a Finder paste makes a copy of it). Under the rig, a private pasteboard.
    static func copy(_ ref: AttachmentRef) -> Bool {
        guard let url = store.url(ref) else { return false }
        let pb = AttachmentPasteboard.board
        pb.clearContents()
        guard pb.writeObjects([url as NSURL]) else { return false }
        store.notice[ref] = AttachmentWords.copied
        return true
    }

    /// Show in Finder: the saved copy (a pending file: the file itself), selected in a Finder window.
    static func showInFinder(_ ref: AttachmentRef) -> Bool {
        guard let target = store.saved[ref] ?? (ref.isPending ? store.url(ref) : nil) else { return false }
        if AttachmentRig.launchesApps { NSWorkspace.shared.activateFileViewerSelecting([target]) }
        store.revealed[ref] = target.path
        return true
    }

    static func remove(_ ref: AttachmentRef) -> Bool {
        guard ref.isPending, let a = ComposerModel.shared.attachments.first(where: { $0.name == ref.name }) else { return false }
        ComposerModel.shared.removeAttachment(a)
        if store.selected == ref { store.selected = nil }
        return true
    }
}

/// The open Save As sheet, so the socket can answer it (save or cancel) without a person.
@MainActor
enum SaveAsSheet {
    static weak var current: NSSavePanel?
    /// Hidden rig: the save the sheet would propose (attachment, local file, destination), answered by the socket.
    static var intent: (ref: AttachmentRef, url: URL, dest: URL)?
}

/// The pasteboard Copy writes and paste intake reads: the system's, or under the rig a private one, so a test never
/// replaces what the owner copied.
@MainActor
enum AttachmentPasteboard {
    static var board: NSPasteboard { rigName.map { NSPasteboard(name: NSPasteboard.Name($0)) } ?? .general }
    /// The rig's private pasteboard's name; nil on the owner's copy (the system's).
    static var rigName: String? { AttachmentPlaces.isRig ? "io.nexus.whoosh.rig.attachments" : nil }
}
#endif
