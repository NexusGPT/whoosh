import SwiftUI

/// One mail open in place. Owner's picks of 14 Nov 2026 (docs/surfaces/mac-notch.md): the mail's WORDS by default (the sender's own new text; what the
/// engine folds is one line), and on a mail that has a page of its own the button "Full mail" (O), which shows the sender's page in the same card with
/// its pictures loaded; "Its words" goes back. Under it the buttons: Reply (the one filled control), Archive, Mark read and Snooze as picture and key,
/// and a menu for the rest. Reply folds the mail to one line and gives the card to the reply. Whoosh itself never comes to the front.
struct NotchOpenView: View {
    let item: NotchItem
    let model: NotchModel
    @Bindable var app = AppModel.shared

    private var account: RowAccount { RowAccount.of(item.accountId, in: app) }
    private var attachments: [AttachmentFile] { app.core.attachments(messageId: item.id) }
    private var words: NotchWords { model.words(of: item, app) }
    private var full: Bool { model.fullMailId == item.id && words.hasPage }

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            if model.replyTargetsOpen {
                foldedMail
                NotchReply(model: model)
            } else {
                header
                Text(item.subject).whooshText(WhooshTypography.title3, emphasized: true).foregroundStyle(WhooshPalette.label).lineLimit(2)
                if full {
                    NotchPage(item: item, loadPictures: model.alerts.fullMailLoadsPictures)
                    pageLine
                } else {
                    NotchWordsBlock(paragraphs: words.paragraphs.isEmpty && !item.preview.isEmpty ? [item.preview] : words.paragraphs)
                    if !words.folds.isEmpty { foldLine }
                    attachmentChips
                    if words.hasPage { wordsLine }
                }
                if let code = item.code { NotchCodeControls(code: code, copied: item.codeCopied) }
                actions
                if model.snoozeId == item.id { NotchSnoozeChoices(model: model).transition(.opacity) }
            }
        }
        .animation(WhooshMotion.hover, value: model.snoozeId)
    }

    private var header: some View {
        HStack(alignment: .center, spacing: WhooshSpacing.s10) {
            AccountMark(name: account.name, address: account.address, colour: account.colour, icon: account.iconId.map { AccountMarkIcon.library($0) } ?? AccountMarkIcon.providerDefault,
                        size: .sidebar, needsAttention: account.needsAttention, hasUnread: false, onSelection: false)
                .notchTip(Words.text("To %@ · %@", account.name, account.address))
            VStack(alignment: .leading, spacing: 0) {
                Text(item.sender).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label)
                if !item.address.isEmpty { Text(item.address).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel) }
            }
            .lineLimit(1)
            Spacer(minLength: WhooshSpacing.s8)
            Text(item.timeWords()).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
        }
        .whooshAx(NotchIds.openHeader)
    }

    /// While the reply is written the mail is one line: who, and its first words.
    private var foldedMail: some View {
        HStack(spacing: WhooshSpacing.s6) {
            Image(systemName: "chevron.down").font(.system(size: WhooshTypography.caption2.size, weight: .semibold)).foregroundStyle(WhooshPalette.secondaryLabel)
            Text(item.sender).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.label).lineLimit(1).layoutPriority(1)
            Text("· " + model.firstWords(of: item, app)).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, WhooshSpacing.s10)
        .frame(height: NotchMetrics.smallButton)
        .background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: WhooshRadius.libraryTile, style: .continuous))
        .whooshAx(NotchIds.openFolded)
    }

    /// "Folded: 4 earlier messages, Signature": what the engine set aside, named, one line.
    private var foldLine: some View {
        HStack(spacing: WhooshSpacing.s6) {
            Image(systemName: "chevron.down").font(.system(size: WhooshTypography.caption2.size, weight: .semibold))
            Text(NotchCopy.folded(words.folds)).whooshText(WhooshTypography.caption).lineLimit(1)
            Spacer(minLength: 0)
        }
        .foregroundStyle(WhooshPalette.secondaryLabel)
        .padding(.horizontal, WhooshSpacing.s10)
        .frame(height: NotchMetrics.foldLine)
        .background(WhooshPalette.fill.opacity(0.6), in: RoundedRectangle(cornerRadius: WhooshRadius.libraryTile, style: .continuous))
        .whooshAx(NotchIds.openFolds)
    }

    /// Under the words of a mail that has a page: how many pictures the words leave out, and the button to the full mail.
    private var wordsLine: some View {
        HStack(spacing: WhooshSpacing.s8) {
            if words.pictures > 0 { Text(NotchCopy.picturesNotLoaded(Int(words.pictures))).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.tertiaryLabel).lineLimit(1) }
            Spacer(minLength: 0)
            NotchGhostButton(symbol: nil, title: NotchCopy.fullMail, key: NotchFeature.fullMailKey, height: NotchMetrics.foldLine) { _ = model.toggleFullMail(app) }
                .whooshAx(NotchIds.openFullMail)
        }
    }

    /// Under the page: what happened to its pictures (loaded, or left out when the setting is off) and its tracking pixels, and the way back.
    private var pageLine: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Text(NotchCopy.pageFacts(pictures: Int(words.pictures), trackers: Int(words.trackers), loaded: model.alerts.fullMailLoadsPictures))
                .whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.tertiaryLabel).lineLimit(1)
                .whooshAx(NotchIds.openPageFacts)
            Spacer(minLength: 0)
            NotchGhostButton(symbol: nil, title: NotchCopy.itsWords, key: NotchFeature.fullMailKey, height: NotchMetrics.foldLine) { _ = model.toggleFullMail(app) }
                .whooshAx(NotchIds.openFullMail)
        }
    }

    /// Reply (filled), Archive, then Mark read and Snooze as picture and key, and the menu.
    private var actions: some View {
        HStack(spacing: WhooshSpacing.s6) {
            NotchGhostButton(symbol: "arrowshape.turn.up.left", title: NotchCopy.reply, key: NotchFeature.replyKey, primary: true) { _ = model.reply(app) }
                .whooshAx(NotchIds.openReply)
            NotchGhostButton(symbol: "archivebox", title: NotchCopy.archive, key: NotchFeature.archiveKey) { model.archive(id: item.id, app) }
                .whooshAx(NotchIds.openArchive)
            if !item.read {
                NotchGhostButton(symbol: "envelope.open", title: NotchCopy.markRead, key: NotchFeature.readKey, titleShown: false) { model.markRead(id: item.id, app) }
                    .whooshAx(NotchIds.openMarkRead)
                    .transition(.opacity)
            }
            NotchGhostButton(symbol: "clock", title: NotchCopy.snooze, key: NotchFeature.snoozeKey, titleShown: false) { _ = model.toggleSnooze() }
                .whooshAx(NotchIds.openSnooze)
            Spacer(minLength: WhooshSpacing.s8)
            Menu {
                if item.read { Button(NotchCopy.markUnread) { _ = model.markUnread(id: item.id, app) } }
                Button(NotchCopy.openInWhoosh) { _ = model.openInWhoosh(app) }
                Button(NotchCopy.delete, role: .destructive) { _ = model.delete(id: item.id, app) }
            } label: {
                Image(systemName: "ellipsis").font(.system(size: WhooshTypography.subhead.size, weight: .semibold)).foregroundStyle(WhooshPalette.secondaryLabel)
                    .frame(width: NotchMetrics.smallButton, height: NotchMetrics.smallButton)
                    .background(WhooshPalette.fill, in: Circle())
                    .contentShape(Circle())
            }
            .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden).fixedSize()
            .accessibilityLabel(NotchCopy.more)
            .whooshAx(NotchIds.openMore)
        }
        .padding(.top, WhooshSpacing.s2)
        .animation(WhooshMotion.hover, value: item.read)
    }

    /// The first two attachments as chips, the rest as "+N" (the whole list is one chip row; the composer holds the files).
    @ViewBuilder private var attachmentChips: some View {
        let files = attachments
        if !files.isEmpty {
            HStack(spacing: WhooshSpacing.s4) {
                ForEach(files.prefix(2), id: \.id) { f in
                    HStack(spacing: WhooshSpacing.s6) {
                        Image(systemName: "paperclip").font(.system(size: WhooshTypography.subhead.size, weight: .regular)).foregroundStyle(WhooshPalette.secondaryLabel)
                        Text(f.fileName).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label).lineLimit(1)
                        Text(ByteCountFormatter.string(fromByteCount: Int64(f.sizeBytes), countStyle: .file)).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.tertiaryLabel)
                    }
                    .padding(.horizontal, WhooshSpacing.s10)
                    .frame(height: NotchMetrics.smallButton)
                    .background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: WhooshRadius.libraryTile, style: .continuous))
                }
                if files.count > 2 {
                    Text("+\(files.count - 2)").whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
                }
            }
            .whooshAx(NotchIds.openAttachments)
            .accessibilityLabel(NotchCopy.attachments(files.count))
        }
    }
}

/// The mail's words: up to `NOTCH_WORDS_MAX_PARAGRAPHS` paragraphs. A text taller than the card's room is cut at `NOTCH_WORDS_MAX_H` and fades out
/// at its lower edge, so a cut reads as "there is more" (Full mail or Open in Whoosh shows the rest), never as a broken line.
struct NotchWordsBlock: View {
    let paragraphs: [String]

    private var text: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
            ForEach(Array(paragraphs.prefix(WhooshConstants.NOTCH_WORDS_MAX_PARAGRAPHS).enumerated()), id: \.offset) { _, p in
                Text(p).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label).fixedSize(horizontal: false, vertical: true)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    var body: some View {
        let room = CGFloat(WhooshConstants.NOTCH_WORDS_MAX_H)
        ViewThatFits(in: .vertical) {
            text
            text.frame(height: room, alignment: .top).clipped()
                .mask(LinearGradient(stops: [.init(color: .black, location: 0), .init(color: .black, location: 0.72), .init(color: .clear, location: 1)], startPoint: .top, endPoint: .bottom))
        }
        .frame(maxHeight: room, alignment: .top)
        .whooshAx(NotchIds.openBody)
    }
}

/// The full mail: the sender's page in the reader's own page view (the same sanitised document, the same rules), in a scroll area of the card. It is
/// made when the button is pressed and gone when the words come back or the mail closes. Pictures follow the notch's own setting.
struct NotchPage: View {
    let item: NotchItem
    let loadPictures: Bool
    @Bindable var app = AppModel.shared

    var body: some View {
        Group {
            if let mail = app.core.body(messageId: item.id) {
                ScrollView(.vertical) {
                    MessageBodyView(content: ReaderThread.content(mail, trackers: Int(mail.trackersBlocked)), sender: item.address, pictures: loadPictures)
                        .id(item.id)
                }
            } else {
                Color.clear
            }
        }
        .frame(height: CGFloat(WhooshConstants.NOTCH_PAGE_H))
        .clipShape(RoundedRectangle(cornerRadius: WhooshRadius.libraryTile, style: .continuous))
        .whooshAx(NotchIds.openPage)
    }
}

/// Snooze from the notch: the three times the app names everywhere (later today, tomorrow morning, next week), one press each.
struct NotchSnoozeChoices: View {
    let model: NotchModel
    static let choices: [(word: String, when: SnoozeWhen, key: String)] = [("later-today", .laterToday, "1"), ("tomorrow", .tomorrowMorning, "2"), ("next-week", .nextWeek, "3")]

    var body: some View {
        HStack(spacing: WhooshSpacing.s6) {
            ForEach(Self.choices, id: \.word) { c in
                NotchGhostButton(symbol: nil, title: NotchCopy.snoozeChoice(c.when), key: c.key) { _ = model.snooze(c.when, AppModel.shared) }
                    .whooshAx(NotchIds.openSnoozePick(c.word))
            }
            Spacer(minLength: 0)
        }
    }
}
