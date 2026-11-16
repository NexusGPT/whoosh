import Foundation

/// The iPhone labels and contacts screen's registrations: rows, clicks and the door's entry points register here, never in a hub.
/// ax ids follow `<surface>.<region>[.<element>...][.<index>]` (checks/labelscontactsphone-check.py).
/// The doors: Mailboxes' Labels row calls `LabelsContactsPhoneState.shared.openList(in:)`, a sender's name in a thread calls
/// `openContact(address:in:)`, a label chip calls `openLabel(_:in:)`; each sets `AppModel.overlay` to this surface and the host
/// draws `LabelsContactsPhoneRoot()` while it is set (docs/lanes/labelscontactsphone.md "## Doors").
public enum LabelsContactsPhoneFeature: FeatureModule {
    public static let id = "labelscontactsphone"
    public static let surfaces: [String] = [LabelsContactsPhoneState.surface]

    private typealias Probe = LabelsContactsPhoneProbe
    private static var state: LabelsContactsPhoneState { .shared }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        let ax = Probe.ax
        return [
            "\(ax).page": { m in Probe.page(m) },
            "\(ax).labels": { m in Probe.labels(m) },
            "\(ax).smart-folders": { m in Probe.smartFolders(m) },
            "\(ax).match-count": { m in Probe.matchCount(m) },
            "\(ax).people": { m in Probe.people(m) },
            "\(ax).label": { m in Probe.label(m) },
            "\(ax).label.cards": { m in Probe.cards(m) },
            "\(ax).mail": { m in Probe.mail(m) },
            "\(ax).contact": { m in Probe.contact(m) },
            "\(ax).contact.messages": { m in Probe.contactMessages(m) },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        let ax = Probe.ax, m = AppModel.shared
        var c: [String: (AppModel) -> Bool] = [
            "\(ax).open": { m in state.openList(in: m); return true },
            "\(ax).back": { m in state.back(m) },
            "\(ax).label.pin": { m in
                guard let id = state.currentLabelId, let l = m.core.labels().first(where: { $0.id == id }) else { return false }
                return state.setPinned(id, !l.pinned, m)
            },
            "\(ax).label.mail": { _ in
                guard let id = state.currentLabelId else { return false }
                state.push(.mail(id)); return true
            },
            "\(ax).contact.merge": { m in
                guard let a = state.currentContactAddress, let r = PhoneContactReader.read(address: a, m), PhoneContactReader.merge(r, m) else { return false }
                state.touch(); return true
            },
        ]
        for l in m.core.labels() {
            c["\(ax).list.label.\(l.id)"] = { _ in state.push(.label(l.id)); return true }
        }
        for (i, p) in PhoneContactReader.people(m).enumerated() {
            c["\(ax).list.person.\(i)"] = { _ in
                guard let a = p.addresses.first else { return false }
                state.push(.contact(a)); return true
            }
        }
        if case .mail(let id)? = state.path.last {
            for (i, r) in PhoneLabelReader.mail(id, m).enumerated() {
                c["\(ax).mail.row.\(i)"] = { m in state.openThread(r.messageId, in: m) }
            }
        }
        if let a = state.currentContactAddress, let card = PhoneContactReader.read(address: a, m) {
            for (i, r) in card.messages.enumerated() {
                c["\(ax).contact.messages.\(i)"] = { m in state.openThread(r.messageId, in: m) }
            }
        }
        return c
    }
}
