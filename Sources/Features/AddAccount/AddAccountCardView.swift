import SwiftUI

/// The sheet's content, top to bottom: head, the two fields, the step line and its words, then (only when needed) the refusal, the
/// Esc question and Manual setup, then the footer. Everything above the footer keeps its place as the sheet grows downward.
struct AddAccountCardView: View {
    @Bindable var model: AddAccountModel
    let narrow: Bool
    let markSurface: String
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            AddAccountHeadView(model: model, surface: markSurface)
            if model.stage == .added {
                AddAccountStepView(model: model).padding(.top, 4)
                AddAccountSessionView(model: model).transition(.opacity)
            } else {
                AddAccountFieldsView(model: model, narrow: narrow).padding(.top, 18)
                AddAccountStepView(model: model).padding(.top, 14)
                AddAccountOAuthView(model: model)
                if model.hasProblem { AddAccountErrorView(model: model).padding(.top, 12).transition(.opacity) }
                if model.confirmingStop { AddAccountStopRow(model: model).padding(.top, 12).transition(.opacity) }
                if model.manualOpen { AddAccountManualView(model: model, narrow: narrow).transition(.opacity) }
            }
            footer.padding(.top, 22)
            if model.style == .inline && !model.added.isEmpty { AddAccountAddedList(model: model) }
            if model.stage == .added { doneChord }
        }
        .animation(AddAccountMetrics.motion(AddAccountMetrics.unfold, reduceMotion), value: layoutKey)
        .accessibilityElement(children: .contain).accessibilityIdentifier("\(AddAccountWords.surface).card")
    }

    /// What changes the card's height: each of these grows or shrinks it from the bottom.
    private var layoutKey: String {
        let parts: [String] = [model.stage.rawValue, String(model.manualOpen), String(model.hasProblem), String(model.confirmingStop),
                               String(model.added.count), model.oauthKind?.rawValue ?? "", model.oauthPhase.name]
        return parts.joined(separator: "-")
    }

    private var footer: some View {
        let f = AddAccountControls.footer(model)
        // Every label whole at any width and in any of the 14 languages (main107: "Ca…", "Conti…", "Manual…" at 960x600): the buttons keep their own width, and when the
        // one row does not fit the card the right-hand buttons drop to a second row under the left-hand ones.
        return ViewThatFits(in: .horizontal) {
            HStack(spacing: 8) {
                ForEach(f.left) { AddAccountButton(model: model, control: $0) }
                Spacer(minLength: 8)
                ForEach(f.right) { AddAccountButton(model: model, control: $0) }
            }
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 8) { ForEach(f.left) { AddAccountButton(model: model, control: $0) } }
                HStack(spacing: 8) { Spacer(minLength: 0); ForEach(f.right) { AddAccountButton(model: model, control: $0) } }
            }
        }
    }

    /// ⌘↩ is Done's second key, hidden: the board draws ↩ on the button.
    private var doneChord: some View {
        Button("") { _ = model.doneTapped() }.keyboardShortcut(.return, modifiers: .command).opacity(0).frame(width: 0, height: 0).accessibilityHidden(true)
    }
}
