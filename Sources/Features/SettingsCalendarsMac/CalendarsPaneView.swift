import SwiftUI

/// Settings > Agendas: the state of macOS Calendar access, one button to ask once, then the calendars macOS gives (read only here).
/// The switches that make a calendar a source for replies are in Settings > Replies > Sources, not here.
struct CalendarsPaneView: View {
    @State private var model = CalendarsModel.shared

    var body: some View {
        SettingsPaneScaffold(pane: .agendas) {
            CalendarAccessGroup(model: model)
            if model.state == .refused || model.noPrompt { CalendarStepsGroup(model: model).transition(.opacity) }
            if model.state == .allowed { CalendarListGroup(model: model).transition(.opacity) }
            SettingsFootnote(text: CalendarWords.footnote, ax: CalendarsIds.footnote)
        }
        .animation(.easeOut(duration: WhooshMotion.rowInsert), value: model.state)
        .animation(.easeOut(duration: WhooshMotion.rowInsert), value: model.calendars)
        .onAppear { model.start(); model.refresh() }
        .accessibilityIdentifier(CalendarsIds.pane)
    }
}

/// The ids the socket reads and clicks, in one place.
enum CalendarsIds {
    static let pane = "mac-settings-calendars"
    static let access = pane + ".access"
    static let ask = access + ".ask"
    static let openSettings = access + ".open-settings"
    static let steps = pane + ".steps"
    static let list = pane + ".list"
    static let listState = list + ".state"
    static let footnote = pane + ".footnote"
    static let feed = pane + ".feed"
    static func fixtureDoor(_ word: String) -> String { pane + ".fixture." + word }
}

/// The state of the access, and the one button while it was not asked.
struct CalendarAccessGroup: View {
    let model: CalendarsModel

    var body: some View {
        SettingsGroup(ax: CalendarsIds.access + ".section") {
            SettingsRow(ax: CalendarsIds.access, title: CalendarWords.accessTitle, caption: CalendarWords.line(for: model.state, asking: model.asking, noPrompt: model.noPrompt)) {
                switch model.state {
                case .notAsked:
                    SettingsButton(title: CalendarWords.allowAccess, primary: true, ax: CalendarsIds.ask) { model.ask() }
                        .disabled(model.asking).opacity(model.asking ? 0.5 : 1)
                case .allowed:
                    SettingsChip(text: CalendarWords.chipAllowed, tone: .good)
                case .refused:
                    SettingsChip(text: CalendarWords.chipRefused, tone: .warn)
                }
            }
        }
    }
}

/// The refused state: the plain steps to allow it in System Settings, and a button that opens it.
struct CalendarStepsGroup: View {
    let model: CalendarsModel

    var body: some View {
        SettingsGroup(title: CalendarWords.stepsTitle, ax: CalendarsIds.steps) {
            ForEach(Array(CalendarWords.steps.enumerated()), id: \.offset) { i, text in
                SettingsRow(ax: CalendarsIds.steps + ".\(i + 1)", title: text, leading: AnyView(StepNumber(n: i + 1))) {
                    if i == 0 { SettingsButton(title: CalendarWords.openSettings, ax: CalendarsIds.openSettings) { model.openSystemSettings() } }
                }
            }
        }
    }
}

/// A drawn step number: a small grey disc with the digit.
struct StepNumber: View {
    let n: Int
    var body: some View {
        Text("\(n)").font(.system(size: 11, weight: .semibold)).monospacedDigit().foregroundStyle(WhooshPalette.secondaryLabel)
            .frame(width: 20, height: 20).background(Circle().fill(WhooshPalette.fill)).accessibilityHidden(true)
    }
}

/// The calendars macOS gives: a colour dot, the name, the account. No control: a reading of what macOS holds.
struct CalendarListGroup: View {
    let model: CalendarsModel

    var body: some View {
        SettingsGroup(title: CalendarWords.listTitle, note: CalendarWords.readOnly, ax: CalendarsIds.list + ".section") {
            if model.loading {
                SettingsRow(ax: CalendarsIds.list + ".loading", title: CalendarWords.reading) { EmptyView() }
            } else if model.calendars.isEmpty {
                SettingsRow(ax: CalendarsIds.list + ".empty", title: CalendarWords.emptyTitle, caption: CalendarWords.emptyLine) { EmptyView() }
            } else {
                ForEach(model.calendars) { c in
                    SettingsRow(ax: CalendarsIds.list + "." + c.id, title: c.name,
                                caption: c.account.isEmpty ? CalendarWords.onThisMac : c.account,
                                leading: AnyView(Circle().fill(settingsColor(hex: c.colourHex)).frame(width: 10, height: 10).accessibilityHidden(true))) { EmptyView() }
                }
            }
        }
    }
}
