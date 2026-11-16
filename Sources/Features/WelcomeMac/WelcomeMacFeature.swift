import Foundation
import SwiftUI

/// The welcomemac lane: the Welcome and Accounts screen of the Mac (mac-welcome-accounts). Rows and clicks register here;
/// the screen mounts in the shell's window slot (view(for:)). The signin lane no longer registers the four ids it shadowed.
public enum WelcomeMacFeature: FeatureModule {
    public static let id = "welcomemac"
    public static let surfaces: [String] = ["mac-welcome-accounts"]
    static let W = "mac-welcome-accounts"

    @MainActor static func openInbox(_ m: AppModel) -> Bool {
        guard WelcomeModel.shared.canOpenInbox else { return false }
        m.surface = "mac-main"; return true
    }

    /// The one door to the Welcome window at its QR step: `mac-settings-accounts.move-to-iphone` (control and click target) calls it.
    /// The QR state is set before the window opens so the first drawn frame is the QR step; this feature registers no click id of Settings.
    @MainActor public static func openAtQR(_ m: AppModel) -> Bool {
        WelcomeModel.shared.qrOpen = true
        WelcomeQR.shared.refreshIfExpired()
        return WindowBridge.open(SurfaceSlot(W), in: m)
    }

    /// The one door to the Welcome window at its import step: Settings > Accounts "Import mail…" calls it. The panel is reset and opened
    /// before the window, so the first drawn frame is the import step.
    @MainActor public static func openAtImport(_ m: AppModel) -> Bool {
        WelcomeModel.shared.qrOpen = false   // a QR step left open earlier must not draw over the import step
        ImportModel.shared.openFromSettings()
        return WindowBridge.open(SurfaceSlot(W), in: m)
    }

    /// The shell's window slot: `shot mac-welcome-accounts` opens a window that draws this view.
    public static func view(for slot: SurfaceSlot) -> AnyView? {
        slot.id == W ? AnyView(WelcomeView()) : nil
    }

    /// `type` reaches a row's editor field or its password field once a field door put the focus there.
    @MainActor public static func typeText(_ text: String, _ m: AppModel) -> Bool { WelcomeModel.shared.typeIntoField(text) }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        WelcomePasteDoor.listen()
        var out: [String: (AppModel) throws -> [ControlRow]] = [
            "\(W).accounts": { _ in WelcomeRows.accounts() },
            "\(W).provider": { _ in WelcomeRows.provider() },
            "\(W).steps": { _ in WelcomeRows.steps() },
            "\(W).server": { _ in WelcomeRows.server() },
            "\(W).route": { _ in WelcomeRows.route() },
            "\(W).qr": { _ in WelcomeRows.qr() },
            "\(W).summary": { _ in WelcomeRows.summary() },
        ]
        out.merge(ImportRows.rowSources()) { a, _ in a }
        out.merge(WelcomeDoors.rowSources()) { a, _ in a }
        out.merge(RepliesStepRows.rowSources()) { a, _ in a }
        return out
    }

    /// The end of a drag of one Welcome row onto another (the socket's `drag`): the dragged row takes the place of the other.
    public static func drop(from: String, onto: String, _ m: AppModel) -> Bool {
        let p = "\(W).accounts."
        guard from.hasPrefix(p), onto.hasPrefix(p) else { return false }
        return WelcomeModel.shared.drop(String(from.dropFirst(p.count)), onto: String(onto.dropFirst(p.count)))
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        WelcomePasteDoor.listen()
        let t: [String: (AppModel) -> Bool] = [
            "\(W).continue": { m in m.surface = W; return WelcomeModel.shared.addTyped() },
            "\(W).add-another": { _ in WelcomeModel.shared.address = ""; return true },
            "\(W).open-inbox": { m in openInbox(m) },
            "\(W).paste-list": { _ in
                let w = WelcomeModel.shared
                guard !w.address.isEmpty else { w.pasteOpen.toggle(); return true }
                w.pasteText = w.address; w.address = ""; return w.addPasted() > 0
            },
            "\(W).paste-add": { _ in WelcomeModel.shared.addPasted() > 0 },
            "\(W).move-to-iphone": { _ in WelcomeModel.shared.qrOpen = true; WelcomeQR.shared.refreshIfExpired(); return true },
            "\(W).connect-waiting": { _ in WelcomeModel.shared.connectWaiting() > 0 },
            "\(W).reset": { _ in WelcomeModel.shared.reset(); return true },
        ]
        return t.merging(ImportRows.clickTargets()) { a, _ in a }.merging(RepliesStepRows.clickTargets()) { a, _ in a }
            .merging(WelcomeDoors.clickTargets()) { a, _ in a }
    }
}

@MainActor enum WelcomeRows {
    static func accounts() -> [ControlRow] {
        WelcomeModel.shared.rows.enumerated().map { i, r in
            ["ax": "\(WelcomeMacFeature.W).accounts.\(r.id)", "id": r.id, "order": i, "address": r.address, "name": AccountNames.display(address: r.address, name: r.name), "provider": r.routeName,
             "colour": r.colour, "icon": r.iconId ?? "", "signature": r.signature, "progress": r.progressWords, "phase": r.phase.rawValue,
             "status": r.status, "status_link": r.statusLink, "editing": WelcomeModel.shared.editing == r.id,
             "field": WelcomeModel.shared.fieldRow == r.id ? WelcomeModel.shared.fieldKind : "", "secret_typed": (WelcomeModel.shared.secrets[r.id] ?? "").count]
        }
    }
    static func provider() -> [ControlRow] {
        guard let r = WelcomeModel.shared.route else { return [] }
        return [["ax": "\(WelcomeMacFeature.W).provider.\(r.kind.rawValue)", "name": r.name, "route": r.kind.rawValue, "how": r.how,
                 "sheet": r.usesSheet, "mac_only": r.macOnly]]
    }
    static func steps() -> [ControlRow] {
        guard let r = WelcomeModel.shared.route else { return [] }
        return r.steps.enumerated().map { i, s in
            ["ax": "\(WelcomeMacFeature.W).steps.\(i + 1)", "step": i + 1, "text": s, "link": i == 0 ? (r.link ?? "") : "", "link_words": r.linkWords ?? ""]
        }
    }
    /// The typed address's route in one row, under an id no other feature registers (SignIn answers `.provider` and `.steps` first).
    static func route() -> [ControlRow] {
        guard let r = WelcomeModel.shared.route else { return [] }
        return [["ax": "\(WelcomeMacFeature.W).route", "name": r.name, "route": r.kind.rawValue, "how": r.how, "link": r.link ?? "",
                 "link_words": r.linkWords ?? "", "server": r.server, "sheet": r.usesSheet, "mac_only": r.macOnly, "steps": r.steps.count]]
    }
    static func server() -> [ControlRow] {
        guard let r = WelcomeModel.shared.route else { return [] }
        return [["ax": "\(WelcomeMacFeature.W).server", "host": r.server]]
    }
    static func qr() -> [ControlRow] {
        let q = WelcomeQR.shared
        guard WelcomeModel.shared.qrOpen else { return [] }
        q.refreshIfExpired()
        return [["ax": "\(WelcomeMacFeature.W).qr", "payload": q.token.qrPayload, "expires_in_s": q.secondsLeft(), "issued": q.issued]]
    }
    static func summary() -> [ControlRow] {
        let w = WelcomeModel.shared
        return [["ax": "\(WelcomeMacFeature.W).summary", "total": w.rows.count, "ready": w.healthyCount, "refused": w.refusedCount, "preset": w.preset]]
    }
}
