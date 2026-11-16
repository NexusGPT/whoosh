import Foundation
#if os(macOS)
import AppKit
#endif

/// fix-screens: the doors and rows the control sweep of the Welcome window found missing (docs/lanes/sweep-welcomemac.md F-a to F-c, F-d, F-i, F-j).
/// Each door runs the method the control itself runs, so a click through the socket and a click by hand take one path; none reaches a network
/// or a real sign-in. Registered next to the Welcome rows by WelcomeMacFeature.
@MainActor enum WelcomeDoors {
    private static let W = WelcomeMacFeature.W
    static let presets = ["Superhuman", "GotEmail", "Mail"]

    /// F-a: the paste area's state, to see the toggle of `.paste-list` and the text `.paste-add` would use.
    static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        ["\(W).paste": { _ in
            let w = WelcomeModel.shared
            return [["ax": "\(W).paste", "open": w.pasteOpen, "text": w.pasteText]]
        }]
    }

    /// What a Link does: the system opens the address. A rig copy refuses a web address off this Mac and counts it; the door and the Link share the call.
    static func openLink(_ u: URL) -> Bool {
        #if os(macOS)
        _ = NSWorkspace.shared.open(u)
        return true
        #else
        return false
        #endif
    }

    static func clickTargets() -> [String: (AppModel) -> Bool] {
        let w = WelcomeModel.shared
        var t: [String: (AppModel) -> Bool] = [
            // F-b: the typed words go into the paste area (and it opens) without making rows; `.paste-add` then makes them.
            "\(W).paste-text": { _ in
                guard !w.address.isEmpty else { return false }
                w.pasteText = w.address; w.address = ""; w.pasteOpen = true; return true
            },
        ]
        // F-c: the three key presets of the footer's picker.
        for p in presets { t["\(W).preset.\(p)"] = { _ in w.setPreset(p); return w.preset == p } }
        // F-d and F-i: a row's own tap (opens or closes its editor) and its place in the list (one step up: the keyboard's way of the drag).
        for (i, r) in w.rows.enumerated() {
            let id = r.id
            t["\(W).accounts.\(id)"] = { _ in w.editing = (w.editing == id) ? nil : id; return true }
            if i > 0 { t["\(W).accounts.\(id).move-up"] = { _ in w.move(from: IndexSet(integer: i), to: i - 1); return true } }
            // F-e, F-f: the row's own buttons (the view's Sign in with, Connect and Try again call these methods)
            t["\(W).sign-in.\(id)"] = { _ in w.signInWithSheet(id) }
            t["\(W).connect.\(id)"] = { _ in w.connectTyped(id) }
            t["\(W).accounts.\(id).try-again"] = { _ in w.tryAgain(id) }
            // F-g: the editor's fields take `type` after a focus door; the colours, the icons and Remove answer only while the editor is open
            for kind in ["name", "signature", "secret"] { t["\(W).accounts.\(id).field.\(kind)"] = { _ in w.focusField(id, kind) } }
            for n in WelcomeColour.swatches.indices { t["\(W).accounts.\(id).colour.\(n)"] = { _ in w.pickColour(id, index: n) } }
            for icon in AccountMarkIcon.libraryIds.keys { t["\(W).accounts.\(id).icon.\(icon)"] = { _ in w.pickIcon(id, icon: icon) } }
            t["\(W).accounts.\(id).remove"] = { _ in w.removeFromEditor(id) }
            // F-h: the refused row's link: the door runs the same open the Link runs; a rig copy refuses a web address and counts it (`uistate.hidden.system_ui.open`)
            t["\(W).accounts.\(id).link"] = { _ in
                guard let r = w.rows.first(where: { $0.id == id }), r.phase == .refused, let u = URL(string: r.statusLink), !r.statusLink.isEmpty else { return false }
                return WelcomeDoors.openLink(u)
            }
        }
        // F-h: the provider page's link (the first step of the route the typed address names)
        t["\(W).provider.link"] = { _ in
            guard let l = w.route?.link, let u = URL(string: l) else { return false }
            return WelcomeDoors.openLink(u)
        }
        // F-j: the import panel's "Into" picker: the matching account, or one of the core's accounts.
        let imp = ImportModel.shared
        t["\(W).import.into.matching"] = { _ in imp.intoAccount = nil; return true }
        for a in imp.accounts { t["\(W).import.into.\(a.id)"] = { _ in imp.intoAccount = a.id; return true } }
        return t
    }
}
