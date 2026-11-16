import SwiftUI

/// Registration for the settingsgeneralmac lane: Settings > General, Notifications and Shortcuts on the Mac, and (the
/// registry being frozen) the Settings window's own rows, clicks and keys, which live in Sources/Features/SettingsWindow.
public enum SettingsGeneralMacFeature: FeatureModule {
    public static let id = "settingsgeneralmac"
    public static let surfaces: [String] = ["mac-settings-general"]
    static let P = "mac-settings-general"
    /// The slot the Settings scene mounts: the whole window (sidebar, toolbar, pane).
    public static let windowSlot = SurfaceSlot("mac-settings-window")

    public static func view(for slot: SurfaceSlot) -> AnyView? {
        if slot == windowSlot { return AnyView(SettingsWindowView()) }
        return slot == .macSettingsGeneral ? AnyView(GeneralPaneView()) : nil
    }

    /// The Settings window is the focused feature while any of its panes is the surface.
    public static func hasFocus(_ m: AppModel) -> Bool { m.surface.hasPrefix("mac-settings") }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        let s = GeneralStore.shared
        func err(_ a: GeneralStore.Area) -> (AppModel) throws -> [ControlRow] { { _ in s.errors[a].map { [["text": $0]] } ?? [] } }
        var out: [String: (AppModel) throws -> [ControlRow]] = [
            "\(P).tabs": { _ in GeneralStore.Tab.allCases.map { ["ax": "\(P).tab.\($0.id)", "title": $0.title, "current": $0 == s.tab, "pane": $0.pane.id] } },
            "\(P).notch.error": err(.notch), "\(P).notifications.error": err(.notifications),
            "\(P).shortcuts.error": err(.shortcuts), "\(P).appearance.error": err(.appearance),
            "\(P).settings": { _ in [["emblems": s.showEmblems, "pictures_on_open": s.loadPictures, "hints": TrainerModel.shared.hintsOn, "hidden_in_screen_share": s.hiddenInShare, "notch_shown": s.notchShown,
                                       "follow_focus": s.followFocus, "unread_only": s.unreadOnly, "buttons": s.buttons.map(GeneralWords.button),
                                       "preset": ShortcutEngine.shared.presetId, "language": s.language,
                                       "appearance": s.appearance.rawValue, "density": s.density.rawValue, "motion": s.motion.rawValue,
                                       "undo_seconds": s.undoSeconds,
                                       "alert_stay": notchStayWord(stay: s.alerts.stay), "alert_sound": s.alerts.sound, "alert_pictures": s.alerts.fullMailLoadsPictures,
                                       "pinned_labels": labelsWithoutViews(labels: AppModel.shared.core.pinnedLabels()).map(\.name)]] },
            // the notch's alerts: one row per kind of mail with the style it holds and the five values it can take
            "\(P).alerts": { _ in GeneralStore.alertKinds.map { k in
                ["ax": "\(P).alerts.\(notchKindWord(kind: k))", "kind": notchKindWord(kind: k), "title": GeneralWords.alertKind(k),
                 "style": notchAlertStyleWord(style: s.alertStyle(k)), "style_title": GeneralWords.alertStyle(s.alertStyle(k)),
                 "values": GeneralStore.alertStyles.map { notchAlertStyleWord(style: $0) }]
            } },
            "\(P).alerts.legend": { _ in [NotchAlertStyle.light, .line, .card, .opened].map {
                ["ax": "\(P).alerts.legend.\(notchAlertStyleWord(style: $0))", "title": GeneralWords.alertStyle($0), "line": GeneralWords.alertStyleLine($0)]
            } },
            "\(P).languages": { _ in GeneralLanguagePopup.options.map { ["ax": "\(P).appearance.language.\($0.code)", "code": $0.code, "name": $0.name, "current": $0.code == s.language] } },
            "\(P).pinned.heading": { _ in [["text": GeneralWords.pinnedTitle, "line": GeneralWords.pinnedNote]] },
            "\(P).pinned": { _ in labelsWithoutViews(labels: AppModel.shared.core.labels()).map { ["ax": "\(P).pinned.label.\($0.id)", "name": $0.name, "pinned": $0.pinned] } },
            "\(P).language.error": err(.language), "\(P).pinned.error": err(.labels),
            "\(P).notifications": { _ in AccountBook.shared.accounts().map { ["ax": "\(P).notifications.account.\($0.id)", "name": $0.name, "enabled": !s.isMuted($0.id)] } },
        ]
        #if os(macOS)
        // the notch's key: its caps ("" when there is none), whether the recorder listens, and why the last key was refused
        out["\(P).notch.key"] = { _ in
            let r = NotchKeyRecorder.shared
            return [["ax": "\(P).notch.key", "title": GeneralWords.notchKey, "line": GeneralWords.notchKeyLine, "key": r.words ?? "", "recording": r.recording, "refusal": r.refusal ?? NSNull()]]
        }
        #endif
        for (k, v) in SettingsWindowRegistry.rowSources() { out[k] = v }
        return out
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        let s = GeneralStore.shared
        /// A refused save still did something: the reason is on screen. Only an unknown id does nothing.
        func act(_ save: @escaping @MainActor () -> Bool) -> (AppModel) -> Bool { { _ in _ = save(); return true } }
        var out: [String: (AppModel) -> Bool] = [
            "\(P).quick-start.show": { _ in FirstStartModel.shared.replay() },
            "\(P).notch.enabled": act { s.setNotchShown(!s.notchShown) },
            "\(P).notch.hidden": act { s.setHiddenInShare(!s.hiddenInShare) },
            "\(P).notch.focus-filter": act { s.setFollowFocus(!s.followFocus) },
            "\(P).notch.show": act { s.setUnreadOnly(!s.unreadOnly) },
            "\(P).appearance.emblems": act { s.setShowEmblems(!s.showEmblems) },
            "\(P).privacy.pictures": act { s.setLoadPictures(!s.loadPictures) },
            "\(P).shortcuts.hints": act { s.setHints(!TrainerModel.shared.hintsOn) },
        ]
        // The old six tabs are sections now: a tab click shows the pane that holds its section.
        for k in GeneralStore.alertKinds {
            for st in GeneralStore.alertStyles { out["\(P).alerts.\(notchKindWord(kind: k)).\(notchAlertStyleWord(style: st))"] = act { s.setAlert(k, st) } }
        }
        for st in GeneralStore.alertStays { out["\(P).alerts.stay.\(notchStayWord(stay: st))"] = act { s.setStay(st) } }
        out["\(P).alerts.sound"] = act { s.setAlertSound(!s.alerts.sound) }
        #if os(macOS)
        // the notch's key: Change starts and stops the recorder, Default puts the start key back
        out["\(P).notch.key.change"] = { _ in NotchKeyRecorder.shared.toggle(); return true }
        out["\(P).notch.key.default"] = { _ in NotchKeyRecorder.shared.reset(); return true }
        #endif
        out["\(P).alerts.pictures"] = act { s.setAlertPictures(!s.alerts.fullMailLoadsPictures) }
        for t in GeneralStore.Tab.allCases { out["\(P).tab.\(t.id)"] = { _ in s.tab = t; _ = SettingsWindowModel.shared.select(t.pane); return true } }
        for b in GeneralStore.allButtons { out["\(P).notch.buttons.\(b)"] = { _ in GeneralStore.allButtons.contains(b) && { _ = s.toggleButton(b); return true }() } }
        for id in ShortcutEngine.shared.allPresets.map(\.id) { out["\(P).shortcuts.preset.\(id)"] = act { s.setPreset(id) } }
        for o in GeneralLanguagePopup.options { out["\(P).appearance.language.\(o.code)"] = act { s.setLanguage(o.code) } }
        for a in GeneralStore.Appearance.allCases { out["\(P).appearance.theme.\(a.rawValue)"] = act { s.setAppearance(a) } }
        for d in GeneralStore.Density.allCases { out["\(P).appearance.density.\(d.rawValue)"] = act { s.setDensity(d) } }
        for m in GeneralStore.MotionMode.allCases { out["\(P).appearance.motion.\(m.rawValue)"] = act { s.setMotion(m) } }
        for n in GeneralStore.undoChoices { out["\(P).appearance.undo-send.\(n)"] = act { s.setUndoSeconds(n) } }
        for l in labelsWithoutViews(labels: AppModel.shared.core.labels()) { out["\(P).pinned.label.\(l.id)"] = act { s.togglePin(l.id) } }
        /// The drive's door for what the list never offers: pin a view. It refuses in words and the pane shows them.
        for v in inboxViews() { out["\(P).pinned.ask.pin-view.\(inboxViewId(view: v))"] = act { s.togglePin(inboxViewId(view: v)) } }
        for a in AccountBook.shared.accounts() { out["\(P).notifications.account.\(a.id)"] = act { s.toggleMute(a.id) } }
        for (k, v) in SettingsWindowRegistry.clickTargets() { out[k] = v }
        return out
    }

    /// The Settings window's keys. Asking for any key also applies the stored appearance, once per launch: the settings
    /// store is otherwise created only when the window opens.
    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        GeneralStore.applyStoredAppearanceOnce()
        return SettingsWindowRegistry.keyBindings()
    }

    public static func paletteCommands() -> [(title: String, key: String)] { [] }
}
