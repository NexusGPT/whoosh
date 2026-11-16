import Foundation

/// The one door the notch settings go through: the core's NotchSetting, read with `notchSetting()` and saved with
/// `Command.setNotchSetting`. The pane never keeps a second copy; it shows what the core holds after each save.
@MainActor
enum GeneralNotchDoor {
    static func current(_ m: AppModel = .shared) -> NotchSetting { m.core.notchSetting() }

    /// Sends the changed setting: nil when the core saved it, else the core's reason in words.
    /// On a save the notch reads the core again at once (its window's screen-share exclusion included).
    static func send(_ change: (inout NotchSetting) -> Void, _ m: AppModel = .shared) -> String? {
        var n = current(m)
        change(&n)
        do {
            _ = try m.core.run(command: .setNotchSetting(setting: n))
        } catch CoreError.Action(let failure) {
            return failure.reasonWords
        } catch {
            return "\(error)"
        }
        NotchFeature.rereadSettings(m)
        return nil
    }

    /// The alert settings (one style per kind of mail, the stay, sound, the full mail's pictures): the core's `notchAlerts()`.
    static func alerts(_ m: AppModel = .shared) -> NotchAlerts { m.core.notchAlerts() }

    /// Sends the changed alert settings whole: nil when the core saved them, else the core's reason in words. The notch takes them at once.
    static func sendAlerts(_ change: (inout NotchAlerts) -> Void, _ m: AppModel = .shared) -> String? {
        var a = alerts(m)
        change(&a)
        do {
            _ = try m.core.run(command: .setNotchAlerts(alerts: a))
        } catch CoreError.Action(let failure) {
            return failure.reasonWords
        } catch CoreError.Invalid(_, let words) {
            return words
        } catch {
            return "\(error)"
        }
        NotchModel.shared.alerts = alerts(m)
        return nil
    }
}
