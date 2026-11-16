import SwiftUI

/// The screen's navigation: the labels list is the root, a label, its mail or a person is pushed on it with the system push.
/// The door is `LabelsContactsPhoneState.openList/openLabel/openContact`, which also set the overlay (docs/lanes/labelscontactsphone.md).
public struct LabelsContactsPhoneRoot: View {
    @Bindable private var state = LabelsContactsPhoneState.shared

    public init() {}

    public var body: some View {
        NavigationStack(path: $state.path) {
            LabelsListPhone()
                .navigationDestination(for: PhonePage.self) { page in
                    switch page {
                    case .label(let id): LabelPagePhone(id: id)
                    case .mail(let id): LabelMailPhone(id: id)
                    case .contact(let address): ContactPagePhone(address: address)
                    }
                }
        }
    }
}
