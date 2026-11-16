import Foundation

/// The root's base registrations: what the thin shell answers before any feature lane lands.
/// Every entry is overridable: a feature that registers the same key or ax-id wins (Registrations).
@MainActor
public enum BaseFeature: FeatureModule {
    public static let id = "contracts"
    public static let surfaces = ["mac-main"] + SurfaceSlot.settingsPages.map(\.id)

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "mac-main.list": { m in
                m.rows.enumerated().map { i, r in
                    ["ax": "mac-main.list.row.\(i)", "id": r.messageId, "sender": r.senderName, "subject": r.subject,
                     "account": r.accountId, "unread": r.unread, "pinned": r.pinned, "chips": r.labelNames]
                }
            },
            // the root's one account directory (AccountBook), the facts every account surface reads
            "mac-main.account-book": { _ in
                AccountBook.shared.accounts().map { a in
                    ["id": a.id, "name": a.name, "address": a.address, "colour": a.colour, "icon_id": a.iconId ?? NSNull(),
                     "unread": a.unread, "health": a.health ?? NSNull()]
                }
            },
            "mac-main.tabs": { m in
                m.counts.tabs.enumerated().map { i, t in
                    ["ax": "mac-main.tabs.\(i)", "id": t.tabId, "title": m.host.fixture?.tabName(t.tabId) ?? t.tabId,
                     "unread": Int(t.unread), "waiting": Int(t.waiting)]
                }
            },
        ]
    }

    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        [
            "J": { $0.moveFocus(1) },
            "K": { $0.moveFocus(-1) },
            "E": { $0.onFocused { .archive(messageIds: [$0]) } },
            "Z": { $0.perform(.undo) },
            "Tab": { $0.switchTab(1) },
            "Shift+Tab": { $0.switchTab(-1) },
            "Escape": { m in m.overlay = nil; m.surface = "mac-main"; return true },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        [
            "dock.whoosh": { m in m.surface = "mac-main"; WindowBridge.bringMainToFront(); return true },
            "mac-main.accounts.all": { m in m.accountFilter = nil; m.refresh(); return true },
        ]
    }
}
