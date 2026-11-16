import Foundation

/// The receiving account of a row, as the mark draws it. One rule with the Mac: name, address and colour from the
/// account, the icon from `Account.iconId` the way `AccountMark(account:)` picks it (an id → the library glyph,
/// none → the provider default). The contract has no accounts read yet (the Mac inbox list lane reports the same gap):
/// under the fixture core the fixture's own account list answers; under any other core the mark is neutral.
public struct PhoneAccount: Equatable, Sendable, Identifiable {
    public let id: String
    public let name: String
    public let address: String
    public let colour: String
    public let group: String?
    public let iconId: String?
    public let needsAttention: Bool

    /// What a screen reader says for the mark: the account's name (the tile's accessibility label and the probe's `mark_label`).
    public var markLabel: String { name }
    /// The icon id the probe reports: the library id, or "provider-default" (the Mac's rule).
    public var markIcon: String { iconId.flatMap { AccountMarkIcon.libraryIds[$0] != nil ? $0 : nil } ?? "provider-default" }
    public var icon: AccountMarkIcon { iconId.flatMap { AccountMarkIcon.libraryIds[$0] != nil ? .library($0) : nil } ?? .providerDefault }
    /// The SF Symbol the tile draws for that icon.
    public var symbol: String { iconId.flatMap { AccountMarkIcon.libraryIds[$0] } ?? AccountMarkIcon.defaultSymbol }
}

@MainActor
enum PhoneAccounts {
    private struct File: Decodable {
        struct Acc: Decodable { let id: String; let address: String; let name: String; let colour: String; let group: String?; let icon_id: String? }
        let accounts: [Acc]
    }
    private static let file: [File.Acc] = {
        guard let url = FixtureFile.locate(), let data = try? Data(contentsOf: url),
              let f = try? JSONDecoder().decode(File.self, from: data) else { return [] }
        return f.accounts
    }()

    /// Accounts in the fixture's order (the Mac sidebar's order); empty under a core with no accounts read.
    static func all(_ m: AppModel) -> [PhoneAccount] {
        guard m.host.kind == .fixture else { return [] }
        let failing = failingIds(m)
        return file.map { PhoneAccount(id: $0.id, name: $0.name, address: $0.address, colour: $0.colour, group: $0.group,
                                       iconId: $0.icon_id, needsAttention: failing.contains($0.id)) }
    }

    static func of(_ accountId: String, in m: AppModel) -> PhoneAccount {
        if let a = all(m).first(where: { $0.id == accountId }) { return a }
        return PhoneAccount(id: accountId, name: accountId, address: "", colour: "", group: nil, iconId: nil,
                            needsAttention: failingIds(m).contains(accountId))
    }

    /// Accounts whose sync reports an error: the dashed tile and the orange dot.
    static func failingIds(_ m: AppModel) -> Set<String> {
        Set(m.core.syncStates().filter { $0.lastError != nil }.map(\.accountId))
    }

    static let groups = ["Work", "Personal", "Clients", "Side"]
}
