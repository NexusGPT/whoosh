#if os(macOS)
import SwiftUI

private let P = "mac-settings-accounts"

/// One account's own page, reached from the list; ⌘[ or the back button returns. Name, address, provider, colour and
/// icon, sync status with Sign in again, pause syncing, notifications, and Remove account… with its review.
struct AccountPageView: View {
    let accountId: String
    @State private var session = AccountsSession.shared

    var body: some View {
        if let a = session.entry(accountId) {
            let failing = session.state(a.id)?.lastError != nil
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    header(a, failing: failing)
                    SettingsGroup(title: AccountsWords.accountSection) {
                        SettingsRow(ax: "\(P).\(a.id).name", title: AccountsWords.nameTitle) {
                            SettingsField(ax: "\(P).\(a.id).name.field", placeholder: AccountsWords.nameTitle, text: nameBinding(a), width: 220) {
                                _ = session.submitName(a.id)
                            }
                        }
                        SettingsRow(ax: "\(P).\(a.id).signature", title: AccountsWords.signatureTitle) {
                            SettingsField(ax: "\(P).\(a.id).signature.field", placeholder: AccountsWords.signaturePlaceholder, text: signatureBinding(a), width: 220) {
                                _ = session.submitSignature(a.id)
                            }
                        }
                        SettingsRow(ax: "\(P).\(a.id).address", title: AccountsWords.addressTitle) {
                            Text(a.address).font(.system(size: 13)).foregroundStyle(WhooshPalette.secondaryLabel).textSelection(.enabled)
                        }
                        SettingsRow(ax: "\(P).\(a.id).provider", title: AccountsWords.providerTitle) {
                            Text(a.provider).font(.system(size: 13)).foregroundStyle(WhooshPalette.secondaryLabel)
                        }
                    }
                    SettingsGroup(title: AccountsWords.syncSection) {
                        SettingsRow(ax: "\(P).\(a.id).status", title: AccountsWords.statusTitle, caption: session.syncLine(a.id), captionStrong: failing) {
                            if failing {
                                SettingsButton(title: AccountsWords.signInAgain, primary: true, ax: "\(P).\(a.id).sign-in-again") {
                                    _ = session.signInAgain(a.id, model: AppModel.shared)
                                }
                            }
                        }
                        SettingsRow(ax: "\(P).\(a.id).pause", title: AccountsWords.pauseTitle, caption: AccountsWords.pauseLine,
                                    onTap: { _ = session.togglePause(a.id) }) { SettingsSwitch(on: a.paused) }
                    }
                    SettingsGroup(title: AccountsWords.notifications) {
                        SettingsRow(ax: "\(P).\(a.id).notifications", title: AccountsWords.bannersTitle, caption: AccountsWords.bannersLine,
                                    onTap: { _ = session.toggleNotifications(a.id) }) { SettingsSwitch(on: !session.isMuted(a.id)) }
                        SettingsRow(ax: "\(P).\(a.id).delivery", title: AccountsWords.deliveryTitle) {
                            SettingsPopup(ax: "\(P).\(a.id).delivery.popup",
                                          options: [NotificationDelivery.notch, .notifications, .both].map { ($0, AccountsWords.delivery($0)) },
                                          selection: session.setting(a.id).delivery ?? .both) { _ = session.setDelivery($0, for: a.id) }
                        }
                    }
                    SettingsNotice(text: session.failure, ax: "\(P).\(a.id).notice")
                    HStack {
                        Spacer()
                        SettingsButton(title: AccountsWords.removeButton + "…", destructive: true, ax: "\(P).\(a.id).remove") { _ = session.askRemove(a.id) }
                    }
                    .padding(.top, 14)
                }
                .frame(maxWidth: SettingsMetrics.contentMax, alignment: .leading)
                .padding(.horizontal, SettingsMetrics.contentSide).padding(.top, SettingsMetrics.contentTop).padding(.bottom, SettingsMetrics.contentBottom)
                .frame(maxWidth: .infinity)
            }
            .overlay { if session.removing == a.id { RemoveReview(session: session, account: a) } }
            .animation(.easeOut(duration: WhooshMotion.readerCrossFade), value: session.removing)
            .accessibilityElement(children: .contain)
            .accessibilityIdentifier("\(P).page.\(a.id)")
            .onDisappear { session.clearDrafts() }
        } else {
            SettingsPaneScaffold(pane: .accounts) {
                SettingsFootnote(text: AccountsWords.findNothing)
            }
        }
    }

    private func signatureBinding(_ a: AccountEntry) -> Binding<String> {
        Binding(get: { session.signatureDrafts[a.id] ?? a.signature }, set: { session.setSignatureDraft($0, for: a.id); session.fieldFocus = nil })
    }

    private func nameBinding(_ a: AccountEntry) -> Binding<String> {
        Binding(get: { session.nameDrafts[a.id] ?? a.name }, set: { session.setNameDraft($0, for: a.id); session.fieldFocus = nil })
    }

    /// `.ahead` the page's head: the account's mark, its name, address and provider, its state in words, and the icon button.
    private func header(_ a: AccountEntry, failing: Bool) -> some View {
        FormSplit(labelMin: SettingsPaneHeader<EmptyView>.labelMin, probeId: "\(P).\(a.id).head", title: a.name, caption: a.address) {
          HStack(spacing: 16) {
            EmblemMark(glyph: EmblemLibrary.resolve(a.iconId), colourHex: a.colour, size: .points(64),
                       state: failing ? .needsSignIn : .live, imagePath: a.imagePath, name: a.name, address: a.address)
            VStack(alignment: .leading, spacing: 3) {
                Text(a.name).font(.system(size: 20, weight: .semibold)).tracking(-0.3).foregroundStyle(WhooshPalette.label)
                Text(a.address + " · " + a.provider).font(.system(size: 12.5)).foregroundStyle(WhooshPalette.secondaryLabel)
                HStack(spacing: 6) {
                    if failing { Circle().fill(WhooshPalette.warning).frame(width: 7, height: 7) }
                    Text(failing ? AccountsWords.needsSignInLine : (a.paused ? AccountsWords.paused : AccountsWords.allWorking))
                        .font(.system(size: 12)).foregroundStyle(WhooshPalette.label)
                }
            }
            Spacer(minLength: 0)
          }
        } control: {
            Button { session.editing = a.id } label: {
                HStack(spacing: 6) {
                    Text(AccountsWords.iconButton + "…").font(.system(size: 12.5, weight: .medium)).foregroundStyle(WhooshPalette.label)
                    Circle().fill(settingsColor(hex: a.colour)).frame(width: 12, height: 12)
                        .overlay(Circle().strokeBorder(Color.black.opacity(0.2), lineWidth: 0.5))
                }
                .padding(.horizontal, 12).frame(height: SettingsMetrics.buttonHeight)
                .background(Capsule().fill(WhooshPalette.fill))
                .overlay(Capsule().strokeBorder(WhooshPalette.separator, lineWidth: 0.5))
                .contentShape(Capsule())
            }
            .buttonStyle(.plain).whooshAx("\(P).\(a.id).icon")
        }
        .padding(16)
        .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(SettingsPalette.card))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(SettingsPalette.cardEdge, lineWidth: 0.5))
        .padding(.top, 4)
        .whooshAx("\(P).\(a.id).head")
    }
}

/// Remove account: a review before anything goes (which account, what it does, Cancel or Remove).
private struct RemoveReview: View {
    let session: AccountsSession
    let account: AccountEntry
    var body: some View {
        ZStack {
            Rectangle().fill(WhooshPalette.windowScrim).onTapGesture { session.removing = nil }
            VStack(alignment: .leading, spacing: 10) {
                Text(AccountsWords.removeTitle).font(.system(size: 15, weight: .semibold)).foregroundStyle(WhooshPalette.label)
                Text(account.name + " · " + account.address).font(.system(size: 12.5, weight: .medium)).foregroundStyle(WhooshPalette.label)
                Text(session.removalWords(account.id)).font(.system(size: 12)).foregroundStyle(WhooshPalette.secondaryLabel).fixedSize(horizontal: false, vertical: true)
                HStack(spacing: 8) {
                    Spacer()
                    SettingsButton(title: AccountsWords.cancel, key: "esc", ax: "\(P).remove.cancel") { session.removing = nil }
                    SettingsButton(title: AccountsWords.removeButton, destructive: true, ax: "\(P).remove.confirm") { _ = session.confirmRemove() }
                }
                .padding(.top, 6)
            }
            .padding(20).frame(width: 340)
            .background(RoundedRectangle(cornerRadius: SettingsMetrics.cardRadius + 4, style: .continuous).fill(WhooshPalette.background))
            .overlay(RoundedRectangle(cornerRadius: SettingsMetrics.cardRadius + 4, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: 0.5))
            .shadow(color: .black.opacity(0.22), radius: 24, y: 10)
            .accessibilityElement(children: .contain).whooshAx("\(P).remove.review")
        }
        // Escape is Cancel while the review is up (the "esc" cap on Cancel was bound to nothing): a zero-size button owns the key,
        // so it works whichever control holds the focus, and goes with the review.
        .background { Button("") { session.removing = nil }.keyboardShortcut(.cancelAction).opacity(0).frame(width: 0, height: 0).accessibilityHidden(true) }
        .transition(.opacity)
    }
}
#endif
