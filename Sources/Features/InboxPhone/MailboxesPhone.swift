import SwiftUI

/// The Mailboxes screen (mail-26 frame 2): the root the inbox's Back returns to. Unified mailboxes, then the
/// accounts in their four groups; a group folds open in place; tick circles pick several, "Show N selected" opens them.
public struct MailboxesPhone: View {
    var onOpen: () -> Void = {}
    @State private var open: Set<String> = []
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private let m = AppModel.shared
    private let state = InboxPhoneState.shared

    public init(onOpen: @escaping () -> Void = {}) { self.onOpen = onOpen }

    /// The unified mailboxes after All Inboxes (mail-26 frame 2, box 3); counts are the core's, absent = no number.
    static let unified = [("VIPs", "star"), ("Pinned", "pin"), ("Snoozed", "clock"), ("Sent", "paperplane"), ("Drafts", "doc")]

    public var body: some View {
        let accounts = PhoneAccounts.all(m)
        ZStack(alignment: .bottom) {
            PhoneGround.grouped.ignoresSafeArea()
            ScrollView {
                VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
                    Text("Mailboxes").phoneText(PhoneType.largeTitle).foregroundStyle(WhooshPalette.label)
                        .padding(.horizontal, PhoneMetrics.gutter).padding(.top, PhoneMetrics.titleTop).accessibilityAddTraits(.isHeader)
                    section("Unified")
                    card {
                        mailbox("All Inboxes", "tray", count: m.counts.mailboxes["all inboxes"].map(Int.init)) { state.picked = []; state.apply(m); onOpen() }
                        ForEach(Self.unified, id: \.0) { name, symbol in
                            mailbox(name, symbol, count: m.counts.mailboxes[name.lowercased()].map(Int.init)) { m.overlay = "iphone-mailbox:\(name)" }
                        }
                    }
                    section("Accounts", trailing: "\(accounts.count)")
                    card {
                        ForEach(PhoneAccounts.groups, id: \.self) { g in
                            let members = accounts.filter { $0.group == g }
                            if !members.isEmpty { group(g, members) }
                        }
                    }
                    // Both rows push on the stack (Sources/App/PhoneRouter.swift); the ids are what the socket's click reaches them by.
                    card {
                        mailbox("Settings", "gear", count: nil) { PhoneRouter.shared.openSettings() }
                            .accessibilityIdentifier("iphone-mailboxes.settings")
                        mailbox("Labels", "tag", count: nil) { PhoneRouter.shared.openLabelsContacts() }
                            .accessibilityIdentifier("iphone-mailboxes.labels")
                    }
                }
                .padding(.bottom, PhoneMetrics.listBottomRoom)
            }
            if state.picked.count > 1 {
                Button { state.apply(m); onOpen() } label: {
                    Text("Show \(state.picked.count) selected").phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.label)
                        .frame(width: PhoneMetrics.undoWidth, height: PhoneMetrics.roundButton).whooshGlass(in: Capsule())
                }
                .buttonStyle(PressStyle()).accessibilityIdentifier("iphone-mailboxes.show-selected")
                .transition(.offset(y: WhooshMotion.undoRise).combined(with: .opacity))
            }
        }
        .overlay(alignment: .topTrailing) {
            Text("Edit").phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.label)
                .frame(width: PhoneMetrics.selectWidth, height: PhoneMetrics.roundButton).whooshGlass(in: Capsule())
                .padding(.horizontal, PhoneMetrics.gutter)
        }
        .animation(reduceMotion ? nil : WhooshMotion.newRow, value: state.picked.count > 1)
    }

    private func section(_ title: String, trailing: String = "") -> some View {
        HStack { Text(title); Spacer(); Text(trailing) }
            .phoneText(PhoneType.footnote, emphasized: true).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
            .padding(.horizontal, PhoneMetrics.gutter * 2).padding(.top, WhooshSpacing.s12)
    }

    private func card<C: View>(@ViewBuilder _ c: () -> C) -> some View {
        VStack(spacing: 0) { c() }
            .background(PhoneGround.card, in: RoundedRectangle(cornerRadius: PhoneMetrics.cardRadius, style: .continuous))
            .padding(.horizontal, PhoneMetrics.gutter)
    }

    private func mailbox(_ name: String, _ symbol: String, count: Int?, _ go: @escaping () -> Void) -> some View {
        Button(action: go) {
            HStack(spacing: WhooshSpacing.s12) {
                Image(systemName: symbol).foregroundStyle(WhooshPalette.accent).frame(width: WhooshSpacing.s24)
                Text(name).foregroundStyle(WhooshPalette.label)
                Spacer()
                if let count { Text("\(count)").foregroundStyle(WhooshPalette.sidebarSecondaryLabel) }
                Image(systemName: "chevron.right").font(.system(size: PhoneType.footnote.size, weight: .semibold)).foregroundStyle(WhooshPalette.tertiaryLabel)
            }
            .phoneText(PhoneType.body).padding(.horizontal, PhoneMetrics.gutter).frame(height: PhoneMetrics.mailboxRow).contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private func group(_ g: String, _ members: [PhoneAccount]) -> some View {
        let all = Set(members.map(\.id))
        let unfolded = open.contains(g)
        return VStack(spacing: 0) {
            HStack(spacing: WhooshSpacing.s12) {
                tick(all.isSubset(of: state.picked)) { if all.isSubset(of: state.picked) { state.picked.subtract(all) } else { state.picked.formUnion(all) } }
                VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                    HStack(spacing: WhooshSpacing.s6) {
                        Text(g).phoneText(PhoneType.headline)
                        Text("\(members.count)").phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                    }
                    if !unfolded {
                        HStack(spacing: WhooshSpacing.s2) {
                            ForEach(members) { a in AccountMark(name: a.name, address: a.address, colour: a.colour, icon: a.icon, size: .compact, needsAttention: a.needsAttention) }
                        }
                    }
                }
                Spacer()
                Image(systemName: "chevron.down").rotationEffect(.degrees(unfolded ? 0 : -90)).foregroundStyle(WhooshPalette.tertiaryLabel)
            }
            .padding(.horizontal, PhoneMetrics.gutter).frame(minHeight: PhoneMetrics.groupRow).contentShape(Rectangle())
            .onTapGesture { withAnimation(reduceMotion ? nil : WhooshMotion.unfold) { if unfolded { open.remove(g) } else { open.insert(g) } } }
            if unfolded { ForEach(members) { accountRow($0) } }
        }
    }

    private func accountRow(_ a: PhoneAccount) -> some View {
        HStack(spacing: WhooshSpacing.s12) {
            tick(state.picked.contains(a.id)) { if state.picked.contains(a.id) { state.picked.remove(a.id) } else { state.picked.insert(a.id) } }
            PhoneTile(account: a, small: true)
            VStack(alignment: .leading, spacing: 0) {
                Text(a.name).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.label)
                Text(a.needsAttention ? "Sign in again" : a.address).phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, PhoneMetrics.gutter).frame(height: PhoneMetrics.accountRow).contentShape(Rectangle())
        .onTapGesture { state.picked = [a.id]; state.apply(m); onOpen() }
        .accessibilityIdentifier("iphone-mailboxes.account.\(a.id)")
    }

    private func tick(_ on: Bool, _ flip: @escaping () -> Void) -> some View {
        Button(action: flip) {
            Image(systemName: on ? "checkmark.circle.fill" : "circle").font(.system(size: PhoneMetrics.tick))
                .foregroundStyle(on ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(WhooshPalette.tertiaryLabel))
        }
        .buttonStyle(PressStyle()).accessibilityLabel(on ? "Selected" : "Not selected")
    }
}

/// The phone's inbox navigation: Mailboxes is the root; the inbox opens on top of it at launch (J4.10 lands on "All Inboxes"), and the
/// stack, its sheets and the in-place search are `PhoneRootStack` (Sources/App/PhoneRootStack.swift). The iPhone app root hosts this view.
public struct InboxPhoneRoot: View {
    public init() {}
    public var body: some View { PhoneRootStack() }
}

extension View {
    @ViewBuilder func phoneBarHidden() -> some View {
        #if os(iOS)
        self.toolbar(.hidden, for: .navigationBar)
        #else
        self
        #endif
    }
}
