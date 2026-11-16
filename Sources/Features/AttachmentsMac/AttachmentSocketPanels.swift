#if os(macOS)
import AppKit

/// The socket's view of Quick Look, the menu, the Save As sheet and the rig pasteboards, and the menu items' acts.
@MainActor
enum AttachmentSocketPanels {
    static var store: AttachmentStore { .shared }

    static func panel() -> ControlRow {
        let ql = QuickLookController.shared
        var r: ControlRow = ["open": ql.isOpen, "count": ql.isOpen ? ql.items.count : 0, "item": ql.current?.name ?? "",
                             // a hidden rig copy keeps the panel as the controller's intent ("system_panel": false): the
                             // system panel's wiring is judged on a visible copy only
                             "index": ql.currentIndex ?? -1, "system_panel": !HiddenRig.isOn,
                             "has_data_source": HiddenRig.isOn || ql.panel?.dataSource === ql,
                             "has_delegate": HiddenRig.isOn || ql.panel?.delegate === ql,
                             "on_screen": ql.panel?.isVisible == true]
        if let c = ql.current, let f = AttachmentChipFrames.screenFrame(c) { r["zoom_from"] = ["x": f.minX, "y": f.minY, "w": f.width, "h": f.height] }
        if let w = ql.panel, w.isVisible { r["frame"] = ["x": w.frame.minX, "y": w.frame.minY, "w": w.frame.width, "h": w.frame.height] }
        return r
    }

    /// The menu the selected chip shows on right-click, item by item (built fresh, as a right-click builds it).
    static func menu(_ ref: AttachmentRef?) -> [ControlRow] {
        guard let ref else { return [] }
        let shown = AttachmentMenu.shown != nil && AttachmentMenu.shownFor == ref
        return AttachmentMenu.make(ref).items.filter { !$0.isSeparatorItem }.map { i in
            let key = i.keyEquivalent.isEmpty ? "" : (i.keyEquivalentModifierMask.contains(.command) ? "⌘" : "") + (i.keyEquivalent == " " ? "Space" : i.keyEquivalent.uppercased())
            return ["id": i.identifier?.rawValue ?? "", "title": i.title, "key": key, "enabled": i.isEnabled, "shown": shown,
                    "submenu": i.submenu?.items.filter { !$0.isSeparatorItem }.map(\.title) ?? [], "has_submenu": i.submenu != nil]
        }
    }

    static func state(_ m: AppModel) -> ControlRow {
        let clip = (AttachmentPasteboard.board.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL])?.first
        return ["selected": store.liveSelection(m).map(AttachmentSocket.ax) ?? "", "holds_keys": AttachmentKeys.holdsKeys(m),
                "menu_shown": AttachmentMenu.shown != nil, "menu_opens": AttachmentMenu.opens,
                "hidden": HiddenRig.isOn, "menus_built_hidden": AttachmentMenu.builtHidden, "save_panel_open": SaveAsSheet.current?.isVisible == true || SaveAsSheet.intent != nil,
                "save_as_proposes": SaveAsSheet.intent?.dest.path ?? "",
                "pasteboard_file": clip?.lastPathComponent ?? "", "intake": store.intakeLog,
                "download_holding": FixtureAttachmentSource.shared.isHolding, "source": AttachmentSources.current == nil ? "none" : "fixture",
                "rig_open_launches_apps": AttachmentRig.launchesApps, "mutant": AttachmentRig.mutant, "shots": AttachmentShots.last]
    }

    /// The menu's items as socket clicks: each runs the same act as the menu item.
    static func menuActs(_ ref: AttachmentRef) -> [String: (AppModel) -> Bool] {
        var out = menuItemActs(ref)
        // doors-read: Open With ▸ has one door per app of its submenu, in the menu's order (0 is the default app): the app
        // is looked up when the door is clicked, never while the click tables are built
        for n in 0..<openWithDoors {
            out["open-with.\(n)"] = { _ in
                let apps = openWithApps(ref)
                guard apps.indices.contains(n), present(ref, "open-with"), store.url(ref) != nil else { return false }
                return AttachmentActs.open(ref, with: apps[n])
            }
        }
        return out
    }

    static let openWithDoors = 8

    /// The submenu's apps in the order the menu lists them: the default first, then the rest.
    static func openWithApps(_ ref: AttachmentRef) -> [URL] {
        let (first, all) = AttachmentMenu.apps(ref)
        return [first].compactMap { $0 } + all.filter { $0 != first }
    }

    private static func menuItemActs(_ ref: AttachmentRef) -> [String: (AppModel) -> Bool] {
        [
            "open": { _ in present(ref, "open") && AttachmentActs.open(ref) },
            "open-with": { _ in
                guard present(ref, "open-with"), let app = AttachmentMenu.apps(ref).0, store.url(ref) != nil else { return false }
                return AttachmentActs.open(ref, with: app)
            },
            "quick-look": { _ in present(ref, "quick-look") && QuickLookController.shared.show(ref) },
            "save-to-downloads": { _ in present(ref, "save-to-downloads") && AttachmentActs.saveToDownloads(ref) },
            "save-as": { _ in present(ref, "save-as") && AttachmentActs.saveAs(ref) },
            "copy": { _ in present(ref, "copy") && AttachmentActs.copy(ref) },
            "share": { _ in
                guard present(ref, "share"), let u = store.url(ref) else { return false }
                store.shared[ref] = [u.path]
                if AttachmentRig.launchesApps, let v = AttachmentChipFrames.view(ref) {   // never on a rig copy
                    NSSharingServicePicker(items: [u]).show(relativeTo: v.bounds, of: v, preferredEdge: .minY)
                }
                return true
            },
            "show-in-finder": { _ in present(ref, "show-in-finder") && AttachmentActs.showInFinder(ref) },
            "remove": { _ in present(ref, "remove") && AttachmentActs.remove(ref) },
        ]
    }

    /// An item the menu does not carry (or carries disabled) cannot be clicked.
    private static func present(_ ref: AttachmentRef, _ id: String) -> Bool {
        AttachmentMenu.make(ref).items.contains { $0.identifier?.rawValue == id && $0.isEnabled }
    }

    static func answerSaveAs(save: Bool) -> Bool {
        // The save panel runs out of process (it implements no ok:), so the socket ends the sheet on its window, as
        // the panel's own buttons do; the destination is the panel's folder and name field.
        if let i = SaveAsSheet.intent { SaveAsSheet.intent = nil; return save ? AttachmentActs.save(i.ref, i.url, to: i.dest) : true }
        guard let p = SaveAsSheet.current, p.isVisible, let parent = p.sheetParent else { return false }
        parent.endSheet(p, returnCode: save ? .OK : .cancel)
        return true
    }

    /// Rig only: puts a fixture file on the rig's pasteboard as the Finder's Copy does (its file URL and its name as text).
    static func stageFile() -> Bool {
        guard let dir = AttachmentPlaces.fixtures else { return false }
        let url = dir.appendingPathComponent("sample-2-pages.pdf")
        let pb = AttachmentPasteboard.board
        pb.clearContents()
        return pb.writeObjects([url as NSURL]) && pb.setString(url.lastPathComponent, forType: .string)
    }

    /// Rig only: puts the sample PNG (or a line of text) on the rig's pasteboard, as a person copying an image would.
    static func stage(image: Bool) -> Bool {
        let pb = AttachmentPasteboard.board
        pb.clearContents()
        if !image { return pb.setString(String(describing: AttachmentRef.self), forType: .string) }   // any text: rig data
        guard let dir = AttachmentPlaces.fixtures, let data = try? Data(contentsOf: dir.appendingPathComponent("sample.png")) else { return false }
        return pb.setData(data, forType: .png)
    }
}
#endif
