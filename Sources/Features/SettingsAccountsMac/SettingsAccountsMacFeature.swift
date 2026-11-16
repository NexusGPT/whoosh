import SwiftUI

/// Registration for the settingsaccountsmac lane: Settings > Accounts on the Mac (health, notifications, the icon picker).
/// The pane mounts in the shell's Settings scene (`.macSettingsAccounts`); rows, clicks and palette commands register here,
/// never in a hub.
public enum SettingsAccountsMacFeature: FeatureModule {
    public static let id = "settingsaccountsmac"
    public static let surfaces: [String] = ["mac-settings-accounts"]
    static let S = "mac-settings-accounts"

#if os(macOS)
    public static func view(for slot: SurfaceSlot) -> AnyView? {
        slot == .macSettingsAccounts ? AnyView(AccountsPaneView()) : nil
    }
#endif

    @MainActor static func row(_ a: AccountEntry, _ s: AccountsSession) -> ControlRow {
        let st = s.state(a.id)
        return ["ax": "\(S).\(a.id)", "id": a.id, "name": a.name, "address": a.address, "provider": a.provider, "colour": a.colour,
                "icon": a.imagePath != nil ? "uploaded" : EmblemLibrary.resolve(a.iconId).id,
                "sync": s.syncLine(a.id), "error": st?.lastError?.reasonWords ?? "", "healthy": st?.lastError == nil,
                "notifications": s.isMuted(a.id) ? "off" : "on", "delivery": "\(s.setting(a.id).delivery.map { String(describing: $0) } ?? "both")", "note": s.note(a.id),
                "group": a.group ?? "", "signature": a.signature, "paused": a.paused, "backfill": a.backfill.map { $0 as Any } ?? NSNull()]
    }

    /// `type <text>` while a click door has focused a field of this pane (an account page's name or signature, the find field): that
    /// field takes it in the same turn. No focused field, or the keys elsewhere: false, the text goes on.
    public static func typeText(_ text: String, _ m: AppModel) -> Bool {
        guard !text.isEmpty, hasFocus(m) else { return false }
        return AccountsSession.shared.typeIntoField(text)
    }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        let s = AccountsSession.shared
        var out: [String: (AppModel) throws -> [ControlRow]] = [
            "\(S).accounts": { _ in
                // an account added while this list was open (the Welcome rows, `fixture add-account`, a sign-in) reaches the list now, not at the next open: the session read the core
                // on open only, so main111 read 30 rows after the 31st was added (thirty-first-added-last)
                let held = s.accounts.count
                let core = CoreHost.shared.core.accounts().count
                if held != core { s.reload() }
                return s.visible.map { row($0, s) }
            },
            "\(S).failing": { _ in s.failing.map { a in
                ["ax": "\(S).\(a.id)", "name": a.name, "error": s.state(a.id)?.lastError?.reasonWords ?? "", "sign-in-again": "\(S).\(a.id).sign-in-again"]
            } },
            // One row per mail service in the fixed order; `state` is the machine id, `words` what the pane says (this Mac's accounts only).
            "\(S).service-status": { _ in s.serviceRows.map { r in
                ["ax": "\(S).service-status.\(r.service.rawValue)", "id": r.service.rawValue, "name": r.service.name, "state": r.state.rawValue,
                 "words": r.state.words, "accounts": r.accounts]
            } },
            "\(S).remove.words": { _ in s.removing.map { [["text": s.removalWords($0)]] } ?? [] },
            "\(S).notice": { _ in s.failure.map { [["text": $0]] } ?? [] },
            // The field a door focused and the unsaved text of the account page's two fields (`focus` is "none", "find", "name:<id>" or "signature:<id>").
            "\(S).field": { _ in
                var focus = "none", nameDraft = "", signatureDraft = ""
                switch s.fieldFocus {
                case .name(let id)?: focus = "name:" + id; nameDraft = s.nameDrafts[id] ?? ""
                case .signature(let id)?: focus = "signature:" + id; signatureDraft = s.signatureDrafts[id] ?? ""
                case .find?: focus = "find"
                case nil: break
                }
                return [["focus": focus, "name_draft": nameDraft, "signature_draft": signatureDraft, "find_text": s.query]]
            },
            "\(S).import.sheet": { _ in
                let m = ImportModel.shared
                return [["open": s.importing, "running": m.running, "selected": m.selectedSource?.kind ?? "", "needs_full_disk_access": m.needsFullDiskAccess,
                         "fda_opened": s.fdaOpened, "refusal": m.refusalWords ?? "", "state": m.run?.state ?? "none"]]
            },
            "\(S).import.sources": { _ in
                ImportModel.shared.sources.map { x in
                    ["ax": "\(S).import.source.\(x.kind)", "kind": x.kind, "name": x.name, "count": x.count.map { $0 as Any } ?? NSNull(), "imported": x.imported,
                     "access": x.access, "selected": x.id == ImportModel.shared.selected]
                }
            },
            "\(S).import.result": { _ in
                let m = ImportModel.shared
                return [["line": AccountsImportLine.of(m), "progress": m.running ? m.progressLine : "", "done": m.run?.done ?? 0, "stored": m.result?.stored ?? 0]]
            },
            "\(S).import": { _ in [["ax": "\(S).import", "title": ImportWords.settingsOffer]] },   // the offer to import mail (PRD A83), opens the Welcome import step
        ]
        for a in s.accounts { out["\(S).\(a.id)"] = { _ in s.entry(a.id).map { [row($0, s)] } ?? [] } }
#if os(macOS)
        out["\(S).icon.stored"] = { _ in AccountIconFixtureDoor.storedRows(s) }   // fixture-r3: the stored images (account, px, bytes), one per uploaded icon
#endif
        return out
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        let s = AccountsSession.shared
        var out: [String: (AppModel) -> Bool] = [
            "\(S).move-to-iphone": { m in WelcomeMacFeature.openAtQR(m) },
            "\(S).import": { _ in s.openImport(); return true },
            "\(S).picker.done": { _ in s.editing = nil; return true },
            "\(S).import.start": { _ in guard s.importing else { return false }; ImportModel.shared.start(); return ImportModel.shared.run != nil },
            "\(S).import.cancel": { _ in guard s.importing, ImportModel.shared.running else { return false }; ImportModel.shared.cancel(); return true },
            "\(S).import.close": { _ in guard s.importing, !ImportModel.shared.running else { return false }; s.closeImport(); return true },
            "\(S).import.scrim": { _ in guard s.importing, !ImportModel.shared.running else { return false }; s.closeImport(); return true },
        ]
        for kind in ["apple-mail", "olm", "mbox"] {
            out["\(S).import.source.\(kind)"] = { _ in
                guard s.importing, !ImportModel.shared.running else { return false }
                ImportModel.shared.selectKind(kind); return ImportModel.shared.selectedSource?.kind == kind
            }
        }
        // The file panel and System Settings are never opened by a test: on the fixture core these doors hand the scan a path as the panel
        // would, and record the ask for Full Disk Access (`fda_opened`). On the real core they are the buttons' own actions and the door refuses.
        for (name, path) in [("choose-olm", "~/Documents/Outlook.olm"), ("choose-mbox", "~/Mail/in.mbox"), ("choose-bad", "~/Documents/notes.txt")] {
            out["\(S).import.\(name)"] = { _ in
                guard s.importing, CoreHost.shared.kind == .fixture else { return false }
                ImportModel.shared.scanNow(picking: path); return true
            }
        }
        out["\(S).import.open-settings"] = { _ in
            guard s.importing, CoreHost.shared.kind == .fixture, ImportModel.shared.needsFullDiskAccess else { return false }
            s.fdaOpened = true; return true
        }
        var colours: [String] = []
        for a in s.accounts {
            out["\(S).\(a.id).notifications"] = { _ in s.toggleNotifications(a.id) }
            out["\(S).\(a.id).sign-in-again"] = { m in s.signInAgain(a.id, model: m) }
            out["\(S).\(a.id).icon"] = { _ in s.editing = a.id; return true }
            // The two text fields of the account's page: a click focuses the field for `type`, Return (`.submit`) saves what was typed.
            for (field, focus) in [("name", AccountsSession.Field.name(a.id)), ("signature", AccountsSession.Field.signature(a.id))] {
                out["\(S).\(a.id).\(field).field"] = { _ in
                    guard s.entry(a.id) != nil, SettingsWindowModel.shared.accountPage == a.id else { return false }
                    s.fieldFocus = focus; return true
                }
            }
            out["\(S).\(a.id).name.field.submit"] = { _ in s.submitName(a.id) }
            out["\(S).\(a.id).signature.field.submit"] = { _ in s.submitSignature(a.id) }
            if !colours.contains(a.colour) { colours.append(a.colour) }
        }
        for g in EmblemLibrary.all { out["\(S).icon-library.\(g.id)"] = { _ in s.editing.map { s.pickGlyph(g.id, for: $0) } ?? false } }
        for hex in colours {
            out["\(S).colour.\(hex.trimmingCharacters(in: CharacterSet(charactersIn: "#")))"] = { _ in s.editing.map { s.pickColour(hex, for: $0) } ?? false }
        }
        out["\(S).find.clear"] = { _ in s.query = ""; return true }
        out["\(S).find"] = { _ in guard s.accounts.count > 6 else { return false }; s.fieldFocus = .find; return true }   // the field is drawn from the seventh account on
        // The two scrims: a click outside the card closes the review of a removal and the picker, like their own buttons.
        out["\(S).remove.scrim"] = { _ in guard s.removing != nil else { return false }; s.removing = nil; return true }
        out["\(S).picker.scrim"] = { _ in guard s.editing != nil else { return false }; s.editing = nil; return true }
#if os(macOS)
        // Picker test doors for the sizes of an upload; `fixture upload-icon <bytes|path>` is served by the open picker (fixture-r3).
        let limit = WhooshConstants.ACCOUNT_ICON_MAX_BYTES
        // `\(S).upload` is the Upload button: its own click opens the file panel (a modal the rig cannot answer), so the door takes the
        // picture the panel would give, one of exactly the limit, through the same store and refusal path.
        for (name, bytes) in [("at-limit", limit), ("over-limit", limit + 1), ("button", limit)] {
            out[name == "button" ? "\(S).upload" : "\(S).fixture.upload-\(name)"] = { _ in
                guard let id = s.editing else { return false }
                switch AccountIconStore.fixtureUpload(declaredBytes: bytes, accountId: id) {
                case .success(let path): s.setImage(path: path, for: id)
                case .failure(let e): s.refuse(e.words, for: id)
                }
                return true
            }
        }
        let open: (AppModel) -> Bool = { m in s.reload(); return WindowBridge.open(.macSettingsAccounts, in: m) }
        out["mac-palette.result.accounts-settings"] = open
        out["\(S).open"] = open
#endif
        return out
    }

    public static func paletteCommands() -> [(title: String, key: String)] { [(AccountsWords.paletteTitle, "")] }
}
