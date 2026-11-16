#if os(macOS)
import AppKit
import UniformTypeIdentifiers

/// The right-click menu of a chip, as the Finder's for a file: Open · Open With ▸ (the system's apps for the type,
/// the default first) · Quick Look · Save to Downloads · Save As… · Copy · Share ▸ · Show in Finder (once saved), and
/// Remove for a pending file. Each item shows its key where one exists (⌘O, Space, ⌘C).
@MainActor
enum AttachmentMenu {
    static weak var shown: NSMenu?
    static var shownFor: AttachmentRef?
    /// Times a menu really opened on screen (its delegate's menuWillOpen), for the socket; the shot and tick
    /// bookkeeping of a socket-opened menu (AttachmentMenuPopUp.swift).
    static var opens = 0
    static var shotOpen = 0
    static var ticks = 0
    /// Menus a hidden rig copy built for the socket instead of showing them.
    static var builtHidden = 0

    static func make(_ ref: AttachmentRef) -> NSMenu {
        let t = AttachmentMenuTarget(ref)
        let local = AttachmentStore.shared.url(ref)
        let m = NSMenu(title: ref.name)
        m.autoenablesItems = false
        m.addItem(t.item("open", AttachmentWords.open, "o", [.command], #selector(AttachmentMenuTarget.open)))
        m.addItem(openWith(ref, t))
        m.addItem(t.item("quick-look", AttachmentWords.quickLook, " ", [], #selector(AttachmentMenuTarget.quickLook)))
        m.addItem(.separator())
        m.addItem(t.item("save-to-downloads", AttachmentWords.saveToDownloads, "", [], #selector(AttachmentMenuTarget.saveToDownloads), on: local != nil))
        m.addItem(t.item("save-as", AttachmentWords.saveAs, "", [], #selector(AttachmentMenuTarget.saveAs), on: local != nil))
        m.addItem(t.item("copy", AttachmentWords.copy, "c", [.command], #selector(AttachmentMenuTarget.copyFile), on: local != nil))
        m.addItem(share(ref, local))
        let revealable = AttachmentStore.shared.saved[ref] != nil || ref.isPending
        m.addItem(t.item("show-in-finder", AttachmentWords.showInFinder, "", [], #selector(AttachmentMenuTarget.showInFinder), on: revealable))
        if ref.isPending {
            m.addItem(.separator())
            m.addItem(t.item("remove", AttachmentWords.remove, "", [], #selector(AttachmentMenuTarget.remove)))
        }
        for i in m.items where AttachmentRig.mutant("menu-without-\(i.identifier?.rawValue ?? "")") { m.removeItem(i) }
        objc_setAssociatedObject(m, &AttachmentMenuTarget.key, t, .OBJC_ASSOCIATION_RETAIN)
        return m
    }

    /// Open With ▸: the default app first (marked), then every other app that opens the type.
    static func openWith(_ ref: AttachmentRef, _ t: AttachmentMenuTarget) -> NSMenuItem {
        let item = NSMenuItem(title: AttachmentWords.openWith, action: nil, keyEquivalent: "")
        item.identifier = NSUserInterfaceItemIdentifier("open-with")
        let sub = NSMenu(title: AttachmentWords.openWith)
        let (first, all) = apps(ref)
        for app in ([first].compactMap { $0 } + all.filter { $0 != first }) {
            var name = FileManager.default.displayName(atPath: app.path).replacingOccurrences(of: ".app", with: "")
            if app == first { name += " (\(AttachmentWords.defaultApp))" }
            let a = t.item("app", name, "", [], #selector(AttachmentMenuTarget.openWithApp(_:)))
            a.representedObject = app
            let icon = NSWorkspace.shared.icon(forFile: app.path); icon.size = NSSize(width: WhooshMetrics.actionIcon, height: WhooshMetrics.actionIcon)
            a.image = icon
            sub.addItem(a)
            if app == first && all.count > 1 { sub.addItem(.separator()) }
        }
        item.submenu = sub
        item.isEnabled = !sub.items.isEmpty
        return item
    }

    /// The default app and every app for the file (its local copy) or, before the download, for its type.
    static func apps(_ ref: AttachmentRef) -> (URL?, [URL]) {
        let ws = NSWorkspace.shared
        if let url = AttachmentStore.shared.peek(ref).url { return (ws.urlForApplication(toOpen: url), ws.urlsForApplications(toOpen: url)) }
        let type = UTType(filenameExtension: (ref.name as NSString).pathExtension.lowercased()) ?? .data
        return (ws.urlForApplication(toOpen: type), ws.urlsForApplications(toOpen: type))
    }

    /// Share ▸: the system's share menu for the file once it is here.
    static func share(_ ref: AttachmentRef, _ local: URL?) -> NSMenuItem {
        let item: NSMenuItem
        if let u = local {
            item = NSSharingServicePicker(items: [u]).standardShareMenuItem
            item.image = nil                   // its icon would push every item below it a column right
        } else {
            item = NSMenuItem(title: AttachmentWords.share, action: nil, keyEquivalent: ""); item.isEnabled = false
        }
        item.identifier = NSUserInterfaceItemIdentifier("share")
        return item
    }

    static func close() -> Bool {
        guard let m = shown else { return false }
        m.cancelTracking()
        return true
    }
}
#endif
