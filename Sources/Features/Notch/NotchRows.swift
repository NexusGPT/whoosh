import Foundation

/// What the socket's `rows` reads on the notch (ids: docs/lanes/tests-j6.md "Assumed ids"; round 1 of look A adds the
/// band, the wings, the list's end mark, the foot, the open mail's header, attachments and From line).
@MainActor
enum NotchRows {
    static var model: NotchModel { .shared }
    /// Where the shape sits along the top edge on a Mac without a housing (0 left, 1 right): the option-drag's place (NotchDrag.fraction); 0.5 elsewhere.
    static var dragFraction: Double {
        #if os(macOS)
        return NotchDrag.shared.fraction
        #else
        return 0.5
        #endif
    }
    static var qr: QuickReplyModel { .shared }
    static var all: [String: (AppModel) throws -> [ControlRow]] {
        [
            "mac-notch.twitch": { _ in NotchTwitch.read().split(separator: "\n").map { ["line": String($0)] } },
            NotchIds.surface: { _ in [withWindowFacts(["ax": NotchIds.surface, "stage": stageWord, "parked": parked, "keyed": keyed, "iskey": isKey, "shape": model.shapeOverride?.rawValue ?? "screen",
                                      "fraction": dragFraction, "focus": model.focusOn, "show": model.settings.show.rawValue,
                                      "delivery": model.settings.delivery.rawValue, "items": model.arrived.count,
                                      "unread": model.unreadCount, "wings": model.wingsShown, "width": shapeWidth,
                                      "sounds": model.soundsAsked, "alert": alertItem?.id ?? NSNull(), "alert_kind": alertItem.map { notchKindWord(kind: $0.kind) } ?? NSNull()])] },
            NotchIds.light: { m in
                guard model.stage == .light, let it = model.dropItem else { return [] }
                return [["ax": NotchIds.light, "id": it.id, "account": it.accountId, "colour": RowAccount.of(it.accountId, in: m).colour]]
            },
            NotchIds.line: { _ in
                guard model.stage == .line, let it = model.dropItem else { return [] }
                return [["ax": NotchIds.line, "id": it.id, "sender": it.sender, "subject": it.subject, "key": "Enter"]]
            },
            NotchIds.alerts: { _ in
                let a = model.alerts
                return [["ax": NotchIds.alerts, "vip": notchAlertStyleWord(style: a.vip), "needs_reply": notchAlertStyleWord(style: a.needsReply),
                         "primary": notchAlertStyleWord(style: a.primary), "updates": notchAlertStyleWord(style: a.updates),
                         "promotions": notchAlertStyleWord(style: a.promotions), "sign_in_codes": notchAlertStyleWord(style: a.signInCodes),
                         "stay": notchStayWord(stay: a.stay), "sound": a.sound, "full_mail_loads_pictures": a.fullMailLoadsPictures]]
            },
            NotchIds.wings: { m in
                guard model.wingsShown, let it = model.newestUnread else { return [] }
                return [["ax": NotchIds.wings, "account": RowAccount.of(it.accountId, in: m).name, "unread": model.unreadCount]]
            },
            NotchIds.band: { m in band(m).map { [["ax": NotchIds.band, "left": $0.left, "right": $0.right]] } ?? [] },
            NotchIds.list: { m in
                guard model.stage != .rest else { return [] }
                let book = AccountBook.shared.accounts(core: m.core)
                return model.listed.enumerated().map { i, it -> ControlRow in
                    let mark = AccountBook.mark(of: it.accountId, in: book)
                    return ["ax": NotchIds.row(i + 1), "id": it.id, "sender": it.sender, "subject": it.subject,
                     "account": it.accountId, "account_icon": mark.iconValue, "account_colour": mark.colour, "code": it.code ?? NSNull(), "read": it.read, "time": it.timeWords(), "kind": notchKindWord(kind: it.kind)]
                }
            },
            NotchIds.end: { _ in
                guard model.stage == .list, model.settings.show == .unread, !model.listed.isEmpty else { return [] }
                return [["ax": NotchIds.end, "text": NotchCopy.empty, "detail": NotchCopy.unreadCount(model.unreadCount)]]
            },
            NotchIds.foot: { _ in
                guard model.stage == .list else { return [] }
                return [["ax": NotchIds.foot, "sharing": model.settings.hiddenInScreenShare ? NotchCopy.hiddenWhileSharing : "", "key": "Escape", "action": NotchCopy.close]]
            },
            NotchIds.codePill: { _ in
                guard let it = model.codeItem, let code = it.code else { return [] }
                return [["ax": NotchIds.codePill, "text": it.codeCopied ? NotchCopy.copied : code]]
            },
            NotchIds.empty: { _ in model.stage == .list && model.listed.isEmpty ? [["ax": NotchIds.empty, "text": NotchCopy.empty]] : [] },
            NotchIds.error: { _ in
                let words = model.errorWords ?? NotchFailure.replyWords(model)
                return words.map { [["ax": NotchIds.error, "text": $0]] } ?? []
            },
            // the open mail's words (the notch's rule over the reader's body): `text` is the paragraphs, one per line; `preview` the arrival's own line
            NotchIds.openBody: { m in
                guard let it = model.openItem, !model.replyTargetsOpen, model.fullMailId != it.id else { return [] }
                let w = model.words(of: it, m)
                let shown = Array((w.paragraphs.isEmpty && !it.preview.isEmpty ? [it.preview] : w.paragraphs).prefix(WhooshConstants.NOTCH_WORDS_MAX_PARAGRAPHS))
                return [["ax": NotchIds.openBody, "id": it.id, "text": shown.joined(separator: "\n"), "paragraphs": shown.count, "preview": it.preview]]
            },
            NotchIds.openFolds: { m in
                guard let it = model.openItem, !model.replyTargetsOpen, model.fullMailId != it.id else { return [] }
                let w = model.words(of: it, m)
                return w.folds.isEmpty ? [] : [["ax": NotchIds.openFolds, "text": NotchCopy.folded(w.folds), "count": w.folds.count]]
            },
            NotchIds.openFullMail: { m in
                guard let it = model.openItem, !model.replyTargetsOpen else { return [] }
                let w = model.words(of: it, m)
                guard w.hasPage else { return [] }
                let full = model.fullMailId == it.id
                return [["ax": NotchIds.openFullMail, "title": full ? NotchCopy.itsWords : NotchCopy.fullMail, "key": NotchFeature.fullMailKey, "full": full,
                         "pictures": Int(w.pictures), "trackers": Int(w.trackers), "loads_pictures": model.alerts.fullMailLoadsPictures,
                         "facts": full ? NotchCopy.pageFacts(pictures: Int(w.pictures), trackers: Int(w.trackers), loaded: model.alerts.fullMailLoadsPictures) : NotchCopy.picturesNotLoaded(Int(w.pictures))]]
            },
            NotchIds.openSnooze: { _ in
                guard let it = model.openItem, !model.replyTargetsOpen else { return [] }
                let open = model.snoozeId == it.id
                let choices: [[String: String]] = open ? NotchSnoozeChoices.choices.map { ["ax": NotchIds.openSnoozePick($0.word), "title": NotchCopy.snoozeChoice($0.when), "key": $0.key] } : []
                return [["ax": NotchIds.openSnooze, "key": NotchFeature.snoozeKey, "open": open, "choices": choices]]
            },
            NotchIds.openFolded: { m in
                guard let it = model.openItem, model.replyTargetsOpen else { return [] }
                return [["ax": NotchIds.openFolded, "sender": it.sender, "text": model.firstWords(of: it, m)]]
            },
            NotchIds.openHeader: { _ in
                guard let it = model.openItem else { return [] }
                return [["ax": NotchIds.openHeader, "sender": it.sender, "address": it.address, "time": it.timeWords(), "subject": it.subject,
                         "back": NotchCopy.backWord(model)]]
            },
            NotchIds.openAttachments: { m in
                guard let it = model.openItem else { return [] }
                return m.core.attachments(messageId: it.id).map { ["ax": NotchIds.openAttachments, "name": $0.fileName, "size": Int($0.sizeBytes)] }
            },
            // the open mail's buttons as drawn, left to right (owner's picks of 14 Nov 2026): Reply, Archive, Mark read while unread, Snooze; while
            // the reply is written the card is the reply's and draws none of them (its send is the row at mac-notch.open.send)
            NotchIds.openActions: { _ in
                guard let it = model.openItem, !model.replyTargetsOpen else { return [] }
                var acts: [ControlRow] = [["title": NotchCopy.reply, "key": NotchFeature.replyKey, "ax": NotchIds.openReply],
                                          ["title": NotchCopy.archive, "key": NotchFeature.archiveKey, "ax": NotchIds.openArchive]]
                if !it.read { acts.append(["title": NotchCopy.markRead, "key": NotchFeature.readKey, "ax": NotchIds.openMarkRead]) }
                acts.append(["title": NotchCopy.snooze, "key": NotchFeature.snoozeKey, "ax": NotchIds.openSnooze])
                return acts
            },
            NotchIds.openFrom: { m in
                guard model.openItem != nil, qr.isOpen else { return [] }
                let picks = qr.accounts.map { a -> ControlRow in ["ax": NotchIds.openFromPick(a.id), "address": a.address, "chosen": a.id == qr.context.accountId] }
                return [["ax": NotchIds.openFrom, "address": qr.context.from, "account": qr.context.accountId, "open": qr.fromOpen,
                         "name": RowAccount.of(qr.context.accountId, in: m).name, "picks": picks]]
            },
            NotchIds.openQuickReply: { _ in
                guard model.replyTargetsOpen else { return [] }   // the reply as drawn: only when it answers the mail open in the notch
                return [["ax": NotchIds.openQuickReply, "reply_to": qr.replyMessageId ?? NSNull(), "text": qr.draft, "focused": true, "placeholder": qr.context.placeholder, "key": NotchReply.sendCap]]
            },
            NotchIds.undo: { _ in
                if case .counting(_, let c) = qr.phase, model.openItem != nil {
                    return [["ax": NotchIds.undo, "progress": 1 - c.fraction(at: Date()), "text": NotchCopy.sending, "action": NotchCopy.undo]]
                }
                return model.sentShown ? [["ax": NotchIds.undo, "progress": 1.0, "text": NotchCopy.sent, "action": NSNull()]] : []
            },
            NotchIds.notice: { _ in model.notice.map { [["ax": NotchIds.notice, "id": $0.messageId, "text": $0.line]] } ?? [] },
            NotchIds.buttons: { _ in
                guard model.stage == .drop, model.notice == nil, let first = model.dropItem else { return [] }
                return model.settings.buttons.compactMap { b -> ControlRow? in
                    switch b {
                    case .markRead: return first.read ? nil : ["title": NotchCopy.markRead, "key": NotchFeature.readKey]
                    case .archive: return ["title": NotchCopy.archive, "key": NotchFeature.archiveKey]
                    case .reply: return ["title": NotchCopy.reply, "key": NotchFeature.replyKey]
                    case .delete: return ["title": NotchCopy.delete]
                    }
                }
            },
            NotchIds.drop: { _ in model.droppedFile.map { [["ax": NotchIds.drop, "name": $0]] } ?? [] },
        ]
    }

    static var stageWord: String {
        switch model.stage { case .rest: return "rest"; case .light: return "light"; case .line: return "line"; case .drop: return "drop"; case .list: return "list"; case .open: return "open" }
    }

    /// The mail the alert shape shows (a light, a line, a card); nil at rest, on the list and on an open mail.
    static var alertItem: NotchItem? {
        switch model.stage { case .light, .line, .drop: return model.dropItem; case .rest, .list, .open: return nil }
    }

    /// The width the stage asks of the shape (board a: 460 / 480 / 540; at rest the housing, or the housing plus two wings).
    static var shapeWidth: Double {
        switch model.stage {
        case .rest, .light: return Double(model.wingsShown ? 2 * NotchMetrics.restWing : 0)   // plus the housing, which the screen sets
        case .line: return Double(NotchMetrics.lineWidth)
        case .drop: return Double(NotchMetrics.cardWidth)
        case .list: return Double(NotchMetrics.listWidth)
        case .open: return Double(NotchMetrics.openWidth)
        }
    }

    /// The band beside the camera, in words: what its left and right wings say at each stage.
    static func band(_ m: AppModel) -> (left: String, right: String)? {
        switch model.stage {
        case .rest, .light: return nil
        case .line:
            guard let it = model.dropItem else { return nil }
            return (it.sender, it.subject)
        case .drop:
            guard let it = model.dropItem else { return nil }
            return (RowAccount.of(it.accountId, in: m).name, it.timeWords())
        case .list: return (model.settings.show == .unread ? NotchCopy.unread : NotchCopy.latest, NotchCopy.openInWhoosh)
        case .open: return (NotchCopy.backWord(model), NotchCopy.openInWhoosh)
        }
    }
}
