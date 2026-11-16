import SwiftUI

/// Registration for the settingscalendars-mac lane: the Agendas pane of the Settings window, and the read-only macOS calendars.
/// The Settings window mounts the pane through its pane switch (`CalendarsPaneView()` for `SettingsPane.agendas`); this type
/// serves the pane's rows and doors to the control socket. Lines the lead adds outside this folder: docs/lanes/settingscalendars-mac.md.
public enum SettingsCalendarsMacFeature: FeatureModule {
    public static let id = "settingscalendarsmac"
    public static let surfaces: [String] = ["mac-settings-calendars"]

    /// The link on the Sources page: opens this pane in the Settings window.
    @discardableResult
    public static func openPane() -> Bool { SettingsWindowModel.shared.select(.agendas) }

    /// Reads the access state and the calendars once and follows later changes. Called at launch after the core is open, and by the pane.
    public static func start() { CalendarsModel.shared.start() }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        let m = CalendarsModel.shared
        typealias Ids = CalendarsIds
        return [
            Ids.access: { _ in
                var row: ControlRow = ["state": m.state.rawValue, "asking": m.asking, "chip": CalendarWords.chip(for: m.state),
                                       "line": CalendarWords.line(for: m.state, asking: m.asking, noPrompt: m.noPrompt), "no_prompt": m.noPrompt, "opened_settings": m.openedSystemSettings]
                row["ask"] = m.state == .notAsked ? Ids.ask : NSNull()
                row["open_settings"] = m.state == .refused || m.noPrompt ? Ids.openSettings : NSNull()
                return [row]
            },
            Ids.steps: { _ in
                guard m.state == .refused || m.noPrompt else { return [] }
                return CalendarWords.steps.enumerated().map { i, text in ControlRow(dictionaryLiteral: ("n", i + 1), ("text", text), ("ax", Ids.steps + ".\(i + 1)")) }
            },
            Ids.list: { _ in
                guard m.state == .allowed else { return [] }
                return m.calendars.map { c in
                    ControlRow(dictionaryLiteral: ("id", c.id), ("name", c.name), ("account", c.account.isEmpty ? CalendarWords.onThisMac : c.account),
                               ("colour", c.colourHex), ("ax", Ids.list + "." + c.id))
                }
            },
            Ids.listState: { _ in
                let word = m.state != .allowed ? "hidden" : (m.loading ? "loading" : (m.calendars.isEmpty ? "empty" : "list"))
                return [ControlRow(dictionaryLiteral: ("state", word), ("count", m.calendars.count), ("title", CalendarWords.listTitle), ("note", CalendarWords.readOnly))]
            },
            Ids.footnote: { _ in [ControlRow(dictionaryLiteral: ("text", CalendarWords.footnote))] },
            Ids.feed: { _ in
                let f = m.feed.snapshot()
                return [ControlRow(dictionaryLiteral: ("calendars", f.calendars), ("events", f.events), ("event_calendars", f.eventCalendars), ("enabled", f.enabled))]
            },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        let m = CalendarsModel.shared
        typealias Ids = CalendarsIds
        var out: [String: (AppModel) -> Bool] = [
            // The pane is reached by the window's own `mac-settings-window.pane-row.agendas`; this door selects it without opening a window.
            Ids.pane + ".open": { _ in openPane() },
            // The link in the heading of Calendars on Settings > Replies > Sources (SourcesPane.swift) calls the same function.
            "mac-settings-replies.sources.calendars.open": { _ in openPane() },
            Ids.ask: { _ in m.ask() },
            Ids.openSettings: { _ in m.openSystemSettings() },
        ]
        // The rig's doors for the access state: the same as `fixture calendar-access <word>` (the verb is served once the lead adds it).
        for word in ["not-asked", "allowed", "refused", "will-refuse", "no-prompt", "no-answer"] { out[Ids.fixtureDoor(word)] = { _ in m.fixtureSet(word) } }
        CalendarsFixture.listen(m)
        return out
    }
}
