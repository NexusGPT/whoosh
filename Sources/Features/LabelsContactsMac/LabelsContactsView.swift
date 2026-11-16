import SwiftUI

/// mac-labels-contacts: the three-pane spine with the glass sidebar (labels across accounts, smart folders, people)
/// and one detail pane: the label, the smart-folder rule editor, or the sender card.
struct LabelsContactsView: View {
    @State private var model = LabelsContactsModel.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        HStack(spacing: 0) {
            LabelsSidebar(model: model).frame(width: WhooshMetrics.blockWidth / 2)
            ZStack {
                Rectangle().fill(WhooshPalette.background).ignoresSafeArea()
                switch model.pane {
                case .labels: LabelDetail(model: model).transition(.opacity)
                case .folder: SmartFolderEditor(model: model).transition(.opacity)
                case .contact:
                    ContactCardView(model: model)
                        .id(model.card?.addresses.first ?? "")
                        .transition(reduceMotion ? .opacity : .move(edge: .trailing).combined(with: .opacity))
                }
            }
            .animation(reduceMotion ? WhooshMotion.readerSwap : .spring, value: model.pane)
            .animation(.spring, value: model.contactAddress)
        }
        .ignoresSafeArea()
        .onAppear { model.reloadLabels(); model.reloadFolders(); model.reloadPeople() }
        .accessibilityIdentifier("mac-labels-contacts")
    }
}

