import SwiftUI

/// The addaccount lane: Add account by IMAP with provider autodetection (mac-add-account). The sheet is drawn over the main window by
/// `AddAccountSheetHost` (the root's overlay) and over Settings by `addAccountSheet(isPresented:)`; Welcome mounts `AddAccountView`
/// inline. This feature also claims the surface's own window (`shot mac-add-account`), registers its rows, clicks and keys.
public enum AddAccountFeature: FeatureModule {
    public static let id = "addaccount"
    public static let surfaces: [String] = [AddAccountWords.surface]
    static let A = AddAccountWords.surface

    public static func view(for slot: SurfaceSlot) -> AnyView? {
        slot.id == A ? AnyView(AddAccountSurfaceView()) : nil
    }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        var out: [String: (AppModel) throws -> [ControlRow]] = [
            "\(A).state": { _ in AddAccountRows.state() },
            "\(A).fields": { _ in AddAccountRows.fields() },
            "\(A).address": { _ in AddAccountRows.address() },
            "\(A).password": { _ in AddAccountRows.password() },
            "\(A).detection.step": { _ in AddAccountRows.steps() },
            "\(A).step": { _ in AddAccountRows.step() },
            "\(A).found": { _ in AddAccountRows.found() },
            "\(A).marks": { _ in AddAccountRows.marks() },
            "\(A).controls": { _ in AddAccountRows.controls() },
            "\(A).summary.mode": { _ in AddAccountRows.mode() },
            "\(A).manual": { _ in AddAccountRows.manual() },
            "\(A).validation.line": { _ in AddAccountRows.validation() },
            "\(A).accounts": { _ in AddAccountRows.accounts() },
            "\(A).tenant": { _ in AddAccountRows.tenant() },
            "\(A).signin.coming-soon": { _ in [] },   // retired 2026-11-09: nothing is "coming soon" now; registered empty so a check can read the absence
            "\(A).provider-note": { _ in AddAccountRows.providerNote() },
            "\(A).oauth": { _ in AddAccountRows.oauth() },
            "\(A).app-password.steps": { _ in AddAccountRows.appPasswordSteps() },
            "\(A).error": { _ in AddAccountRows.error() },
            "\(A).notice": { _ in AddAccountRows.notice() },
        ]
        for f in AddAccountDraft.fieldIds { out["\(A).summary.\(f)"] = { _ in AddAccountRows.field(f) } }
        return out
    }

    /// Every click opens the sheet first (the surface becomes mac-add-account), then does what the control does.
    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        func go(_ body: @escaping @MainActor () -> Bool) -> (AppModel) -> Bool { { m in m.surface = A; return body() } }
        let model = { AddAccountModel.shared }
        var out: [String: (AppModel) -> Bool] = [
            "\(A).open": go { true },
            "\(A).address": go { model().setFocus(.address); return true },
            "\(A).password": go { model().setFocus(.password); return true },
            "\(A).start-over": go { model().startOver(); return true },
            "\(A).release": go { model().release() },
            "\(A).oauth.field": go { model().oauthKind != nil && model().stage == .summary && !model().usesDefaultClient ? { model().setFocus(.appId); return true }() : false },
            "\(A).oauth.sign-in": go { model().signInTapped() },
            "\(A).oauth.change": go { model().changeAppId() },
            "\(A).app-id-guide": go { model().openGuide() },
            // SPEC A87: Details under a refusal, and the "How to get one" link beside the app-ID field
            "\(A).error.details": go { model().toggleDetails() },
            "\(A).oauth.own-id": go { model().openOwnId() },
            "\(A).oauth.guide": go { model().oauthKind != nil && model().stage == .summary ? model().openGuide() : false },
            // the failed added step's Details, and the scrim (a tap on it is swallowed: the sheet stays open)
            "\(A).step.details": go { model().toggleStepDetails() },
            "\(A).scrim": go { true },
            "\(A).app-password.link": go { model().provider.map { model().open(link: $0.link) } ?? false },
            "\(A).error.link": go { model().open(link: model().errorLink) },
            // SPEC A98: the admin-consent link line and its Copy (an empty link: nothing to open or copy, the click is refused)
            "\(A).error.consent-link": go { model().open(link: model().consentLink) },
            "\(A).error.consent-copy": go { model().copy(link: model().consentLink) },
            "\(A).fixture.hold-detection": go { model().holdDetection = true; return true },
            "\(A).fixture.hold-validation": go { model().holdValidation = true; return true },
            "\(A).fixture.inline": go { model().style = .inline; return true },
            "\(A).fixture.sheet": go { model().style = .sheet; return true },
            "\(A).fixture.hold-off": go { model().holdDetection = false; model().holdValidation = false; return true },
        ]
        for id in ["continue", "add", "retry", "cancel", "manual", "details", "add-another", "done", "keep-checking", "stop", "reveal"] {
            out["\(A).\(id)"] = go { model().click(id) }
        }
        for f in AddAccountDraft.fieldIds where f != "authMethod" {
            out["\(A).summary.\(f)"] = go { model().manualOpen && model().mode == .manual ? { model().setFocus(.init(f)); return true }() : false }
        }
        for f in ["imapSecurity", "smtpSecurity"] { for v in ["ssl", "starttls"] {
            out["\(A).summary.\(f).\(v)"] = go { model().manualOpen && model().mode == .manual ? { model().edit(f, v); return true }() : false }
        } }
        return out
    }

    /// Return, Esc, ⌘N and ⌘↩ while the sheet is up. A key the sheet has no use for returns false and passes on.
    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        let model = { AddAccountModel.shared }
        func up(_ m: AppModel) -> Bool { m.surface == A }
        return [
            "Enter": { m in up(m) && model().primaryAction() },
            "Escape": { m in up(m) && model().escapePressed() },
            "⌘N": { m in up(m) && model().addAnother() },
            "⌘Enter": { m in up(m) && model().doneTapped() },
        ]
    }
}

extension AddAccountModel {
    /// A click on a control: it must be on screen and enabled, as for a person; the add and retry buttons are one control.
    func click(_ id: String) -> Bool {
        let alias = id == "add" ? "retry" : (id == "retry" ? "add" : id)
        guard let c = AddAccountControls.visible(self).first(where: { $0.id == id || $0.id == alias }), c.enabled else { return false }
        return perform(c.id)
    }
}
