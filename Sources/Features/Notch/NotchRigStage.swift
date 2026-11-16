import Foundation
#if os(macOS)
import AppKit
#endif

/// The rig's stage clicks: interim inputs for what the socket's `fixture` verbs will deliver (N1). Each one
/// reaches the same model entry the real input reaches; none exists outside a drive.
@MainActor
enum NotchRigStage {
    static var model: NotchModel { .shared }

    static func add(to t: inout [String: (AppModel) -> Bool]) {
        #if os(macOS)
        let panel = NotchPanel.shared
        t[NotchIds.stageLight] = { _ in panel.stage(dark: false) }
        t[NotchIds.stageDark] = { _ in panel.stage(dark: true) }
        t[NotchIds.stageNoNotch] = { _ in panel.stage(dark: panel.forcedDark, shape: NotchShape.none) }
        t["mac-notch.stage.notch"] = { _ in model.shapeOverride = nil; return panel.stage(dark: panel.forcedDark) }
        t[NotchIds.stageHover] = { _ in panel.install(); model.hover(true); return true }
        t["mac-notch.stage.leave"] = { _ in panel.install(); panel.pointer.parkAway(); model.hover(false); return true }
        t[NotchIds.stageSample] = { _ in panel.install(); return NotchSample.deliver() }
        t["mac-notch.stage.reset"] = { _ in panel.install(); reset(); return true }
        t[NotchIds.stageSummon] = { m in panel.install(); return model.summon(m) }
        // the panel stopped being the key window (a hidden window never is one): the act that follows, run by its own door
        t["mac-notch.stage.resign"] = { _ in model.keyboardLost(settled: true) }
        // the recorder of the notch's key: a hidden window gets no key press, so two keys reach the recorder's own door here, one it takes
        // (Control and N) and one it refuses (two modifiers)
        for word in ["control-n", "shift-command-n"] { t["mac-notch.stage.key.\(word)"] = { _ in NotchKeyRecorder.shared.takeForRig(word) } }
        t[NotchIds.stageTracked] = { _ in
            panel.install()
            // the same door the reader's event uses: a notification, not a direct call
            NotificationCenter.default.post(name: NotchEvents.trackedOpened, object: nil,
                                            userInfo: ["messageId": "msg-notch-tracked", "subject": "Proposal v3", "who": "Lea Martin", "seenKey": "notch.rig.seen"])
            return model.notice != nil
        }
        t[NotchIds.stageEdge] = { _ in
            panel.install()
            guard let s = NotchGeometry.screen else { return false }
            // beside the housing, inside the card's reach: a rest on the top edge, not on the housing itself
            let at = CGPoint(x: panel.geometry.housing.midX + NotchMetrics.cardWidth / 2 - WhooshSpacing.s8, y: s.frame.maxY - 1)
            panel.pointer.park = at
            panel.pointer.moved(at, force: true)
            return true
        }
        t["mac-notch.stage.twitch.on"] = { _ in panel.install(); NotchTwitch.arm(); return true }
        t["mac-notch.stage.twitch.off"] = { _ in NotchTwitch.stop(); return true }
        t["mac-notch.stage.away"] = { _ in
            guard NotchGeometry.screen != nil else { return false }
            panel.pointer.parkAway()
            return true
        }
        t[NotchIds.stageDropFile] = { _ in panel.install(); NotchFeature.wire(); return model.dropFile(NotchEvents.rigFile("notch-sample-brief.pdf")) }
        // Shots of board a's states: the third sample mail (read, so Unread holds two), the open mail, and aged times.
        t[NotchIds.stageSampleThree] = { _ in panel.install(); return NotchSample.deliverThird() }
        t[NotchIds.stageOpen] = { m in panel.install(); guard let it = model.listed.first else { return false }; return model.open(id: it.id, m) }
        t[NotchIds.stageAged] = { _ in
            panel.install()
            for i in model.items.indices { model.items[i].receivedAt = model.items[i].receivedAt.addingTimeInterval(-Double(600 + 1300 * i)) }
            model.onChange()
            return !model.items.isEmpty
        }
        for on in [true, false] { t[NotchIds.stageFocus(on)] = { _ in panel.install(); model.setFocus(on); return true } }
        let words: [String: [String]] = ["buttons": ["none", "mark-read", "archive", "mark-read+archive", "mark-read+archive+reply", "mark-read+archive+reply+delete", "delete"],
                                         "show": ["latest", "unread"], "delivery": ["notch", "notifications", "both"],
                                         "focus": ["follow", "ignore"]]
        for (key, values) in words {
            for v in values { t[NotchIds.stageSetting(key, v)] = { _ in model.settings.set(key, v) && { model.onChange(); return true }() } }
        }
        // The alerts by kind (owner's picks of 14 Nov 2026). An arrival of one kind goes through the REAL delivery door (NotchFeature.deliver: the
        // engine's kind, the engine's alert settings); the mail is the first one of that kind the engine holds that the notch does not show yet.
        let kinds: [NotchKind] = [.signInCode, .vip, .needsReply, .updates, .promotions, .primary]
        let styles: [NotchAlertStyle] = [.nothing, .light, .line, .card, .opened]
        for kind in kinds {
            t[NotchIds.stageArrive(notchKindWord(kind: kind))] = { m in
                panel.install()
                let all = m.core.inboxRows(query: InboxQuery(tabId: "all", accountId: nil, labelId: nil, mailbox: nil, accountIds: [], filters: [], smartFolderId: nil), rows: 400)
                // a closure, not a local function: it keeps this click's main-actor isolation (a local function does not: the compile error of main176)
                let spare: (NotchKind) -> InboxRow? = { k in all.first { r in !model.items.contains { $0.id == r.messageId } && m.core.notchKind(messageId: r.messageId) == k } }
                // the test engine's sample mail is all Primary: when it holds no mail of this kind, one Primary mail is filed under the kind's tab first
                let tabs: [NotchKind: String] = [.vip: "vips", .needsReply: "needs-reply", .updates: "updates", .promotions: "promotions"]
                if spare(kind) == nil, let tab = tabs[kind], let fixture = m.core as? FixtureCore, let primary = spare(.primary) {
                    fixture.fileForRig(messageId: primary.messageId, tab: tab)
                }
                guard let row = spare(kind) else { return false }
                let code = kind == .signInCode ? m.core.message(messageId: row.messageId)?.loginCode : nil
                NotchFeature.deliver(NotchDrop(messageId: row.messageId, accountId: row.accountId, senderName: row.senderName, subject: row.subject, loginCode: code),
                                     preview: row.preview, threadId: row.threadId, address: row.senderAddress, m)
                return true
            }
            for style in styles {
                t[NotchIds.stageAlert(notchKindWord(kind: kind), notchAlertStyleWord(style: style))] = { m in
                    m.perform(.setNotchAlerts(alerts: notchAlertsWith(alerts: m.core.notchAlerts(), kind: kind, style: style)))
                }
            }
        }
        t[NotchIds.stageDesigned] = { m in
            guard let it = model.openItem ?? model.dropItem, let fixture = m.core as? FixtureCore else { return false }
            fixture.designForRig(messageId: it.id)
            model.wordsOf[it.id] = nil      // the words were read from the written body: read again
            model.onChange()
            return true
        }
        for stay in [NotchStay.fiveSeconds, .tenSeconds, .untilActed] {
            t[NotchIds.stageStay(notchStayWord(stay: stay))] = { m in
                var a = m.core.notchAlerts(); a.stay = stay
                guard m.perform(.setNotchAlerts(alerts: a)) else { return false }
                model.alerts = m.core.notchAlerts(); model.onChange()
                return true
            }
        }
        #endif
    }

    /// A clean notch for the next step of a drive: no mail, no notice, default settings, Focus off.
    static func reset() {
        #if os(macOS)
        NotchPanel.shared.pointer.parkAway()   // a drive never reads the owner's real cursor (it may rest on the notch)
        #endif
        model.endOpenReply()        // a reply a case left open ends with its mail: the next case opens a mail on its words
        model.pointerOn = false
        model.leaveSearch()
        model.handedOver = nil
        model.items = []
        model.stage = .rest
        model.notice = nil
        model.focusOn = false
        model.errorWords = nil
        model.droppedFile = nil
        model.lastArrivalMs = nil   // a drive's next arrival reports its own time, never an earlier one
        model.sentShown = false
        model.settings = NotchSettings()
        // the alert settings are the engine's: a clean notch starts from the picked start values there too
        _ = try? AppModel.shared.core.run(command: .setNotchAlerts(alerts: notchAlertsDefault()))
        model.alerts = notchAlertsDefault()
        model.alertId = nil
        model.soundsAsked = 0
        model.fullMailId = nil
        model.snoozeId = nil
        model.wordsOf = [:]
        model.muted = []
        model.frontApp = ""
        UserDefaults.standard.removeObject(forKey: "notch.rig.seen")
        model.onChange()
    }
}
