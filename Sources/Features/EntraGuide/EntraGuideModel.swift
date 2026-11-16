import SwiftUI
import Observation

/// The guide's state: where the person is among the steps, what they copied, the ID they pasted. Every control (the buttons, the keys,
/// the socket's clicks) lands on one of these methods, which answer false when the control would do nothing.
@MainActor @Observable
final class EntraGuideModel {
    static let shared = EntraGuideModel()

    let data = EntraGuideData.shared
    var isOpen = false
    /// the reverse morph back into the add-account sheet is running
    var leaving = false
    var index = 0
    var direction = 1
    /// the browser was opened from step 1 (the button then reads Next)
    var opened = false
    /// the number of the value just copied (1, 2, 3), 0 when none: "Copied" shows for 1.6 s
    var copied = 0
    var clipboard = ""
    var pasted = ""
    /// the ID the last Save and sign in handed to the add-account sheet (empty before any)
    var lastSaved = ""
    var pasteFocus = false
    /// A98: the mailbox organisation's domain, read from the add-account sheet when the guide opens (empty with no valid address)
    var domain = ""
    var openedLinks: [String] = []
    @ObservationIgnored var onSave: (@MainActor (String) -> Void)?
    @ObservationIgnored var copyTask: Task<Void, Never>?

    var total: Int { data.steps.count }
    var step: EntraGuideStep { data.steps.indices.contains(index) ? data.steps[index] : .blank }
    var number: Int { index + 1 }
    var isLast: Bool { total > 0 && index == total - 1 }
    var validId: Bool { Self.looksLikeAppId(pasted) }
    var nextIsOpen: Bool { step.primary == true && !opened }
    var idText: String { pasted.trimmingCharacters(in: .whitespacesAndNewlines) }
    var live: Bool { isOpen && !leaving && total > 0 }

    struct ValueRow: Identifiable {
        let def: EntraGuideValue
        /// what is shown and copied: the row's own value, or the one built at open (A98)
        let value: String
        let number: Int
        let current: Bool
        var id: String { def.label }
    }
    /// The current step's value rows: the Copy keys count the copyable ones in reading order (⌘1, ⌘2, ⌘3).
    var values: [ValueRow] {
        var n = 0
        return data.values(of: step).compactMap { v in
            let value = v.dynamic == "consent-link" ? consentLink : v.value
            if value.isEmpty { return nil }   // a link with no shipped ID has nothing to offer
            if v.copy { n += 1 }
            return ValueRow(def: v, value: value, number: v.copy ? n : 0, current: v.steps.contains(step.n))
        }
    }

    /// A98: Microsoft's admin-consent page for the mailbox's organisation. The same helper and the same shipped ID the add-account sheet uses;
    /// with no address the page is the generic `organizations` one, so the admin still lands on a working link. Empty when no ID ships.
    var consentLink: String {
        guard let id = DefaultSignInClients.microsoftInUse, !id.isEmpty else { return "" }
        return EngineRefusal.adminConsentLink(domain: domain.isEmpty ? "organizations" : domain, clientId: id)
    }

    /// A98: the address's domain where the sheet reads it (`AddAccountModel.domain`), only once the address looks like an address.
    static func sheetDomain() -> String {
        let sheet = AddAccountModel.shared
        return sheet.addressLooksValid ? sheet.domain : ""
    }

    /// An app ID is a GUID: 8-4-4-4-12 hexadecimal characters.
    nonisolated static func looksLikeAppId(_ s: String) -> Bool {
        let parts = s.trimmingCharacters(in: .whitespacesAndNewlines).split(separator: "-", omittingEmptySubsequences: false)
        let want = [8, 4, 4, 4, 12]
        return parts.count == 5 && zip(parts, want).allSatisfy { $0.count == $1 && $0.allSatisfy { $0.isASCII && $0.isHexDigit } }
    }

    // MARK: opening and closing
    /// Opens at step 1. `onSave` receives the pasted ID after the reverse morph: the add-account sheet puts it in its field and signs in.
    func open(onSave: (@MainActor (String) -> Void)? = nil) {
        self.onSave = onSave
        index = 0; direction = 1; opened = false; copied = 0; pasteFocus = false; leaving = false; openedLinks = []
        domain = Self.sheetDomain()
        isOpen = true
    }

    @discardableResult
    func close() -> Bool { leave { } }

    /// The reverse morph, then the guide is gone and `then` runs (the sheet is back on screen by then).
    @discardableResult
    func leave(then: @escaping @MainActor () -> Void) -> Bool {
        guard live else { return false }
        leaving = true
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: UInt64(EntraGuideMetrics.enterS * 1_000_000_000))
            self.isOpen = false; self.leaving = false; self.pasteFocus = false
            if AppModel.shared.surface == EntraGuideWords.surface { AppModel.shared.surface = SurfaceSlot.macMain.id }
            then()
        }
        return true
    }

    // MARK: moving
    @discardableResult
    func next() -> Bool {
        guard live else { return false }
        if nextIsOpen { return openEntra() }
        if isLast { return save() }
        return go(index + 1)
    }

    @discardableResult
    func back() -> Bool { live && index > 0 ? go(index - 1) : false }

    /// Skip to paste: straight to the last step, the field focused.
    @discardableResult
    func skip() -> Bool {
        guard live, !isLast else { return false }
        _ = go(total - 1); pasteFocus = true
        return true
    }

    /// The rig's door: any step by its number (1 to the last).
    @discardableResult
    func jump(to n: Int) -> Bool { live && n >= 1 && n <= total ? go(n - 1) : false }

    @discardableResult
    private func go(_ i: Int) -> Bool {
        guard i >= 0, i < total else { return false }
        direction = i >= index ? 1 : -1; index = i
        return true
    }
}
