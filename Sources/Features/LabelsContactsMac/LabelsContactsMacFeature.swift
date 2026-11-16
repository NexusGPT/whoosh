import SwiftUI

/// The labelscontactsmac lane's registration: the rows the three screens show (socket verb `rows`) and the controls
/// that drive them (socket verb `click`). Ids follow checks/labelscontactsmac-check.py. The screen fills the shell's
/// surface slot "mac-labels-contacts" (a window of its own, opened through WindowBridge).
@MainActor
public enum LabelsContactsMacFeature: FeatureModule, CommandProviding {
    public static let id = "labelscontactsmac"
    public static let surfaces: [String] = ["mac-labels-contacts"]
    private static let ax = "mac-labels-contacts"
    static let slot = SurfaceSlot("mac-labels-contacts")

    public static func view(for slot: SurfaceSlot) -> AnyView? {
        slot == Self.slot ? AnyView(LabelsContactsView()) : nil
    }

    /// `drag` label onto label ends here as the row's own drop (appshell 3b: the gesture needs an activated app).
    public static func drop(from: String, onto: String, _ m: AppModel) -> Bool {
        let p = "\(ax).labels.", id = String(from.dropFirst(p.count)), into = String(onto.dropFirst(p.count))
        guard from.hasPrefix(p), onto.hasPrefix(p) else { return false }
        model.reloadLabels(); return model.labels.item(id)?.parentId == into || model.moveLabel(id, into: into)
    }

    /// Routed by focus: outside this screen's rule editor these return false and the next provider answers.
    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        [
            "Escape": { m in
                let i = LabelInspectorModel.shared
                if i.isOpen { return i.whyOpen ? i.toggleWhy() : (i.offer != nil ? i.notNow() : i.close()) }
                return m.surface == slot.id && model.pane == .folder && model.cancel()
            },
            "⌘I": { _ in LabelInspector.toggle() },
            "R": { _ in let i = LabelInspectorModel.shared; return i.isOpen && !i.editing && i.offer != nil && i.rerun() },
            "?": { _ in let i = LabelInspectorModel.shared; return i.isOpen && !i.editing && i.toggleWhy() },
            "A": { _ in let i = LabelInspectorModel.shared; return i.isOpen && !i.editing && i.addOtherChain() },
            "Enter": { _ in
                let i = LabelInspectorModel.shared
                guard i.isOpen, !i.editing else { return false }   // a focused field takes Return itself (onSubmit)
                return i.isNew ? i.create() : (i.offer == nil ? i.apply() : false)
            },
        ]
    }

    /// The inspector, while it is open in pane 3, holds the keys first (Esc, R, ↩ are its own then).
    public static func hasFocus(_ m: AppModel) -> Bool {
        LabelInspectorModel.shared.isOpen || (m.surface != SurfaceSlot.macMain.id && surfaces.contains(m.surface)) || m.overlay.map(surfaces.contains) == true
    }

    /// Enter goes through the command table by focus: in the rule editor it saves; elsewhere the next provider answers.
    public static func commands() -> [String: (AppModel) -> Bool] {
        ["open": { m in model.takeTyped(m); return m.surface == slot.id && model.pane == .folder && model.save() },
         // V: the Move picker opens clean HERE (not when its view appears: letters typed before the view is in its window are its words)
         "move": { m in
            guard m.shownMessageId != nil else { return false }
            #if os(macOS)
            LabelMoveState.installOnce()
            #endif
            LabelMoveState.shared.reset()
            m.overlay = LabelDoors.moveOverlay
            return true
         }]
    }

    private static var model: LabelsContactsModel { LabelsContactsModel.shared }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        LabelInspectorSocket.rows().merging(baseRows()) { a, _ in a }.merging(LabelGeo.rows()) { a, _ in a }.merging(LabelDoorsSocket.rows()) { a, _ in a }
    }

    private static func baseRows() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "\(ax).sidebar": { _ in
                model.reloadFolders()
                return model.folders.map { f in
                    ["ax": "\(ax).sidebar.\(f.id)", "id": f.id, "title": f.name, "parent": model.parentName(f.parentId) ?? NSNull(), "count": model.savedCount(f)]
                }
            },
            "\(ax).rule.matches": { m in
                model.takeTyped(m)
                return model.pane == .folder ? [["ax": "\(ax).rule.matches", "matches": model.draftMatches, "rule": model.draftRule, "name": model.draftName,
                                                 "parent": model.draftParent ?? NSNull(), "new": model.isNewFolder, "folder_id": model.folderId ?? NSNull()]] : []
            },
            "\(ax).rule.refusal": { _ in model.folderRefusal.map { [["ax": "\(ax).rule.refusal", "words": $0]] } ?? [] },
            "\(ax).people": { _ in
                model.reloadPeople()
                return model.people.map { ["ax": "\(ax).people.\($0.id)", "id": $0.id, "name": $0.name, "addresses": $0.addresses] }
            },
            "\(ax).contact.header": { _ in
                guard model.pane == .contact else { return [] }
                model.refreshCard()
                guard let c = model.card else { return [] }
                return [["ax": "\(ax).contact.header", "name": c.name, "address": c.addresses.first ?? "", "addresses": c.addresses]]
            },
            "\(ax).contact.messages": { _ in
                guard model.pane == .contact else { return [] }
                model.refreshCard()
                guard let c = model.card else { return [] }
                return c.messages.enumerated().map { i, r in
                    ["ax": "\(ax).contact.messages.\(i)", "id": r.messageId, "account": r.accountId, "subject": r.subject, "at": r.receivedAtMs]
                }
            },
            "\(ax).labels": { _ in model.labelRows(ax) },
            "\(ax).labels.refusal": { _ in model.labelRefusal.map { [["ax": "\(ax).labels.refusal", "words": $0]] } ?? [] },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        model.reloadLabels(); model.reloadFolders(); model.reloadPeople()
        var t: [String: (AppModel) -> Bool] = [
            "\(ax).rule.save": { m in model.takeTyped(m); return model.save() },
            "\(ax).rule.cancel": { _ in model.cancel() },
            "\(ax).contact.merge": { _ in model.mergeSuggested() },
            "\(ax).parent.none": { _ in model.setParent(nil) },
            "\(ax).contact.focused": { m in m.focusedMessageId.map { model.openSender(ofMessage: $0) } ?? false },
            "\(ax).folder.new": { _ in model.startNewFolder() },
            "\(ax).show.labels": { _ in model.pane = .labels; return true },
        ]
        // A click on a message row of the open sender card opens that thread (`contact.messages.<i>` is the row's read id too).
        if model.pane == .contact {
            model.refreshCard()
            for (i, r) in (model.card?.messages ?? []).enumerated() { t["\(ax).contact.messages.\(i)"] = { _ in model.openThread(ofMessage: r.messageId, threadId: r.threadId) } }
        }
        // Label gestures: type the name, then click; select a label, then drop it on another or on the top.
        t["\(ax).labels.create"] = { m in let n = m.typed; m.typed = ""; return model.createLabel(n) }
        t["\(ax).labels.create.top"] = { m in let n = m.typed; m.typed = ""; return model.createLabel(n, atTop: true) }
        t["\(ax).labels.rename"] = { m in let n = m.typed; m.typed = ""; return model.renameSelected(n) }
        t["\(ax).labels.drop.top"] = { _ in model.selectedLabelId.map { model.moveLabel($0, into: nil) } ?? false }
        for l in model.labels.items {
            t["\(ax).labels.select.\(l.id)"] = { _ in model.selectLabel(l.id) }
            t["\(ax).labels.drop.\(l.id)"] = { _ in model.selectedLabelId.map { model.moveLabel($0, into: l.id) } ?? false }
        }
        for n in model.labels.children(of: nil) { t["\(ax).parent.\(LabelsContactsSource.slug(n.name))"] = { _ in model.setParent(n.name) } }
        for p in model.people { t["\(ax).person.\(p.id)"] = { _ in model.open(address: p.addresses.first ?? "") } }
        for f in model.folders { t["\(ax).edit.\(f.id)"] = { _ in model.edit(f.id) } }
        t["\(ax).open"] = { m in WindowBridge.open(slot, in: m) }
        return t.merging(LabelInspectorSocket.clicks()) { a, _ in a }.merging(LabelDoorsSocket.clicks()) { a, _ in a }
    }
}
