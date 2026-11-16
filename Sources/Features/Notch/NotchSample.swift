import Foundation

/// The stage's sample arrivals, for shots only (click `mac-notch.stage.sample`). Fictional people, copied from
/// fixtures/tests-j6.json; the real arrivals come from the push feed. Under the fixture core each sample first
/// becomes an inbox row through the fixture's own arrival door (contracts delta 13, CONTROL-SOCKET delta 53), the
/// same door `fixture deliver` uses: a notch mail with no inbox row has no thread for the quick reply to answer,
/// and the reply is drawn only when it answers the open mail (NotchReplyTarget.swift).
enum NotchSample {
    static let drops: [NotchDrop] = [
        NotchDrop(messageId: "msg-j6-next", accountId: "acc-04", senderName: "Jonas Weber", subject: "Lunch on Thursday?", loginCode: nil),
        NotchDrop(messageId: "msg-j6-code", accountId: "acc-04", senderName: "Northlight Sign-in", subject: "Your sign-in code", loginCode: "482915"),
    ]
    static let previews = ["msg-j6-code": "Your sign-in code is 482915. It expires in 10 minutes.",
                           "msg-j6-next": "Are you free at 12:30 on Thursday? Same place as last time.",
                           "msg-j6-third": "Your sign-in link for the supplier portal is ready. It works once and expires tonight."]
    static let addresses = ["msg-j6-code": "no-reply@signin-example.com", "msg-j6-next": "jonas@example.test", "msg-j6-third": "portal@supplier.example"]
    /// A third, already-read mail for the list shots (board a's hover shows three rows, two unread).
    static let third = NotchDrop(messageId: "msg-j6-third", accountId: "acc-02", senderName: "Linea Supply", subject: "Your sign-in link", loginCode: nil)

    @MainActor static func deliver() -> Bool {
        let app = AppModel.shared
        for d in drops.reversed() { file(d, app) }
        app.refresh()
        for d in drops.reversed() {
            let row = app.rows.first { $0.messageId == d.messageId }
            NotchModel.shared.arrive(d, preview: previews[d.messageId] ?? "", threadId: row?.threadId ?? "", address: addresses[d.messageId] ?? "")
        }
        return true
    }

    /// The sample's inbox row in the fixture core. A row already there (a second sample after a reset) stays as it is.
    @MainActor private static func file(_ d: NotchDrop, _ app: AppModel) {
        guard let core = app.host.fixture else { return }
        _ = try? core.deliverArrival(id: d.messageId, account: d.accountId, from: addresses[d.messageId] ?? "",
                                     sender: d.senderName, subject: d.subject, body: previews[d.messageId] ?? "")
    }

    @MainActor static func deliverThird() -> Bool {
        let m = NotchModel.shared
        m.arrive(third, preview: previews[third.messageId] ?? "", address: addresses[third.messageId] ?? "", receivedAt: Date().addingTimeInterval(-2040))
        if let i = m.items.firstIndex(where: { $0.id == third.messageId }) { m.items[i].read = true; m.items.append(m.items.remove(at: i)) }
        m.onChange()
        return true
    }
}
