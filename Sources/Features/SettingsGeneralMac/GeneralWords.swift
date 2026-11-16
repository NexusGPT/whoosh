import Foundation

/// Every line Settings > General says, through the 14-language catalogue (the key is the English text).
enum GeneralWords {
    static func t(_ english: String) -> String { Words.text(english) }

    static var title: String { t("General") }
    static var tabShortcuts: String { t("Shortcuts") }
    static var tabNotch: String { t("Notch") }
    static var tabNotifications: String { t("Notifications") }
    static var tabPrivacy: String { t("Privacy") }
    static var tabAppearance: String { t("Appearance") }
    static var tabSync: String { t("Sync") }

    static var messageListTitle: String { t("Message list") }
    static var languageTitle: String { t("Language") }
    static var languageLine: String { t("The whole app speaks this language at once: the main window, the quick reply, Settings and the sidebar. No restart.") }
    static var languageSystem: String { t("System") }
    static var languageHint: String { t("Choose the language") }

    static var pinnedTitle: String { t("Pinned labels") }
    static func pinnedCount(_ n: Int, _ cap: Int) -> String { Words.text("%lld of %lld tabs pinned", n, cap) }
    static var pinnedNone: String { t("No label yet. Add one from the sidebar.") }
    static func pinTip(_ on: Bool) -> String { on ? t("Unpin from the tabs") : t("Pin as a tab") }

    static var presetTitle: String { t("Key map") }
    static var presetLine: String { t("Pick the keys you already know. Single keys only: nothing here needs a chord of three.") }
    static var hintsTitle: String { t("Show a key hint after a click") }
    static var hintsLine: String { t("After you use the mouse, a small note tells you the key that does the same. It stops once you press the key.") }

    static var notchShowTitle: String { t("Show unread mail only") }
    static var notchShowLine: String { t("When the pointer rests on the notch, show unread mail instead of the latest.") }
    // The notch's alerts (owner's picks of 14 Nov 2026): one style per kind of mail, how long an alert stays, sound, the full mail's pictures.
    static var alertsTitle: String { t("How a new mail shows") }
    static var alertsLine: String { t("Each kind of mail has its own style in the notch.") }
    static func alertKind(_ k: NotchKind) -> String {
        switch k {
        case .vip: return t("VIPs")
        case .needsReply: return t("Needs Reply")
        case .primary: return t("Primary")
        case .updates: return t("Updates")
        case .promotions: return "\(t("Promotions")), \(t("Social"))"
        case .signInCode: return t("Sign-in codes")
        }
    }
    /// "Light" here is the notch's light, not the light appearance: its own catalogue key.
    static func alertStyle(_ s: NotchAlertStyle) -> String {
        switch s {
        case .light: return t("notch-alert.light")
        case .line: return t("One line")
        case .card: return t("Card")
        case .opened: return t("Opened")
        case .nothing: return t("Nothing")
        }
    }
    static func alertStyleLine(_ s: NotchAlertStyle) -> String {
        switch s {
        case .light: return t("A colour, no words")
        case .line: return t("Who and what")
        case .card: return t("Two lines and buttons")
        case .opened: return t("The whole mail")
        case .nothing: return ""
        }
    }
    static var alertStay: String { t("It stays") }
    static var alertStayLine: String { t("How long a new mail shows before the notch closes by itself.") }
    static func stay(_ s: NotchStay) -> String {
        switch s {
        case .fiveSeconds: return seconds(WhooshConstants.NOTCH_STAY_SHORT_S)
        case .tenSeconds: return seconds(Int(WhooshConstants.NOTCH_DROP_HOLD_S))
        case .untilActed: return t("Until I act")
        }
    }
    static var alertSound: String { t("Sound") }
    static var notchKey: String { t("The notch's key") }
    static var notchKeyLine: String { t("Opens the notch from any app, with the keyboard in it. The same key closes it.") }
    static var notchKeyPress: String { t("Press the new key") }
    static var change: String { t("Change") }
    static var cancel: String { t("Cancel") }
    static var defaultKey: String { t("Default") }
    static var alertSoundLine: String { t("A short sound with each alert.") }
    static var alertPictures: String { t("Full mail with its pictures") }
    static var alertPicturesLine: String { t("The notch's full mail loads its pictures at once. Tracking pixels stay blocked.") }
    static var notchButtons: String { t("Buttons on a new mail") }
    static var notchButtonsLine: String { t("The actions the notch offers on mail that just arrived.") }
    static var notchFocus: String { t("Follow my Focus") }
    static var notchFocusLine: String { t("The notch stays quiet while a Focus is on.") }
    static var notchEnabled: String { t("Show the notch") }
    static var notchEnabledLine: String { t("New mail drops into the notch. Off, mail only reaches you as notifications.") }
    static var notchHidden: String { t("Hide the notch when I share my screen") }
    static var notchHiddenLine: String { t("The notch is left out of screen recordings and screen sharing.") }

    static var notificationsTitle: String { t("Notify me for") }
    static var notificationsLine: String { t("One switch per account. Mute one without touching the others.") }
    static var noAccounts: String { t("No account yet. Add one in Accounts.") }

    static var privacyLine: String { t("Nothing leaves this Mac unless you send it.") }
    static var privacyMail: String { t("Mail stays on this Mac and on your own provider.") }
    static var privacyAI: String { t("AI runs on this Mac, or on a key you add yourself.") }
    static var privacyShare: String { t("The notch is hidden from screen sharing (set in Notch).") }

    static var emblemsTitle: String { t("Show account emblems") }
    static var picturesTitle: String { t("Load pictures when a mail opens") }
    static var picturesLine: String { t("Off asks first in each mail. Tracking pixels are never loaded. Mail in Junk always asks first.") }
    static var themeTitle: String { t("Appearance") }
    static var themeLine: String { t("Auto follows the Mac.") }
    static func theme(_ a: GeneralStore.Appearance) -> String {
        switch a { case .auto: return t("Auto"); case .light: return t("Light"); case .dark: return t("Dark") }
    }
    static var densityTitle: String { t("List density") }
    static func density(_ d: GeneralStore.Density) -> String {
        switch d { case .regular: return t("Regular · two lines"); case .compact: return t("Compact · one line") }
    }
    static var motionTitle: String { t("Motion") }
    static var motionLine: String { t("Reduce Motion on the Mac turns every movement into a fade.") }
    static func motion(_ m: GeneralStore.MotionMode) -> String {
        switch m { case .full: return t("Full"); case .reduced: return t("Reduced") }
    }
    static var undoTitle: String { t("Undo send") }
    static var undoLine: String { t("How long a sent message can still be taken back.") }
    static func seconds(_ n: Int) -> String { Words.text("%lld seconds", n) }
    static var pinnedNote: String { t("Pinned labels are the tabs above the list") }
    static var languageSystemIn: String { t("System") }
    static var keyMapTitle: String { t("Key map") }
    static var notchSection: String { t("Notch") }
    static var emblemsLine: String { t("Off removes the emblem and gives the text its width back.") }
    static var appearanceTheme: String { t("Follows your Mac, light or dark.") }

    static var syncLine: String { t("Your settings travel to your iPhone, encrypted, on your own iCloud.") }
    static var syncOwn: String { t("Latest change wins. Nothing is sent to a Whoosh server.") }

    static var back: String { t("Not saved") }
    static func off(_ b: Bool) -> String { b ? t("On") : t("Off") }
    static func button(_ id: String) -> String {
        switch id {
        case "mark-read": return t("Mark read")
        case "archive": return t("Archive")
        case "reply": return t("Reply")
        default: return t("Delete")
        }
    }
}
