import Foundation

/// A refusal in one sentence that says whose it is. The core's `ActionFailed` carries the account's name apart from
/// the reason ("Provanta RFP" · "needs you to sign in again"); the notch shows both, because "needs you to sign in
/// again" alone does not say which of thirty accounts needs it (fixtures/tests-j6.json `failing_line`).
enum NotchFailure {
    static func words(account: String, reason: String) -> String {
        guard !account.isEmpty, !reason.hasPrefix(account) else { return reason }
        return account + " " + reason
    }

    /// The quick reply's refusal for the mail open in the notch: the reply keeps only the reason, so the account's
    /// name comes from the open mail's own account.
    @MainActor static func replyWords(_ model: NotchModel) -> String? {
        guard let reason = QuickReplyModel.shared.failureWords, let item = model.openItem else { return nil }
        return words(account: RowAccount.of(item.accountId, in: AppModel.shared).name, reason: reason)
    }
}
