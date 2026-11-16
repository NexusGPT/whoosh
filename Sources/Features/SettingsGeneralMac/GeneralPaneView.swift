import SwiftUI

private let P = "mac-settings-general"

/// Settings > General: language, look, the message list, pinned labels, then privacy and sync in two quiet lines.
/// Every control is a row of a grouped card (label left, control right, one grey caption); each saves at once.
struct GeneralPaneView: View {
    @State private var store = GeneralStore.shared

    var body: some View {
        SettingsPaneScaffold(pane: .general) {
            SettingsGroup(title: GeneralWords.languageTitle) {
                SettingsRow(ax: "\(P).appearance.language-row", title: GeneralWords.languageTitle, caption: GeneralWords.languageLine) {
                    GeneralLanguagePopup(store: store)
                }
            }
            SettingsNotice(text: store.errors[.language], ax: "\(P).language.error")
            SettingsGroup(title: GeneralWords.tabAppearance) {
                SettingsRow(ax: "\(P).appearance.theme", title: GeneralWords.themeTitle, caption: GeneralWords.themeLine) {
                    GeneralAppearancePicker(store: store)
                }
                SettingsRow(ax: "\(P).appearance.density", title: GeneralWords.densityTitle) {
                    SettingsPopup(ax: "\(P).appearance.density.popup", options: GeneralStore.Density.allCases.map { ($0, GeneralWords.density($0)) },
                                  selection: store.density) { store.setDensity($0) }
                }
                SettingsRow(ax: "\(P).appearance.motion", title: GeneralWords.motionTitle, caption: GeneralWords.motionLine) {
                    SettingsPopup(ax: "\(P).appearance.motion.popup", options: GeneralStore.MotionMode.allCases.map { ($0, GeneralWords.motion($0)) },
                                  selection: store.motion) { store.setMotion($0) }
                }
            }
            SettingsGroup(title: GeneralWords.messageListTitle) {
                SettingsRow(ax: "\(P).appearance.emblems", title: GeneralWords.emblemsTitle, caption: GeneralWords.emblemsLine,
                            onTap: { store.setShowEmblems(!store.showEmblems) }) {
                    SettingsSwitch(on: store.showEmblems, refusals: store.refusals)
                }
                SettingsRow(ax: "\(P).appearance.undo-send", title: GeneralWords.undoTitle, caption: GeneralWords.undoLine) {
                    SettingsPopup(ax: "\(P).appearance.undo-send.popup", options: GeneralStore.undoChoices.map { ($0, GeneralWords.seconds($0)) },
                                  selection: store.undoSeconds) { store.setUndoSeconds($0) }
                }
            }
            SettingsNotice(text: store.errors[.appearance], ax: "\(P).appearance.error")
            PinnedLabelsGroup(store: store)
            SettingsGroup(title: GeneralWords.tabPrivacy, ax: "\(P).privacy.section") {
                SettingsRow(ax: "\(P).privacy.pictures", title: GeneralWords.picturesTitle, caption: GeneralWords.picturesLine,
                            onTap: { store.setLoadPictures(!store.loadPictures) }) {
                    SettingsSwitch(on: store.loadPictures, refusals: store.refusals)
                }
                privacyRow("internaldrive", GeneralWords.privacyMail)
                privacyRow("cpu", GeneralWords.privacyAI)
                privacyRow("eye.slash", GeneralWords.privacyShare)
            }
            SettingsGroup(title: GeneralWords.tabSync) {
                SettingsRow(ax: "\(P).sync.note", title: GeneralWords.syncOwn, caption: GeneralWords.syncLine) {
                    Image(systemName: "icloud").foregroundStyle(WhooshPalette.secondaryLabel)
                }
            }
            SettingsGroup(title: FirstStartWords.quickStart) {   // the first-start flow again: a launch shows it only with no account
                SettingsRow(ax: "\(P).quick-start", title: FirstStartWords.quickStartLine) {
                    SettingsButton(title: FirstStartWords.showAgain, ax: "\(P).quick-start.show") { FirstStartModel.shared.replay() }
                }
            }
        }
        .id(store.language)
        .animation(.easeOut(duration: WhooshMotion.rowInsert), value: store.errors.count)
        .accessibilityIdentifier(P)
    }

    private func privacyRow(_ symbol: String, _ text: String) -> some View {
        SettingsRow(ax: "\(P).privacy.\(symbol)", title: text) { Image(systemName: symbol).foregroundStyle(WhooshPalette.secondaryLabel) }
    }
}

/// Pinned labels: one switch per label; the pinned ones are the tabs above the list. The core's refusal reads in words.
struct PinnedLabelsGroup: View {
    let store: GeneralStore
    var body: some View {
        let labels = labelsWithoutViews(labels: AppModel.shared.core.labels())   // delta 16: Primary and All are views, not pinnable labels
        let pinned = labels.filter(\.pinned).count
        VStack(alignment: .leading, spacing: 0) {
            if labels.isEmpty {
                SettingsGroup(title: GeneralWords.pinnedTitle, ax: "\(P).pinned.section") {
                    SettingsRow(ax: "\(P).pinned.none", title: GeneralWords.pinnedNone) { EmptyView() }
                }
            } else {
                SettingsGroup(title: GeneralWords.pinnedTitle, note: GeneralWords.pinnedNote, ax: "\(P).pinned.section") {
                    ForEach(labels, id: \.id) { l in
                        SettingsRow(ax: "\(P).pinned.label.\(l.id)",
                                    title: l.kind == .builtIn ? Words.text(l.name) : l.path.replacingOccurrences(of: " > ", with: " › "),
                                    dot: settingsColor(hex: l.colour),
                                    onTap: { store.togglePin(l.id) }) {
                            SettingsSwitch(on: l.pinned, refusals: store.refusals)
                        }
                        .help(GeneralWords.pinTip(l.pinned))
                    }
                }
                SettingsFootnote(text: GeneralWords.pinnedCount(pinned, WhooshConstants.TAB_PINNED_MAX), ax: "\(P).pinned.count")
            }
            SettingsNotice(text: store.errors[.labels], ax: "\(P).pinned.error")
        }
        .id(store.revision)
    }
}
