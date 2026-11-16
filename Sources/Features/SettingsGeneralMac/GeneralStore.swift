import Foundation
import Observation
#if os(macOS)
import AppKit
#endif

/// What Settings > General holds. The notch values are the core's (GeneralNotchDoor); the others live in the app's
/// defaults until the core serves a door for them. A save asks `FixtureFaults` first so a refusal reads in words.
@MainActor @Observable
public final class GeneralStore {
    public static let shared = GeneralStore()

    public enum Tab: String, CaseIterable, Identifiable, Sendable {
        case shortcuts, notch, notifications, privacy, appearance, sync
        public var id: String { rawValue }
        /// The pane that holds this section of the old six-tab page.
        var pane: SettingsPane {
            switch self {
            case .shortcuts: return .shortcuts
            case .notch, .notifications: return .notifications
            case .privacy, .appearance, .sync: return .general
            }
        }
        var title: String {
            switch self {
            case .shortcuts: return GeneralWords.tabShortcuts
            case .notch: return GeneralWords.tabNotch
            case .notifications: return GeneralWords.tabNotifications
            case .privacy: return GeneralWords.tabPrivacy
            case .appearance: return GeneralWords.tabAppearance
            case .sync: return GeneralWords.tabSync
            }
        }
    }

    /// The part of the pane a refused save is reported in.
    public enum Area: String, Sendable { case shortcuts, notch, notifications, appearance, language, labels }

    public static let emblemsKey = "whoosh.list.showEmblems"
    public static let appearanceKey = "whoosh.appearance"
    public static let densityKey = "whoosh.list.density"
    public static let motionKey = "whoosh.motion"
    public static let undoSendKey = "whoosh.undoSendSeconds"
    static let allButtons = ["mark-read", "archive", "reply", "delete"]   // reply: the notch's Reply button (notch r1)
    private let defaults: UserDefaults

    /// The section the last tab click named (the pane it lives in is shown by the window).
    public var tab: Tab = .shortcuts
    public enum Appearance: String, CaseIterable, Sendable { case auto, light, dark }
    public enum Density: String, CaseIterable, Sendable { case regular, compact }
    public enum MotionMode: String, CaseIterable, Sendable { case full, reduced }
    public private(set) var appearance: Appearance
    public private(set) var density: Density
    public private(set) var motion: MotionMode
    public private(set) var undoSeconds: Int
    /// The undo-send choices: from UNDO_SEND_MIN to UNDO_SEND_MAX in steps of UNDO_SEND_MIN.
    static var undoChoices: [Int] { Array(stride(from: WhooshConstants.UNDO_SEND_MIN, through: WhooshConstants.UNDO_SEND_MAX, by: WhooshConstants.UNDO_SEND_MIN)) }
    public private(set) var showEmblems: Bool
    /// Web pictures load when a mail opens (`PicturesPolicy`); off asks first in each mail.
    public private(set) var loadPictures: Bool
    public private(set) var unreadOnly: Bool
    public private(set) var followFocus: Bool
    public private(set) var hiddenInShare: Bool
    /// The notch on or off: the core's NotchSetting.show (`notch_setting/enabled`), saved with `setNotchSetting` like the other notch values.
    public private(set) var notchShown: Bool
    public private(set) var buttons: [String]
    /// The notch's alert settings: the core's (GeneralNotchDoor.alerts), shown as the core holds them after each save.
    public private(set) var alerts: NotchAlerts
    /// The kinds of mail in the order the pane lists them, and the five values a kind can take (owner's picks of 14 Nov 2026).
    static let alertKinds: [NotchKind] = [.vip, .needsReply, .primary, .updates, .promotions, .signInCode]
    static let alertStyles: [NotchAlertStyle] = [.light, .line, .card, .opened, .nothing]
    static let alertStays: [NotchStay] = [.fiveSeconds, .tenSeconds, .untilActed]
    /// The language the whole app speaks: `Words.system` or a catalogue code. The one stored value is `Words`' own.
    public private(set) var language: String
    /// Counts language and label changes, so the pane re-reads every word and every pinned label on the input frame.
    public private(set) var revision = 0
    /// The reason the last save was refused, per area, in words; cleared by the next save that works.
    public private(set) var errors: [Area: String] = [:]
    /// Counts refusals, so a control can nudge back to where it was.
    public private(set) var refusals = 0

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        func flag(_ k: String, _ d: Bool) -> Bool { (defaults.object(forKey: k) as? Bool) ?? d }
        showEmblems = flag(Self.emblemsKey, true)
        loadPictures = PicturesPolicy.loadsOnOpen
        let notch = GeneralNotchDoor.current()   // the notch values are the core's, never the app's defaults
        unreadOnly = notch.listShows == .unread
        followFocus = notch.followFocus
        hiddenInShare = notch.hiddenInScreenShare
        notchShown = notch.show
        buttons = notch.buttons
        alerts = GeneralNotchDoor.alerts()
        language = Words.selected
        appearance = Appearance(rawValue: defaults.string(forKey: Self.appearanceKey) ?? "") ?? .auto
        density = Density(rawValue: defaults.string(forKey: Self.densityKey) ?? "") ?? .regular
        motion = MotionMode(rawValue: defaults.string(forKey: Self.motionKey) ?? "") ?? .full
        let stored = defaults.object(forKey: Self.undoSendKey) as? Int ?? WhooshConstants.UNDO_SEND_DEFAULT_S
        undoSeconds = min(max(stored, WhooshConstants.UNDO_SEND_MIN), WhooshConstants.UNDO_SEND_MAX)
        // A change made elsewhere (the palette, a launch argument) reaches this pane the same way ours does.
        NotificationCenter.default.addObserver(forName: Words.languageDidChange, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.language = Words.selected; self?.revision += 1 }
        }
    }

    /// One door for every save: a refusal leaves the value where it was and says why.
    @discardableResult
    func save(_ area: Area, op: String, _ apply: () -> Void) -> Bool {
        if let why = FixtureFaults.take(op) {
            errors[area] = why
            refusals += 1
            return false
        }
        errors[area] = nil
        apply()
        return true
    }

    @discardableResult func setShowEmblems(_ on: Bool) -> Bool {
        save(.appearance, op: "setting-save") { showEmblems = on; defaults.set(on, forKey: Self.emblemsKey) }
    }
    @discardableResult func setLoadPictures(_ on: Bool) -> Bool {
        save(.appearance, op: "setting-save") { loadPictures = on; PicturesPolicy.loadsOnOpen = on }
    }
    @discardableResult func setUnreadOnly(_ on: Bool) -> Bool { saveNotch { $0.listShows = on ? .unread : .latest } }
    @discardableResult func setFollowFocus(_ on: Bool) -> Bool { saveNotch { $0.followFocus = on } }
    @discardableResult func setNotchShown(_ on: Bool) -> Bool { saveNotch { $0.show = on } }
    @discardableResult func setHiddenInShare(_ on: Bool) -> Bool { saveNotch { $0.hiddenInScreenShare = on } }
    @discardableResult func toggleButton(_ id: String) -> Bool {
        guard Self.allButtons.contains(id) else { return false }
        return saveNotch { n in if let i = n.buttons.firstIndex(of: id) { n.buttons.remove(at: i) } else { n.buttons.append(id) } }
    }
    /// A notch change goes to the core (GeneralNotchDoor); a refusal, the fixture's or the core's, keeps the old value
    /// and says why. Either way the pane then shows what the core holds.
    private func saveNotch(_ change: (inout NotchSetting) -> Void) -> Bool {
        var refused: String?
        let sent = save(.notch, op: "notch-setting-save") { refused = GeneralNotchDoor.send(change) }
        if let refused { errors[.notch] = refused; refusals += 1 }
        let n = GeneralNotchDoor.current()
        unreadOnly = n.listShows == .unread; followFocus = n.followFocus; hiddenInShare = n.hiddenInScreenShare; notchShown = n.show; buttons = n.buttons
        return sent && refused == nil
    }
    func alertStyle(_ kind: NotchKind) -> NotchAlertStyle { notchAlertStyle(alerts: alerts, kind: kind) }
    @discardableResult func setAlert(_ kind: NotchKind, _ style: NotchAlertStyle) -> Bool { saveAlerts { $0 = notchAlertsWith(alerts: $0, kind: kind, style: style) } }
    @discardableResult func setStay(_ stay: NotchStay) -> Bool { saveAlerts { $0.stay = stay } }
    @discardableResult func setAlertSound(_ on: Bool) -> Bool { saveAlerts { $0.sound = on } }
    @discardableResult func setAlertPictures(_ on: Bool) -> Bool { saveAlerts { $0.fullMailLoadsPictures = on } }
    /// An alert change goes to the core whole (GeneralNotchDoor.sendAlerts); a refusal keeps the old value and says why, in the notch's area.
    private func saveAlerts(_ change: (inout NotchAlerts) -> Void) -> Bool {
        var refused: String?
        let sent = save(.notch, op: "notch-setting-save") { refused = GeneralNotchDoor.sendAlerts(change) }
        if let refused { errors[.notch] = refused; refusals += 1 }
        alerts = GeneralNotchDoor.alerts()
        return sent && refused == nil
    }
    @discardableResult func setPreset(_ id: String) -> Bool {
        var ok = false
        save(.shortcuts, op: "shortcut-preset-save") { ok = ShortcutEngine.shared.setPreset(id) }
        return ok
    }
    @discardableResult func setHints(_ on: Bool) -> Bool {
        save(.shortcuts, op: "setting-save") { TrainerModel.shared.hintsOn = on }
    }
    func isMuted(_ id: String) -> Bool { AccountsSession.shared.isMuted(id) }
    @discardableResult func toggleMute(_ id: String) -> Bool {
        var ok = false
        save(.notifications, op: "notification-setting-save") { ok = AccountsSession.shared.toggleNotifications(id) }
        return ok
    }

    @discardableResult func setAppearance(_ a: Appearance) -> Bool {
        save(.appearance, op: "setting-save") { appearance = a; defaults.set(a.rawValue, forKey: Self.appearanceKey); Self.apply(a) }
    }
    @discardableResult func setDensity(_ d: Density) -> Bool {
        save(.appearance, op: "setting-save") { density = d; defaults.set(d.rawValue, forKey: Self.densityKey) }
    }
    @discardableResult func setMotion(_ m: MotionMode) -> Bool {
        save(.appearance, op: "setting-save") { motion = m; defaults.set(m.rawValue, forKey: Self.motionKey) }
    }
    @discardableResult func setUndoSeconds(_ n: Int) -> Bool {
        guard Self.undoChoices.contains(n) else { return false }
        return save(.appearance, op: "setting-save") { undoSeconds = n; defaults.set(n, forKey: Self.undoSendKey) }
    }

    /// The stored appearance applies once per launch, as soon as the app asks for any key or row.
    private static var appliedStored = false
    static func applyStoredAppearanceOnce() {
        guard !appliedStored else { return }
        appliedStored = true
        let a = Appearance(rawValue: UserDefaults.standard.string(forKey: appearanceKey) ?? "") ?? .auto
        if a != .auto { apply(a) }
    }
    static func apply(_ a: Appearance) {
        #if os(macOS)
        let app: NSApplication? = NSApp   // nil in a process with no application (a unit test): nothing to restyle
        switch a {
        case .auto: app?.appearance = nil
        case .light: app?.appearance = NSAppearance(named: .aqua)
        case .dark: app?.appearance = NSAppearance(named: .darkAqua)
        }
        for w in app?.windows ?? [] { w.appearance = app?.appearance; w.contentView?.needsDisplay = true }
        #endif
    }

    /// The language, applied to the whole app at once: `Words` stores it and tells every window; this pane redraws now.
    @discardableResult func setLanguage(_ code: String) -> Bool {
        save(.language, op: "language-save") { Words.select(code); language = Words.selected; revision += 1 }
    }

    /// Pins or unpins a label (a pinned label is a tab at the top of the list). The core's refusal (the last tab stays) reads in words.
    @discardableResult func togglePin(_ id: String) -> Bool {
        let model = AppModel.shared
        // delta 16: Primary and All are views, never pinnable labels. The list does not offer them; if one is asked for anyway
        // (the drive's door), the contract's door refuses in its own words (field view_not_a_label) and the pane says them.
        if let words = ViewRefusal.words(for: .pinLabel(labelId: id, pinned: true)) {
            errors[.labels] = words
            refusals += 1
            return false
        }
        guard let cur = model.core.labels().first(where: { $0.id == id })?.pinned else { return false }
        var ok = false
        save(.labels, op: "label-pin-save") {
            ok = model.perform(.pinLabel(labelId: id, pinned: !cur))
            if ok { revision += 1 } else {
                errors[.labels] = model.lastFailure?.reasonWords ?? Words.text("Not saved")
                refusals += 1
            }
        }
        return ok
    }
}
