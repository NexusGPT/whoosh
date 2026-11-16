import SwiftUI

/// The iPhone's navigation, drawn: Mailboxes is the root, and what `PhoneRouter.nav` holds is pushed on it with the system push (the inbox,
/// a thread, Settings, Labels and contacts); the composer is a sheet, the palette is a sheet (its own host), the search is drawn in place
/// inside the inbox (InboxPhoneScreen). The stack keeps each screen alive under the next, so Back lands on the same scroll position.
/// No motion is written here: the push, the pop and the sheets are the system's, which reduces itself under Reduce Motion.
struct PhoneRootStack: View {
    private let router = PhoneRouter.shared
    private let m = AppModel.shared

    var body: some View {
        NavigationStack(path: Binding(get: { router.nav.path }, set: { router.pathChanged($0) })) {
            MailboxesPhone { router.showInbox() }
                .navigationDestination(for: PhoneRoute.self) { destination($0) }
                .phoneBarHidden()
        }
        .sheet(isPresented: Binding(get: { router.nav.sheet == .composer }, set: { if !$0 { router.sheetDismissed(.composer) } })) {
            ComposerPhoneScreen().interactiveDismissDisabled(ComposerModel.shared.isCounting)
        }
        .palettePhoneSheet()
        // The screens' own doors set `AppModel.overlay`; the router reads it, so the socket's clicks and every lane's door land here too.
        .onChange(of: m.overlay) { _, _ in router.overlayChanged() }
        .onChange(of: SearchPhoneModel.shared.active) { _, now in router.searchChanged(active: now) }
        .onOpenURL { _ = router.open(url: $0) }
    }

    @ViewBuilder private func destination(_ route: PhoneRoute) -> some View {
        switch route {
        case .inbox: InboxPhoneScreen { router.showMailboxes() }.phoneBarHidden()
        case .thread: ThreadPhoneScreen().phoneBarHidden()
        case .settings: SettingsPhoneRoot().phoneBarHidden()
        case .labelsContacts: LabelsContactsPhoneRoot().phoneBarHidden()
        }
    }
}
