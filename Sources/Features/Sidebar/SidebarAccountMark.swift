import SwiftUI

/// The account control (Components/AccountMark), with the first-sync share drawn over its ring while it backfills.
struct SidebarAccountMark: View {
    let account: SidebarAccount
    let model: AppModel
    var body: some View {
        AccountMark(account: account.account, size: .sidebar, needsAttention: account.problem != nil, hasUnread: account.unread > 0,
                    chosen: model.accountFilter == account.id)
            .axId("mac-main.sidebar.account-mark.\(account.id)")
            .overlay {
                if let p = account.backfill, p < 1 {   // a finished backfill (1.0, every fixture account) drew a full ring over the mark (lead fix 2026-11-08)
                    Circle().trim(from: 0, to: p)
                        .stroke(AccountColour.ring(account.account.colour), style: StrokeStyle(lineWidth: WhooshAccountMark.stroke, lineCap: .round))
                        .rotationEffect(.degrees(WhooshAccountMark.arcStartDegrees))
                        .animation(SidebarMotion.progress(p), value: p)
                }
            }
    }
}

/// An account's health in the owner's words: live, syncing, slowed by something, or needs sign-in.
struct SidebarHealth {
    let words: String
    let warns: Bool
    /// A live account says nothing about itself: the row shows its address instead of "Live" thirty times.
    var live: Bool { words == Words.text("Live") }
}

extension SidebarAccount {
    var health: SidebarHealth {
        if let p = problem {
            return p.contains("sign in") ? SidebarHealth(words: Words.text("Needs sign-in"), warns: true) : SidebarHealth(words: Words.text("Slowed: %@", p), warns: true)
        }
        if account.paused { return SidebarHealth(words: Words.text("Paused"), warns: false) }
        if let b = backfill, b < 1 { return SidebarHealth(words: Words.text("Syncing · %lld%%", Int((b * 100).rounded())), warns: false) }
        return SidebarHealth(words: Words.text("Live"), warns: false)
    }
}
