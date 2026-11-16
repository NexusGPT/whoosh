import Foundation

/// The notch's settings, read from the core's state probe at the contract's `notch_setting` paths
/// (docs/lanes/notch.md N14); the defaults below hold until the core serves a value, and the rig's stage
/// clicks set them for a drive.
public struct NotchSettings: Equatable, Sendable {
    /// GotMail 0.9.0: the buttons on a new mail.
    public enum Button: String, CaseIterable, Sendable { case markRead = "mark-read", archive, reply, delete }   // reply: the owner's ask; Settings saves it
    /// GotMail 0.7.2: latest mail, or only unread, when the pointer rests on the notch.
    public enum Show: String, Sendable { case latest, unread }
    /// GotMail 0.7.2: Settings, Notifications: the notch, notifications, or both.
    public enum Delivery: String, Sendable {
        case notch, notifications, both
        var usesNotch: Bool { self != .notifications }
    }

    public var buttons: [Button] = [.markRead, .archive, .reply]   // the picked card (14 Nov 2026) shows Reply; the owner can still take it off
    public var show: Show = .latest
    public var delivery: Delivery = .both
    public var followFocus = true
    public var hiddenInScreenShare = true

    public init() {}

    /// Reads what the core serves. Both engines answer five `notch_setting` paths (core/crates/engine/src/settings_cmds.rs `state`,
    /// FixtureCore.swift `notch_setting`): `enabled` (bool: the notch is used at all), `show` (the WORD "latest" or "unread": what the
    /// list holds), `buttons` (array of words, or the word "none"), `hidden_in_screen_share` and `follow_focus` (bools).
    /// `enabled` false puts delivery on notifications only; true brings a notifications-only delivery back to both.
    /// An answer of another kind, or an absent or unreadable path, keeps the value it had.
    public static func read(_ core: WhooshCore, from base: NotchSettings = NotchSettings(), seen: inout [String: String]) -> NotchSettings {
        read(answer: { core.state(path: $0) }, from: base, seen: &seen)
    }

    /// The same read over any answer for a path (the core's `state`).
    static func read(answer: (String) -> String?, from base: NotchSettings = NotchSettings(), seen: inout [String: String]) -> NotchSettings {
        var s = base
        /// A path is applied only when the core's answer for it changed since the last read, so a value the rig set
        /// (or the person's own choice held here) is not overwritten by an answer the core has been giving all along.
        func changed(_ path: String) -> Any? {
            guard let json = answer(path), seen[path] != json else { return nil }
            seen[path] = json
            return try? JSONSerialization.jsonObject(with: Data(json.utf8), options: [.fragmentsAllowed])
        }
        if let v = changed("notch_setting/buttons") {
            if let list = v as? [String] { s.buttons = list.compactMap(Button.init(rawValue:)) }
            if let word = v as? String, word == "none" { s.buttons = [] }
        }
        if let on = boolean(changed("notch_setting/enabled")) { s.delivery = on ? (s.delivery == .notifications ? .both : s.delivery) : .notifications }
        if let word = changed("notch_setting/show") as? String, let v = Show(rawValue: word) { s.show = v }
        if let on = boolean(changed("notch_setting/follow_focus")) { s.followFocus = on }
        if let on = boolean(changed("notch_setting/hidden_in_screen_share")) { s.hiddenInScreenShare = on }
        return s
    }

    /// A JSON true or false only: a number, a word or a list is the wrong kind (NSNumber bridges to Bool, so the type id is checked).
    private static func boolean(_ v: Any?) -> Bool? {
        guard let n = v as? NSNumber, CFGetTypeID(n) == CFBooleanGetTypeID() else { return nil }
        return n.boolValue
    }

    /// The rig's stage click `mac-notch.stage.setting.<key>.<value>`; false for a key or value it does not know.
    public mutating func set(_ key: String, _ word: String) -> Bool {
        switch key {
        case "buttons":
            if word == "none" { buttons = []; return true }
            let list = word.split(separator: "+").compactMap { Button(rawValue: String($0)) }
            guard !list.isEmpty else { return false }
            buttons = list
        case "show": guard let v = Show(rawValue: word) else { return false }; show = v
        case "delivery": guard let v = Delivery(rawValue: word) else { return false }; delivery = v
        case "focus": followFocus = word == "follow"
        default: return false
        }
        return true
    }
}

/// GotMail 0.8.0: a one-time notice the first time a tracked message is opened by its recipient.
public struct NotchNotice: Equatable, Sendable {
    public var messageId: String
    public var subject: String
    public var who: String
    public var line: String { Words.text("%@ opened \u{201C}%@\u{201D}", who, subject) }
}

extension NotchModel {
    /// Shown once per message, ever: a second open of the same message changes nothing.
    @discardableResult
    public func trackedOpened(messageId: String, subject: String, who: String, seen: NotchSeenStore = .standard) -> Bool {
        guard settings.delivery.usesNotch, !(focusOn && settings.followFocus), seen.markFirst(messageId) else { return false }
        notice = NotchNotice(messageId: messageId, subject: subject, who: who)
        if stage == .rest || stage == .light || stage == .line { stage = .drop }   // the notice is a card; a light or a line gives way to it
        dropToken &+= 1
        onChange()
        return true
    }

    public func dismissNotice() { notice = nil; if stage == .drop, items.isEmpty { stage = .rest }; onChange() }
}

/// Which tracked messages already showed their notice; kept in the app's own defaults.
public struct NotchSeenStore: Sendable {
    let key: String
    public static let standard = NotchSeenStore(key: "notch.tracked-open.seen")
    func markFirst(_ id: String) -> Bool {
        let d = UserDefaults.standard
        var seen = Set(d.stringArray(forKey: key) ?? [])
        guard seen.insert(id).inserted else { return false }
        d.set(Array(seen), forKey: key)
        return true
    }
}
