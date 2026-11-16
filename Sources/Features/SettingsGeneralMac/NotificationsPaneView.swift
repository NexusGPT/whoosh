import SwiftUI

private let P = "mac-settings-general"

/// Settings > Notifications: what the notch offers on new mail, when it stays quiet, and one switch per account for
/// banners and sounds. Carried one for one from the six-tab page; each control keeps its id and its save.
struct NotificationsPaneView: View {
    @State private var store = GeneralStore.shared

    var body: some View {
        let accounts = AccountBook.shared.accounts()
        SettingsPaneScaffold(pane: .notifications) {
            // The alerts by kind of mail (owner's picks of 14 Nov 2026): the four styles shown, then one value per kind.
            SettingsGroup(title: GeneralWords.alertsTitle, note: GeneralWords.alertsLine, ax: "\(P).alerts.section") {
                NotchStyleLegend()
                ForEach(GeneralStore.alertKinds, id: \.self) { kind in
                    let word = notchKindWord(kind: kind)
                    SettingsRow(ax: "\(P).alerts.\(word)", title: GeneralWords.alertKind(kind)) {
                        SettingsSegmented(axPrefix: "\(P).alerts.\(word)",
                                          options: GeneralStore.alertStyles.map { (value: $0, key: notchAlertStyleWord(style: $0), title: GeneralWords.alertStyle($0)) },
                                          selection: store.alertStyle(kind)) { store.setAlert(kind, $0) }
                    }
                }
                SettingsRow(ax: "\(P).alerts.stay", title: GeneralWords.alertStay, caption: GeneralWords.alertStayLine) {
                    SettingsSegmented(axPrefix: "\(P).alerts.stay",
                                      options: GeneralStore.alertStays.map { (value: $0, key: notchStayWord(stay: $0), title: GeneralWords.stay($0)) },
                                      selection: store.alerts.stay) { store.setStay($0) }
                }
                SettingsRow(ax: "\(P).alerts.sound", title: GeneralWords.alertSound, caption: GeneralWords.alertSoundLine,
                            onTap: { store.setAlertSound(!store.alerts.sound) }) { SettingsSwitch(on: store.alerts.sound, refusals: store.refusals) }
                SettingsRow(ax: "\(P).alerts.pictures", title: GeneralWords.alertPictures, caption: GeneralWords.alertPicturesLine,
                            onTap: { store.setAlertPictures(!store.alerts.fullMailLoadsPictures) }) { SettingsSwitch(on: store.alerts.fullMailLoadsPictures, refusals: store.refusals) }
            }
            SettingsGroup(title: GeneralWords.notchButtons, note: GeneralWords.notchButtonsLine, ax: "\(P).notch.buttons.section") {
                ForEach(GeneralStore.allButtons, id: \.self) { b in
                    SettingsRow(ax: "\(P).notch.buttons.\(b)", title: GeneralWords.button(b), onTap: { store.toggleButton(b) }) {
                        SettingsCheck(on: store.buttons.contains(b))
                    }
                }
            }
            SettingsGroup(title: GeneralWords.notchSection) {
                SettingsRow(ax: "\(P).notch.enabled", title: GeneralWords.notchEnabled, caption: GeneralWords.notchEnabledLine,
                            onTap: { store.setNotchShown(!store.notchShown) }) { SettingsSwitch(on: store.notchShown, refusals: store.refusals) }
                #if os(macOS)
                NotchKeyRow()
                #endif
                SettingsRow(ax: "\(P).notch.show", title: GeneralWords.notchShowTitle, caption: GeneralWords.notchShowLine,
                            onTap: { store.setUnreadOnly(!store.unreadOnly) }) { SettingsSwitch(on: store.unreadOnly, refusals: store.refusals) }
                SettingsRow(ax: "\(P).notch.focus-filter", title: GeneralWords.notchFocus, caption: GeneralWords.notchFocusLine,
                            onTap: { store.setFollowFocus(!store.followFocus) }) { SettingsSwitch(on: store.followFocus, refusals: store.refusals) }
                SettingsRow(ax: "\(P).notch.hidden", title: GeneralWords.notchHidden, caption: GeneralWords.notchHiddenLine,
                            onTap: { store.setHiddenInShare(!store.hiddenInShare) }) { SettingsSwitch(on: store.hiddenInShare, refusals: store.refusals) }
            }
            SettingsNotice(text: store.errors[.notch], ax: "\(P).notch.error")
            SettingsGroup(title: GeneralWords.notificationsTitle, note: GeneralWords.notificationsLine, ax: "\(P).notifications.section") {
                if accounts.isEmpty {
                    SettingsRow(ax: "\(P).notifications.none", title: GeneralWords.noAccounts) { EmptyView() }
                }
                ForEach(accounts, id: \.id) { a in
                    SettingsRow(ax: "\(P).notifications.account.\(a.id)", title: a.name, caption: a.address, onTap: { store.toggleMute(a.id) }) {
                        SettingsSwitch(on: !store.isMuted(a.id), refusals: store.refusals)
                    }
                }
            }
            SettingsNotice(text: store.errors[.notifications], ax: "\(P).notifications.error")
        }
        .animation(.easeOut(duration: WhooshMotion.rowInsert), value: store.errors.count)
        .accessibilityIdentifier("\(P).notifications")
    }
}

/// Settings > Shortcuts: the key map (the keys you already know) and the hint after a click.
struct ShortcutsPaneView: View {
    @State private var store = GeneralStore.shared
    @State private var trainer = TrainerModel.shared
    @State private var engine = ShortcutEngine.shared

    var body: some View {
        SettingsPaneScaffold(pane: .shortcuts) {
            SettingsGroup(title: GeneralWords.presetTitle, note: GeneralWords.presetLine, ax: "\(P).shortcuts.section") {
                ForEach(engine.allPresets, id: \.id) { p in
                    SettingsRow(ax: "\(P).shortcuts.preset.\(p.id)", title: p.name, caption: p.id == Presets.superhuman.id ? "J K E H L P" : nil,
                                onTap: { store.setPreset(p.id) }) { SettingsCheck(on: engine.presetId == p.id) }
                }
            }
            SettingsGroup(title: GeneralWords.tabShortcuts) {
                SettingsRow(ax: "\(P).shortcuts.hints", title: GeneralWords.hintsTitle, caption: GeneralWords.hintsLine,
                            onTap: { store.setHints(!trainer.hintsOn) }) { SettingsSwitch(on: trainer.hintsOn, refusals: store.refusals) }
            }
            SettingsNotice(text: store.errors[.shortcuts], ax: "\(P).shortcuts.error")
        }
        .animation(.easeOut(duration: WhooshMotion.rowInsert), value: store.errors.count)
        .accessibilityIdentifier("\(P).shortcuts")
    }
}

/// The four alert styles, drawn small (Settings > Notifications, as the owner's pick page drew them): the notch as a light, as one line, as a card and
/// with the mail opened. A legend: it sets nothing; the rows under it do.
struct NotchStyleLegend: View {
    private static let shown: [NotchAlertStyle] = [.light, .line, .card, .opened]

    var body: some View {
        HStack(alignment: .top, spacing: WhooshSpacing.s10) {
            ForEach(Self.shown, id: \.self) { style in
                VStack(alignment: .leading, spacing: WhooshSpacing.s4) {
                    NotchStyleGlyph(style: style)
                    Text(GeneralWords.alertStyle(style)).font(.system(size: 13, weight: .semibold)).foregroundStyle(WhooshPalette.label).lineLimit(1)
                    Text(GeneralWords.alertStyleLine(style)).font(.system(size: 12)).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(2)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .whooshAx("\(P).alerts.legend.\(notchAlertStyleWord(style: style))")
            }
        }
        .padding(.horizontal, WhooshSpacing.s12).padding(.vertical, WhooshSpacing.s10)
        .accessibilityElement(children: .contain)
    }
}

/// One style as a small picture: a strip of screen with the notch in it. The light is the accent colour here (an account's colour in the notch itself).
private struct NotchStyleGlyph: View {
    let style: NotchAlertStyle
    private let canvas = CGSize(width: 120, height: 46)

    private var notch: CGSize {
        switch style {
        case .nothing, .light: return CGSize(width: 38, height: 8)
        case .line: return CGSize(width: 88, height: 8)
        case .card: return CGSize(width: 80, height: 28)
        case .opened: return CGSize(width: 96, height: 40)
        }
    }

    var body: some View {
        ZStack(alignment: .top) {
            RoundedRectangle(cornerRadius: 7, style: .continuous).fill(WhooshPalette.fill)
            UnevenRoundedRectangle(bottomLeadingRadius: 5, bottomTrailingRadius: 5, style: .continuous)
                .fill(Color.black)
                .frame(width: notch.width, height: notch.height)
                .overlay(alignment: .bottom) {
                    if style == .light {
                        Capsule().fill(WhooshPalette.accent).frame(height: 2).padding(.horizontal, 4)
                            .shadow(color: WhooshPalette.accent.color(.light).opacity(0.9), radius: 3)
                    }
                }
                .overlay(alignment: .topLeading) {
                    if style == .card || style == .opened || style == .line {
                        VStack(alignment: .leading, spacing: 3) {
                            Capsule().fill(Color.white.opacity(0.85)).frame(width: style == .line ? 30 : 34, height: 2)
                            if style != .line {
                                Capsule().fill(Color.white.opacity(0.45)).frame(width: 56, height: 2)
                                if style == .opened { Capsule().fill(Color.white.opacity(0.45)).frame(width: 64, height: 2); Capsule().fill(Color.white.opacity(0.45)).frame(width: 48, height: 2) }
                                Capsule().fill(WhooshPalette.accent).frame(width: 18, height: 5).padding(.top, 2)
                            }
                        }
                        .padding(.leading, 8).padding(.top, style == .line ? 3 : 9)
                    }
                }
        }
        .frame(width: canvas.width, height: canvas.height)
        .clipShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
        .accessibilityHidden(true)
    }
}
