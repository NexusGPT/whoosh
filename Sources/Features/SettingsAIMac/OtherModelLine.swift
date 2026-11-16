import SwiftUI

private let D = "mac-settings-ai"

/// The last line of the list of on-device decision models (the owner's pick M1, board decision-models-r1, the constant `other` and the drawing "Another
/// model, refused"): "Other model", an empty circle, one grey fact line and Add. Add turns the line into a field holding a Hugging Face address and Check
/// with its Return cap; Escape leaves the field. A model the app runs becomes a line of the list above it; any other is refused in the amber line
/// (the dot and one sentence) under the field.
struct OtherModelLine: View {
    let store: AISettingsStore
    @FocusState private var focused: Bool
    private var address: Binding<String> { Binding(get: { store.otherAddress }, set: { store.otherAddress = $0 }) }

    var body: some View {
        HStack(alignment: .top, spacing: WhooshSpacing.s10) {
            DecisionModelMark(inUse: false).padding(.top, WhooshSpacing.s2)
            VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                Text(AIWords.otherModel).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label)
                if store.otherOpen { field } else {
                    Text(AIWords.otherModelFact).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                        .accessibilityIdentifier("\(D).decisions.other.facts")
                }
                if store.otherOpen, let why = store.otherRefusal { refusal(why) }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if !store.otherOpen {
                SettingsButton(title: AIWords.add, ax: "\(D).decisions.other.add") { store.openOther() }
            }
        }
        .padding(.horizontal, SettingsMetrics.rowSideX).padding(.vertical, WhooshSpacing.s10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .animation(.easeOut(duration: WhooshMotion.rowInsert), value: store.otherOpen)
        .animation(.easeOut(duration: WhooshMotion.rowInsert), value: store.otherRefusal)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("\(D).decisions.other")
    }

    /// The field (board `.fld.typed`, the row's width) and Check (board `.b2.pri` with its Return cap).
    private var field: some View {
        HStack(spacing: WhooshSpacing.s8) {
            TextField("", text: address)
                .textFieldStyle(.plain)
                .font(.system(size: 12.5))
                .foregroundStyle(WhooshPalette.label)
                .focused($focused)
                .onSubmit { store.checkOther() }
                #if os(macOS)   // Escape leaves the field: a Mac command (unavailable on the iPhone, where this file compiles too)
                .onExitCommand { store.closeOther() }
                #endif
                .padding(.horizontal, 8).frame(maxWidth: .infinity).frame(height: 26)
                .background(RoundedRectangle(cornerRadius: 7, style: .continuous).fill(WhooshPalette.field))
                .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: 0.5))
                .accessibilityLabel(AIWords.otherModel)
                .accessibilityIdentifier("\(D).decisions.other.field")
            SettingsButton(title: AIWords.check, key: "\u{21A9}", primary: true, ax: "\(D).decisions.other.check") { store.checkOther() }
                .keyboardShortcut(.defaultAction)
                .disabled(store.otherChecking)
                .opacity(store.otherChecking ? 0.6 : 1)
        }
        .padding(.top, WhooshSpacing.s6)
        .onAppear { focused = true }
    }

    /// The amber line (board `.wl`): the dot and one sentence, no button.
    private func refusal(_ why: String) -> some View {
        HStack(spacing: WhooshSpacing.s8) {
            AmberDot()
            Text(why).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.warning).fixedSize(horizontal: false, vertical: true)
        }
        .padding(.top, WhooshSpacing.s6)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("\(D).decisions.other.refusal")
    }
}

extension OtherModelLine {
    /// What the line shows, as the checks read it. A pure read of what the screen holds.
    @MainActor static func row(_ s: AISettingsStore) -> [String: Any] {
        ["name": AIWords.otherModel, "facts": s.otherOpen ? "" : AIWords.otherModelFact, "in_use": false, "open": s.otherOpen,
         "address": s.otherOpen ? s.otherAddress : "", "focused": s.keyFocus == AISettingsStore.otherField, "checking": s.otherChecking,
         "refusal": s.otherOpen ? (s.otherRefusal ?? "") : "", "button": s.otherOpen ? AIWords.check : AIWords.add, "key": s.otherOpen ? "\u{21A9}" : ""]
    }
}

extension DecisionModelEntry {
    /// The line as the list shows it: an added model's licence is the hub tag's id, or words for "other" and for none (the catalogue's lines as they are).
    @MainActor var shownOnList: DecisionModelEntry {
        guard DecisionModels.shared.isAdded(id) else { return self }
        let words = licence.isEmpty ? AIWords.noLicenceStated : (licence.lowercased() == "other" ? AIWords.ownLicence : licence)
        return DecisionModelEntry(id: id, name: name, indexRow: indexRow, family: family, parametersBillions: parametersBillions, downloadMB: downloadMB,
                                  licence: words, page: page, score: score, scoreEdition: scoreEdition, base: base, files: files, checked: checked,
                                  folder: folder, checkpoint: checkpoint)
    }
}
